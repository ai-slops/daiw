use std::ops::Range;

use crate::render_clock::{RenderClock, checked_frame_index};
use crate::source::SourceBank;
use crate::validate::{finite, positive};
use crate::{AudioError, AudioRegion, EffectiveNote, Instrument, Snapshot, Track};

pub(crate) struct RenderPlan<'a> {
    pub tracks: Vec<PreparedTrack<'a>>,
    pub notes: Vec<PreparedNote<'a>>,
    pub regions: Vec<PreparedRegion<'a>>,
}

pub(crate) struct PreparedTrack<'a> {
    pub track: &'a Track,
    pub note_range: Range<usize>,
    pub region_range: Range<usize>,
    pub filter_state: [f64; 2],
    pub filter_coefficient: f64,
    pub pan_gain: [f64; 2],
}

struct TrackEventRanges {
    notes: Range<usize>,
    regions: Range<usize>,
}

pub(crate) struct PreparedNote<'a> {
    pub note: &'a EffectiveNote,
    pub instrument: Instrument,
    pub start: usize,
    pub end: usize,
    pub frames: usize,
    pub frequency: f64,
}

pub(crate) struct PreparedRegion<'a> {
    pub region: &'a AudioRegion,
    pub start: usize,
    pub end: usize,
    pub source_start: u64,
    pub source_frames: u64,
    pub source_rate: u32,
    pub channels: u16,
    pub gain: f32,
}

pub(crate) fn prepare<'a>(
    snapshot: &'a Snapshot,
    sources: &impl SourceBank,
    clock: &RenderClock,
) -> Result<RenderPlan<'a>, AudioError> {
    let mut tracks = Vec::new();
    let mut notes = Vec::new();
    let mut regions = Vec::new();
    tracks.try_reserve(snapshot.tracks.len())?;
    notes.try_reserve(snapshot.notes.len())?;
    regions.try_reserve(snapshot.regions.len())?;
    if clock.total_frames == 0 {
        return Ok(RenderPlan {
            tracks,
            notes,
            regions,
        });
    }
    let soloed = snapshot.tracks.iter().any(|track| track.solo);
    for track in &snapshot.tracks {
        if track.mute || (soloed && !track.solo) {
            continue;
        }
        let note_start = notes.len();
        for note in snapshot
            .notes
            .iter()
            .filter(|note| note.track_id == track.id)
        {
            if let Some(note) = prepare_note(note, track.instrument, clock)? {
                notes.push(note);
            }
        }
        let region_start = regions.len();
        for region in snapshot
            .regions
            .iter()
            .filter(|region| region.track_id == track.id)
        {
            if let Some(region) = prepare_region(region, sources, clock)? {
                regions.push(region);
            }
        }
        tracks.push(prepare_track(
            track,
            TrackEventRanges {
                notes: note_start..notes.len(),
                regions: region_start..regions.len(),
            },
            clock.sample_rate,
        )?);
    }
    Ok(RenderPlan {
        tracks,
        notes,
        regions,
    })
}

fn prepare_note<'a>(
    note: &'a EffectiveNote,
    instrument: Instrument,
    clock: &RenderClock,
) -> Result<Option<PreparedNote<'a>>, AudioError> {
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
        note,
        instrument,
        start,
        end: start + actual_frames,
        frames,
        frequency,
    }))
}

fn prepare_region<'a>(
    region: &'a AudioRegion,
    sources: &impl SourceBank,
    clock: &RenderClock,
) -> Result<Option<PreparedRegion<'a>>, AudioError> {
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
        region,
        start,
        end: start + duration,
        source_start,
        source_frames: metadata.frames - source_start,
        source_rate: metadata.sample_rate,
        channels: metadata.channels,
        gain: 10.0_f32.powf(region.gain as f32 / 20.0),
    }))
}

fn prepare_track<'a>(
    track: &'a Track,
    events: TrackEventRanges,
    sample_rate: u32,
) -> Result<PreparedTrack<'a>, AudioError> {
    let gain = 10.0_f64.powf(track.gain / 20.0);
    finite(&track.id, "linearGain", gain)?;
    let pan = track.pan.clamp(-1.0, 1.0);
    let left = gain * ((1.0 - pan) * 0.5).sqrt();
    let right = gain * ((1.0 + pan) * 0.5).sqrt();
    finite(&track.id, "panGain", left)?;
    finite(&track.id, "panGain", right)?;
    let maximum = f64::from(sample_rate) * 0.49;
    let bounded = track.cutoff.clamp(20.0_f64.min(maximum), maximum);
    Ok(PreparedTrack {
        track,
        note_range: events.notes,
        region_range: events.regions,
        filter_state: [0.0; 2],
        filter_coefficient: 1.0 - (-std::f64::consts::TAU * bounded / f64::from(sample_rate)).exp(),
        pan_gain: [left, right],
    })
}
