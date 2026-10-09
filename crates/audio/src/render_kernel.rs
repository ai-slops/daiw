use std::f64::consts::TAU;

use crate::render_clock::RenderClock;
use crate::render_plan::{PreparedNote, PreparedRegion, prepare};
use crate::source::SourceBank;
use crate::validate::finite_samples;
use crate::{AudioError, Instrument, Snapshot};

pub(crate) trait BlockSink {
    fn write_block(&mut self, start_frame: usize, frames: &[[f32; 2]]) -> Result<(), AudioError>;
}

pub(crate) struct Collector {
    samples: Vec<f32>,
}

pub(crate) struct RenderContext<'a> {
    pub snapshot: &'a Snapshot,
    pub clock: &'a RenderClock,
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
    context: RenderContext<'_>,
    sources: &mut impl SourceBank,
    sink: &mut impl BlockSink,
) -> Result<(), AudioError> {
    const { assert!(BLOCK_FRAMES > 0, "render block must contain frames") };
    let mut plan = prepare(context.snapshot, sources, context.clock)?;
    let mut master = [[0.0_f32; 2]; BLOCK_FRAMES];
    let mut scratch = [[0.0_f32; 2]; BLOCK_FRAMES];
    let mut source_reader = SourceReader {
        output_rate: context.clock.sample_rate,
        sources,
        frame: [0.0; 8],
    };
    let mut block_start = 0;
    while block_start < context.clock.total_frames {
        let block_frames = BLOCK_FRAMES.min(context.clock.total_frames - block_start);
        let block_end = block_start + block_frames;
        master[..block_frames].fill([0.0; 2]);
        for track in &mut plan.tracks {
            scratch[..block_frames].fill([0.0; 2]);
            let mut block = BlockScratch {
                frames: &mut scratch[..block_frames],
                start: block_start,
            };
            for note in &plan.notes[track.note_range.clone()] {
                accumulate_note(&mut block, note, context.clock.sample_rate);
            }
            for region in &plan.regions[track.region_range.clone()] {
                source_reader.accumulate_region(&mut block, region)?;
            }
            for frame in &mut *block.frames {
                for (channel, sample) in frame.iter_mut().enumerate() {
                    track.filter_state[channel] += track.filter_coefficient
                        * (f64::from(*sample) - track.filter_state[channel]);
                    *sample = track.filter_state[channel] as f32;
                }
            }
            let flat = scratch[..block_frames].as_flattened();
            finite_samples(flat, &track.track.id)?;
            for (target, source) in master[..block_frames]
                .iter_mut()
                .zip(&scratch[..block_frames])
            {
                target[0] += (f64::from(source[0]) * track.pan_gain[0]) as f32;
                target[1] += (f64::from(source[1]) * track.pan_gain[1]) as f32;
            }
        }
        finite_samples(master[..block_frames].as_flattened(), "master")?;
        sink.write_block(block_start, &master[..block_frames])?;
        block_start = block_end;
    }
    Ok(())
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
