use std::collections::BTreeMap;

use daiw_audio::{
    AudioError, AudioRegion, Instrument, PcmAsset, RenderRequest, Snapshot, Track, render,
};

#[path = "support/must_fail.rs"]
mod support;
use support::must_fail;

fn request(region: AudioRegion) -> RenderRequest {
    let snapshot = Snapshot {
        revision: "r1".into(),
        tempo: 120.0,
        length: 2.0,
        tracks: vec![Track {
            id: "sample".into(),
            name: "sample".into(),
            role: "music".into(),
            color: "#fff".into(),
            instrument: Instrument::Sample,
            gain: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            cutoff: 20_000.0,
        }],
        notes: Vec::new(),
        regions: vec![region],
    };
    let mut assets = BTreeMap::new();
    assets.insert(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![0.0],
        },
    );
    RenderRequest { snapshot, assets }
}

fn region(gain: f64, duration: f64) -> AudioRegion {
    AudioRegion {
        id: "region".into(),
        track_id: "sample".into(),
        asset_id: "asset".into(),
        start: 0.0,
        duration,
        gain,
        source_offset: 0.0,
    }
}

#[test]
fn region_gain_outside_core_db_range_is_rejected() {
    // Given
    let input = request(region(24.1, 1.0));

    // When
    let error = must_fail(render(&input));

    // Then
    assert!(matches!(
        error,
        AudioError::InvalidValue { field: "gain", .. }
    ));
}

#[test]
fn zero_length_region_is_rejected() {
    // Given
    let input = request(region(0.0, 0.0));

    // When
    let error = must_fail(render(&input));

    // Then
    assert!(matches!(
        error,
        AudioError::InvalidValue {
            field: "duration",
            ..
        }
    ));
}
