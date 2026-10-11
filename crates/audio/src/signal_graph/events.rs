use std::ops::Range;

use crate::render_clock::{RenderClock, checked_frame_index};
use crate::source::SourceMetadataProvider;
use crate::validate::{finite, positive};
use crate::{AudioError, AudioRegion, EffectiveNote, Instrument, Track};

pub(crate) struct PreparedTrack {
    pub(crate) diagnostic_id: String,
    pub(crate) note_range: Range<usize>,
    pub(crate) region_range: Range<usize>,
    pub(crate) filter_coefficient: f64,
    pub(crate) pan_gain: [f64; 2],
}

pub(super) struct EventRanges {
    pub(super) notes: Range<usize>,
    pub(super) regions: Range<usize>,
}

pub(crate) struct PreparedNote {
    pub(crate) instrument: Instrument,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) frames: usize,
    pub(crate) frequency: f64,
    pub(crate) velocity: f64,
}

pub(crate) struct PreparedRegion {
    pub(crate) source_index: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) source_start: u64,
    pub(crate) source_frames: u64,
    pub(crate) source_rate: u32,
    pub(crate) channels: u16,
    pub(crate) gain: f32,
}

pub(crate) fn prepare_note(
    note: &EffectiveNote,
    instrument: Instrument,
    clock: &RenderClock,
) -> Result<Option<PreparedNote>, AudioError> {
    if instrument == Instrument::Sample {
        return Ok(None);
    }
    let start = clock.frame_at(note.start)?;
    if start >= clock.total_frames {
        return Ok(None);
    }
    let frames = clock.frames_for(note.start, note.duration)?;
    let actual_frames = frames.min(clock.total_frames - start);
    if actual_frames == 0 {
        return Ok(None);
    }
    let frequency = match instrument {
        Instrument::Synth | Instrument::Bass | Instrument::Pad => {
            let frequency = 440.0 * 2.0_f64.powf((note.pitch - 69.0) / 12.0);
            positive(&note.id, "frequency", frequency)?;
            let last_time = (actual_frames - 1) as f64 / f64::from(clock.sample_rate);
            finite(&note.id, "phase", frequency * last_time)?;
            frequency
        }
        Instrument::Kick | Instrument::Snare | Instrument::Hat | Instrument::Sample => 0.0,
    };
    Ok(Some(PreparedNote {
        instrument,
        start,
        end: start + actual_frames,
        frames,
        frequency,
        velocity: note.velocity,
    }))
}

pub(crate) fn prepare_region(
    region: &AudioRegion,
    source_index: usize,
    sources: &impl SourceMetadataProvider,
    clock: &RenderClock,
) -> Result<Option<PreparedRegion>, AudioError> {
    let start = clock.frame_at(region.start)?;
    if start >= clock.total_frames {
        return Ok(None);
    }
    let duration = clock
        .frames_for(region.start, region.duration)?
        .min(clock.total_frames - start);
    if duration == 0 {
        return Ok(None);
    }
    let metadata = sources.metadata(&region.asset_id)?;
    let source_start = u64::try_from(checked_frame_index(
        (region.source_offset.max(0.0) * f64::from(metadata.sample_rate)).round(),
    )?)?;
    if source_start >= metadata.frames {
        return Ok(None);
    }
    Ok(Some(PreparedRegion {
        source_index,
        start,
        end: start + duration,
        source_start,
        source_frames: metadata.frames - source_start,
        source_rate: metadata.sample_rate,
        channels: metadata.channels,
        gain: 10.0_f32.powf(region.gain as f32 / 20.0),
    }))
}

pub(crate) fn prepare_track(
    track: &Track,
    events: EventRanges,
    clock: &RenderClock,
) -> Result<PreparedTrack, AudioError> {
    let gain = 10.0_f64.powf(track.gain / 20.0);
    finite(&track.id, "linearGain", gain)?;
    let pan = track.pan.clamp(-1.0, 1.0);
    let left = gain * ((1.0 - pan) * 0.5).sqrt();
    let right = gain * ((1.0 + pan) * 0.5).sqrt();
    finite(&track.id, "panGain", left)?;
    finite(&track.id, "panGain", right)?;
    let maximum = f64::from(clock.sample_rate) * 0.49;
    let bounded = track.cutoff.clamp(20.0_f64.min(maximum), maximum);
    Ok(PreparedTrack {
        diagnostic_id: track.id.clone(),
        note_range: events.notes,
        region_range: events.regions,
        filter_coefficient: 1.0
            - (-std::f64::consts::TAU * bounded / f64::from(clock.sample_rate)).exp(),
        pan_gain: [left, right],
    })
}
