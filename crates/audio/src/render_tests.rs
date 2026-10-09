use std::collections::BTreeMap;

use super::{render_with_sample_rate, render_with_sample_rate_and_block_size};
use crate::render_clock::RenderClock;
use crate::render_kernel::{Collector, RenderContext, render_blocks};
use crate::source::{SourceBank, SourceMetadata};
use crate::{
    AudioError, AudioRegion, EffectiveNote, Instrument, Origin, PcmAsset, RenderRequest, Snapshot,
    Track,
};

struct RecordingSourceBank {
    reads: Vec<u64>,
}

impl SourceBank for RecordingSourceBank {
    fn metadata(&self, _asset_id: &str) -> Result<SourceMetadata, AudioError> {
        Ok(SourceMetadata {
            sample_rate: 48,
            channels: 1,
            frames: 64,
        })
    }

    fn read_frame(
        &mut self,
        _asset_id: &str,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<bool, AudioError> {
        self.reads.push(frame);
        output[0] = frame as f32 / 64.0;
        Ok(true)
    }
}

fn track(id: &str, instrument: Instrument, gain: f64, pan: f64, cutoff: f64) -> Track {
    Track {
        id: id.into(),
        name: id.into(),
        role: "music".into(),
        color: "#fff".into(),
        instrument,
        gain,
        pan,
        mute: false,
        solo: false,
        cutoff,
    }
}

fn note(id: &str, track_id: &str, start: f64, duration: f64, pitch: f64) -> EffectiveNote {
    EffectiveNote {
        id: id.into(),
        start,
        duration,
        pitch,
        velocity: 0.8,
        instance_id: format!("instance-{id}"),
        track_id: track_id.into(),
        origin: Origin {
            definition_id: "definition".into(),
            note_id: id.into(),
            modifier_ids: vec![],
        },
    }
}

fn boundary_request() -> RenderRequest {
    RenderRequest {
        snapshot: Snapshot {
            revision: "block-parity".into(),
            tempo: 60.0,
            tempo_map: None,
            length: 17.0 / 48.0,
            tracks: vec![
                track("pad", Instrument::Pad, -1.5, -0.25, 13.0),
                track("noise", Instrument::Snare, 2.0, 0.4, 9.0),
                track("sample", Instrument::Sample, -3.0, 0.1, 17.0),
            ],
            notes: vec![
                note("pad-a", "pad", 0.0, 17.0 / 48.0, 64.5),
                note("pad-b", "pad", 7.0 / 48.0, 8.0 / 48.0, 71.0),
                note("noise", "noise", 8.0 / 48.0, 7.0 / 48.0, f64::MAX),
            ],
            regions: vec![AudioRegion {
                id: "crossing".into(),
                track_id: "sample".into(),
                asset_id: "pcm".into(),
                start: 5.0 / 48.0,
                duration: 11.0 / 48.0,
                gain: 1.25,
                source_offset: 0.5 / 32.0,
            }],
        },
        assets: BTreeMap::from([(
            "pcm".into(),
            PcmAsset {
                sample_rate: 32,
                channels: 2,
                samples: (0..24)
                    .flat_map(|frame| {
                        let value = frame as f32 / 24.0;
                        [value, -value]
                    })
                    .collect(),
            },
        )]),
    }
}

#[test]
fn shared_kernel_is_bit_identical_for_small_and_production_blocks() -> Result<(), crate::AudioError>
{
    // Given
    let request = boundary_request();
    let legacy = render_with_sample_rate(&request, 48)?;

    // When
    let one = render_with_sample_rate_and_block_size::<1>(&request, 48)?;
    let three = render_with_sample_rate_and_block_size::<3>(&request, 48)?;
    let production = render_with_sample_rate_and_block_size::<4096>(&request, 48)?;

    // Then
    assert_eq!(one.samples, legacy.samples);
    assert_eq!(three.samples, legacy.samples);
    assert_eq!(production.samples, legacy.samples);
    Ok(())
}

#[test]
fn overlapping_regions_read_each_source_range_contiguously_per_block() -> Result<(), AudioError> {
    // Given
    let mut snapshot = boundary_request().snapshot;
    snapshot.length = 4.0 / 48.0;
    snapshot.tracks.retain(|track| track.id == "sample");
    snapshot.notes.clear();
    snapshot.regions = vec![
        AudioRegion {
            id: "first".into(),
            track_id: "sample".into(),
            asset_id: "pcm".into(),
            start: 0.0,
            duration: 4.0 / 48.0,
            gain: 0.0,
            source_offset: 0.0,
        },
        AudioRegion {
            id: "second".into(),
            track_id: "sample".into(),
            asset_id: "pcm".into(),
            start: 0.0,
            duration: 4.0 / 48.0,
            gain: 0.0,
            source_offset: 8.0 / 48.0,
        },
    ];
    let clock = RenderClock::new(&snapshot, 48)?;
    let mut sources = RecordingSourceBank { reads: vec![] };
    let mut collector = Collector::new(clock.total_frames)?;

    // When
    render_blocks::<4>(
        RenderContext {
            snapshot: &snapshot,
            clock: &clock,
        },
        &mut sources,
        &mut collector,
    )?;

    // Then
    assert_eq!(sources.reads, vec![0, 1, 2, 3, 8, 9, 10, 11]);
    Ok(())
}
