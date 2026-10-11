use crate::AudioError;
use crate::render_synthesis::accumulate_note;
use crate::signal_graph::{BufferSlot, CompiledDspProgram, Instruction, PreparedRegion};
use crate::source::SourceBank;

pub(crate) trait BlockSink {
    fn write_block(&mut self, start_frame: usize, frames: &[[f32; 2]]) -> Result<(), AudioError>;
}

pub(crate) struct Collector {
    samples: Vec<f32>,
}

pub(crate) trait KernelSource {
    type Error;

    fn read_frame(
        &mut self,
        source_index: usize,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KernelError<E> {
    Source(E),
    FrameIndexOverflow,
    NonFiniteTrack { track: usize },
    NonFiniteMaster,
}

pub(crate) struct KernelState<'a> {
    pub(crate) master: &'a mut [[f32; 2]],
    pub(crate) track: &'a mut [[f32; 2]],
    pub(crate) filters: &'a mut [[f64; 2]],
    pub(crate) source_frame: &'a mut [f32; 8],
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

struct OfflineSources<'a, S> {
    ids: &'a [String],
    bank: &'a mut S,
}

impl<S: SourceBank> KernelSource for OfflineSources<'_, S> {
    type Error = AudioError;

    fn read_frame(
        &mut self,
        source_index: usize,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<(), Self::Error> {
        let id = self
            .ids
            .get(source_index)
            .ok_or(AudioError::InvalidSignalGraph {
                issue: "region source index is invalid",
            })?;
        if self.bank.read_frame(id, frame, output)? {
            Ok(())
        } else {
            Err(AudioError::InvalidPcmLength)
        }
    }
}

pub(crate) fn render_blocks<const BLOCK_FRAMES: usize>(
    program: &CompiledDspProgram,
    sources: &mut impl SourceBank,
    sink: &mut impl BlockSink,
) -> Result<(), AudioError> {
    const { assert!(BLOCK_FRAMES > 0, "render block must contain frames") };
    validate_program(program)?;
    let mut master = [[0.0; 2]; BLOCK_FRAMES];
    let mut track = [[0.0; 2]; BLOCK_FRAMES];
    let mut filters = vec![[0.0; 2]; program.track_count()];
    let mut source_frame = [0.0; 8];
    let mut source_adapter = OfflineSources {
        ids: &program.source_ids,
        bank: sources,
    };
    let mut block_start = 0;
    while block_start < program.total_frames() {
        let frames = BLOCK_FRAMES.min(program.total_frames() - block_start);
        execute_span(
            program,
            KernelState {
                master: &mut master[..frames],
                track: &mut track[..frames],
                filters: &mut filters,
                source_frame: &mut source_frame,
            },
            &mut source_adapter,
            block_start,
        )
        .map_err(|error| offline_error(program, error))?;
        sink.write_block(block_start, &master[..frames])?;
        block_start += frames;
    }
    Ok(())
}

pub(crate) fn execute_span<S: KernelSource>(
    program: &CompiledDspProgram,
    state: KernelState<'_>,
    sources: &mut S,
    start: usize,
) -> Result<(), KernelError<S::Error>> {
    for instruction in program.instructions() {
        match *instruction {
            Instruction::ClearMaster => state.master.fill([0.0; 2]),
            Instruction::ClearTrack { track: _ } => state.track.fill([0.0; 2]),
            Instruction::RenderTrackSource { track } => {
                let prepared = &program.tracks[track];
                for note in &program.notes[prepared.note_range.clone()] {
                    accumulate_note(state.track, start, note, program.sample_rate());
                }
                for region in &program.regions[prepared.region_range.clone()] {
                    accumulate_region(
                        state.track,
                        start,
                        region,
                        program.sample_rate(),
                        sources,
                        state.source_frame,
                    )?;
                }
            }
            Instruction::LowPass { track } => {
                let prepared = &program.tracks[track];
                for frame in state.track.iter_mut() {
                    for (channel, sample) in frame.iter_mut().enumerate() {
                        state.filters[track][channel] += prepared.filter_coefficient
                            * (f64::from(*sample) - state.filters[track][channel]);
                        *sample = state.filters[track][channel] as f32;
                    }
                }
            }
            Instruction::ValidateTrack { track } => {
                if state
                    .track
                    .as_flattened()
                    .iter()
                    .any(|sample| !sample.is_finite())
                {
                    return Err(KernelError::NonFiniteTrack { track });
                }
            }
            Instruction::GainPanIntoMaster { track } => {
                let pan_gain = program.tracks[track].pan_gain;
                for (target, source) in state.master.iter_mut().zip(state.track.iter()) {
                    target[0] += (f64::from(source[0]) * pan_gain[0]) as f32;
                    target[1] += (f64::from(source[1]) * pan_gain[1]) as f32;
                }
            }
            Instruction::ValidateMaster => {
                if state
                    .master
                    .as_flattened()
                    .iter()
                    .any(|sample| !sample.is_finite())
                {
                    return Err(KernelError::NonFiniteMaster);
                }
            }
        }
    }
    Ok(())
}

fn accumulate_region<S: KernelSource>(
    frames: &mut [[f32; 2]],
    block_start: usize,
    region: &PreparedRegion,
    output_rate: u32,
    sources: &mut S,
    source_frame: &mut [f32; 8],
) -> Result<(), KernelError<S::Error>> {
    let start = region.start.max(block_start);
    let end = region.end.min(block_start + frames.len());
    for absolute_frame in start..end {
        let offset = u64::try_from(absolute_frame - region.start)
            .map_err(|_| KernelError::FrameIndexOverflow)?;
        let source_offset = offset
            .checked_mul(u64::from(region.source_rate))
            .ok_or(KernelError::FrameIndexOverflow)?
            / u64::from(output_rate);
        if source_offset >= region.source_frames {
            break;
        }
        let source_frame_index = region
            .source_start
            .checked_add(source_offset)
            .ok_or(KernelError::FrameIndexOverflow)?;
        sources
            .read_frame(region.source_index, source_frame_index, source_frame)
            .map_err(KernelError::Source)?;
        let output = &mut frames[absolute_frame - block_start];
        output[0] += source_frame[0] * region.gain;
        output[1] += source_frame[usize::from(region.channels != 1)] * region.gain;
    }
    Ok(())
}

pub(crate) fn validate_program(program: &CompiledDspProgram) -> Result<(), AudioError> {
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
    Ok(())
}

fn offline_error(program: &CompiledDspProgram, error: KernelError<AudioError>) -> AudioError {
    match error {
        KernelError::Source(error) => error,
        KernelError::FrameIndexOverflow => AudioError::InvalidFrameCount,
        KernelError::NonFiniteTrack { track } => AudioError::InvalidValue {
            entity_id: program.tracks[track].diagnostic_id.clone(),
            field: "samples",
        },
        KernelError::NonFiniteMaster => AudioError::InvalidValue {
            entity_id: "master".to_owned(),
            field: "samples",
        },
    }
}
