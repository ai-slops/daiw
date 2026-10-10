use std::f64::consts::TAU;

use crate::signal_graph::{
    BufferSlot, CompiledSignalProgram, Instruction, PreparedNote, PreparedRegion,
};
use crate::source::SourceBank;
use crate::validate::finite_samples;
use crate::{AudioError, Instrument};

pub(crate) trait BlockSink {
    fn write_block(&mut self, start_frame: usize, frames: &[[f32; 2]]) -> Result<(), AudioError>;
}

pub(crate) struct Collector {
    samples: Vec<f32>,
}

struct BlockScratch<'a> {
    frames: &'a mut [[f32; 2]],
    start: usize,
}

struct SourceReader<'a, S> {
    output_rate: u32,
    sources: &'a mut S,
    frame: [f32; 8],
}

struct ExecutionState<const BLOCK_FRAMES: usize> {
    master: [[f32; 2]; BLOCK_FRAMES],
    track: [[f32; 2]; BLOCK_FRAMES],
    filter_states: Vec<[f64; 2]>,
}

#[derive(Clone, Copy)]
struct BlockSpan {
    start: usize,
    frames: usize,
}

struct ExecutionBlock<'execution, 'program, 'source, S, const BLOCK_FRAMES: usize> {
    program: &'program CompiledSignalProgram<'program>,
    state: &'execution mut ExecutionState<BLOCK_FRAMES>,
    source_reader: &'execution mut SourceReader<'source, S>,
    span: BlockSpan,
}

impl Collector {
    pub fn new(frames: usize) -> Result<Self, AudioError> {
        let samples = frames.checked_mul(2).ok_or(AudioError::InvalidFrameCount)?;
        let mut output = Vec::new();
        output.try_reserve_exact(samples)?;
        Ok(Self { samples: output })
    }

    pub fn into_samples(self) -> Vec<f32> {
        self.samples
    }
}

impl BlockSink for Collector {
    fn write_block(&mut self, _start_frame: usize, frames: &[[f32; 2]]) -> Result<(), AudioError> {
        for frame in frames {
            self.samples.extend_from_slice(frame);
        }
        Ok(())
    }
}

pub(crate) fn render_blocks<const BLOCK_FRAMES: usize>(
    program: &CompiledSignalProgram<'_>,
    sources: &mut impl SourceBank,
    sink: &mut impl BlockSink,
) -> Result<(), AudioError> {
    const { assert!(BLOCK_FRAMES > 0, "render block must contain frames") };
    let mut state: ExecutionState<BLOCK_FRAMES> = ExecutionState::new(program)?;
    let mut source_reader = SourceReader {
        output_rate: program.provenance.sample_rate,
        sources,
        frame: [0.0; 8],
    };
    let mut block_start = 0;
    while block_start < program.provenance.total_frames {
        let block_frames = BLOCK_FRAMES.min(program.provenance.total_frames - block_start);
        let block_end = block_start + block_frames;
        ExecutionBlock {
            program,
            state: &mut state,
            source_reader: &mut source_reader,
            span: BlockSpan {
                start: block_start,
                frames: block_frames,
            },
        }
        .execute()?;
        sink.write_block(block_start, &state.master[..block_frames])?;
        block_start = block_end;
    }
    Ok(())
}

impl<const BLOCK_FRAMES: usize> ExecutionState<BLOCK_FRAMES> {
    fn new(program: &CompiledSignalProgram<'_>) -> Result<Self, AudioError> {
        if program.latency_frames() != 0 {
            return Err(AudioError::InvalidSignalGraph {
                issue: "nonzero execution latency is unsupported",
            });
        }
        if !matches!(
            program.buffer_slots(),
            [BufferSlot::Master, BufferSlot::Track]
        ) {
            return Err(AudioError::InvalidSignalGraph {
                issue: "execution buffer plan is invalid",
            });
        }
        let mut filter_states = Vec::new();
        filter_states.try_reserve_exact(program.track_count())?;
        filter_states.resize(program.track_count(), [0.0; 2]);
        Ok(Self {
            master: [[0.0; 2]; BLOCK_FRAMES],
            track: [[0.0; 2]; BLOCK_FRAMES],
            filter_states,
        })
    }
}

