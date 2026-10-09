use std::collections::BTreeMap;

use daiw_audio::{
    AudioRegion, EffectiveNote, Instrument, Origin, PcmAsset, RenderRequest, Snapshot, Track,
    render, wav,
};
use sha2::{Digest, Sha256};

#[path = "support/must.rs"]
mod support;
use support::must;

const FRAME_RATE: f64 = 48_000.0;

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

fn note(id: &str, track_id: &str, start_frame: usize, frames: usize, pitch: f64) -> EffectiveNote {
    EffectiveNote {
        id: id.into(),
        start: start_frame as f64 / FRAME_RATE,
        duration: frames as f64 / FRAME_RATE,
        pitch,
        velocity: 0.73,
        instance_id: format!("instance-{id}"),
        track_id: track_id.into(),
        origin: Origin {
            definition_id: "definition".into(),
            note_id: id.into(),
            modifier_ids: vec![],
        },
    }
}

fn legacy_boundary_request() -> RenderRequest {
    let snapshot = Snapshot {
        revision: "legacy-block-golden-v1".into(),
        tempo: 60.0,
        tempo_map: None,
        length: 4097.0 / FRAME_RATE,
        tracks: vec![
            track("noise", Instrument::Snare, -2.75, -0.37, 7_321.0),
            track("pad", Instrument::Pad, 1.25, 0.41, 3_219.0),
            track("regions", Instrument::Sample, -4.5, 0.18, 11_137.0),
        ],
        notes: vec![
            note("noise-bed", "noise", 0, 4097, 12.0),
            note("pad-bed", "pad", 1, 4096, 61.25),
            note("pad-overlap", "pad", 4094, 8, 72.5),
            note("noise-overlap", "noise", 4095, 6, 90.0),
        ],
        regions: vec![AudioRegion {
            id: "fractional-region".into(),
            track_id: "regions".into(),
            asset_id: "stereo-source".into(),
            start: 4093.0 / FRAME_RATE,
            duration: 8.0 / FRAME_RATE,
            gain: -1.75,
            source_offset: 1.5 / 32_000.0,
        }],
    };
    let source: Vec<f32> = (0..64)
        .flat_map(|frame| {
            let value = (frame as f32 - 16.0) / 32.0;
            [value, -value * 0.625]
        })
        .collect();
    RenderRequest {
        snapshot,
        assets: BTreeMap::from([(
            "stereo-source".into(),
            PcmAsset {
                sample_rate: 32_000,
                channels: 2,
                samples: source,
            },
        )]),
    }
}

#[test]
fn resident_legacy_waveform_across_4096_boundary_has_locked_sha256() {
    // Given
    let request = legacy_boundary_request();

    // When
    let encoded = must(wav(&must(render(&request))));
    let actual = format!("{:x}", Sha256::digest(&encoded));

    // Then
    assert_eq!(
        actual,
        "2a5be862a67a18c711b59d12329955ccfa442d0e39780be5287f9acd8b7cc9aa"
    );
}