impl<S: SourceBank, const BLOCK_FRAMES: usize> ExecutionBlock<'_, '_, '_, S, BLOCK_FRAMES> {
    fn execute(&mut self) -> Result<(), AudioError> {
        for instruction in self.program.instructions() {
            self.execute_instruction(*instruction)?;
        }
        Ok(())
    }

    fn execute_instruction(&mut self, instruction: Instruction) -> Result<(), AudioError> {
        let block_frames = self.span.frames;
        match instruction {
            Instruction::ClearMaster => self.state.master[..block_frames].fill([0.0; 2]),
            Instruction::ClearTrack { track: _ } => self.state.track[..block_frames].fill([0.0; 2]),
            Instruction::RenderTrackSource { track } => {
                let prepared = &self.program.tracks[track];
                let mut block = BlockScratch {
                    frames: &mut self.state.track[..block_frames],
                    start: self.span.start,
                };
                for note in &self.program.notes[prepared.note_range.clone()] {
                    accumulate_note(&mut block, note, self.program.provenance.sample_rate);
                }
                for region in &self.program.regions[prepared.region_range.clone()] {
                    self.source_reader.accumulate_region(&mut block, region)?;
                }
            }
            Instruction::LowPass { track } => {
                let prepared = &self.program.tracks[track];
                let filter_state = &mut self.state.filter_states[track];
                for frame in &mut self.state.track[..block_frames] {
                    for (channel, sample) in frame.iter_mut().enumerate() {
                        filter_state[channel] += prepared.filter_coefficient
                            * (f64::from(*sample) - filter_state[channel]);
                        *sample = filter_state[channel] as f32;
                    }
                }
            }
            Instruction::ValidateTrack { track } => {
                finite_samples(
                    self.state.track[..block_frames].as_flattened(),
                    &self.program.tracks[track].track.id,
                )?;
            }
            Instruction::GainPanIntoMaster { track } => {
                let pan_gain = self.program.tracks[track].pan_gain;
                for (target, source) in self.state.master[..block_frames]
                    .iter_mut()
                    .zip(&self.state.track[..block_frames])
                {
                    target[0] += (f64::from(source[0]) * pan_gain[0]) as f32;
                    target[1] += (f64::from(source[1]) * pan_gain[1]) as f32;
                }
            }
            Instruction::ValidateMaster => {
                finite_samples(self.state.master[..block_frames].as_flattened(), "master")?;
            }
        }
        Ok(())
    }
}

impl BlockScratch<'_> {
    fn overlap(&self, event_start: usize, event_end: usize) -> Option<std::ops::Range<usize>> {
        let start = event_start.max(self.start);
        let end = event_end.min(self.start + self.frames.len());
        (start < end).then_some(start..end)
    }
}

fn accumulate_note(block: &mut BlockScratch<'_>, note: &PreparedNote<'_>, sample_rate: u32) {
    let Some(overlap) = block.overlap(note.start, note.end) else {
        return;
    };
    for absolute_frame in overlap {
        let offset = absolute_frame - note.start;
        let time = offset as f64 / f64::from(sample_rate);
        let progress = offset as f64 / note.frames.max(1) as f64;
        let envelope = note_envelope(progress, note.instrument);
        let wave = instrument_sample(note.instrument, note.frequency, time, offset as u64);
        let sample = (wave * envelope * note.note.velocity.clamp(0.0, 1.0)) as f32;
        let output = &mut block.frames[absolute_frame - block.start];
        output[0] += sample;
        output[1] += sample;
    }
}

impl<S: SourceBank> SourceReader<'_, S> {
    fn accumulate_region(
        &mut self,
        block: &mut BlockScratch<'_>,
        region: &PreparedRegion<'_>,
    ) -> Result<(), AudioError> {
        let Some(overlap) = block.overlap(region.start, region.end) else {
            return Ok(());
        };
        for absolute_frame in overlap {
            let target_offset = absolute_frame - region.start;
            let offset = u64::try_from(target_offset).map_err(|_| AudioError::InvalidFrameCount)?;
            let source_offset = offset
                .checked_mul(u64::from(region.source_rate))
                .ok_or(AudioError::InvalidFrameCount)?
                / u64::from(self.output_rate);
            if source_offset >= region.source_frames {
                break;
            }
            let source_index = region
                .source_start
                .checked_add(source_offset)
                .ok_or(AudioError::InvalidFrameCount)?;
            if !self
                .sources
                .read_frame(&region.region.asset_id, source_index, &mut self.frame)?
            {
                return Err(AudioError::InvalidPcmLength);
            }
            let output = &mut block.frames[absolute_frame - block.start];
            output[0] += self.frame[0] * region.gain;
            output[1] += self.frame[usize::from(region.channels != 1)] * region.gain;
        }
        Ok(())
    }
}

fn note_envelope(progress: f64, instrument: Instrument) -> f64 {
    let attack = match instrument {
        Instrument::Pad => 0.2,
        Instrument::Synth
        | Instrument::Bass
        | Instrument::Kick
        | Instrument::Snare
        | Instrument::Hat
        | Instrument::Sample => 0.01,
    };
    let onset = (progress / attack).min(1.0);
    let release = ((1.0 - progress) / 0.12).clamp(0.0, 1.0);
    onset * release
}

fn instrument_sample(instrument: Instrument, frequency: f64, time: f64, seed: u64) -> f64 {
    let phase = (frequency * time).fract();
    match instrument {
        Instrument::Synth => (phase * 2.0 - 1.0) * 0.25,
        Instrument::Bass => ((TAU * phase).sin() + 0.35 * (TAU * phase * 2.0).sin()) * 0.3,
        Instrument::Kick => {
            (TAU * (45.0 + 95.0 * (-time * 18.0).exp()) * time).sin() * (-time * 10.0).exp() * 0.9
        }
        Instrument::Snare => noise(seed) * (-time * 14.0).exp() * 0.5,
        Instrument::Hat => noise(seed.wrapping_mul(17)) * (-time * 45.0).exp() * 0.28,
        Instrument::Pad => ((TAU * phase).sin() + 0.5 * (TAU * (phase + time * 0.7)).sin()) * 0.18,
        Instrument::Sample => 0.0,
    }
}

fn noise(seed: u64) -> f64 {
    let value = seed
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let mixed = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((mixed ^ (mixed >> 31)) as f64 / u64::MAX as f64) * 2.0 - 1.0
}
