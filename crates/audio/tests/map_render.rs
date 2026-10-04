use std::collections::BTreeMap;

use daiw_audio::{
    AudioRegion, EffectiveNote, Instrument, Origin, PcmAsset, RenderRequest, Snapshot, Track,
    render_with_sample_rate,
};
use daiw_timing::{Curve, TempoMap, TempoPoint, Tick};
use std::fmt::Debug;

fn must<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn beat(value: f64) -> Tick {
    must(Tick::from_beats(value))
}

fn map() -> TempoMap {
    must(TempoMap::new(vec![
        TempoPoint::new(beat(0.0), 60.0, Curve::Step),
        TempoPoint::new(beat(1.0), 120.0, Curve::Step),
    ]))
}

fn track(instrument: Instrument) -> Track {
    Track {
        id: "track".into(),
        name: "Track".into(),
        role: "music".into(),
        color: "#fff".into(),
        instrument,
        gain: 0.0,
        pan: 0.0,
        mute: false,
        solo: false,
        cutoff: 20_000.0,
    }
}

#[test]
fn mapped_note_uses_absolute_start_and_end_boundaries() {
    let snapshot = Snapshot {
        revision: "map-note".into(),
        tempo: 60.0,
        tempo_map: Some(map()),
        length: 3.0,
        tracks: vec![track(Instrument::Synth)],
        notes: vec![EffectiveNote {
            id: "note".into(),
            start: 1.0,
            duration: 1.0,
            pitch: 69.0,
            velocity: 1.0,
            instance_id: "instance".into(),
            track_id: "track".into(),
            origin: Origin {
                definition_id: "definition".into(),
                note_id: "source".into(),
                modifier_ids: vec![],
            },
        }],
        regions: vec![],
    };

    let rendered = must(render_with_sample_rate(&RenderRequest::new(snapshot), 4));
    assert_eq!(rendered.samples.len(), 16);
    assert!(rendered.samples[..10].iter().all(|sample| *sample == 0.0));
    assert!(rendered.samples[10].abs() > 0.001);
}

#[test]
fn mapped_region_duration_uses_end_minus_start_and_offset_stays_seconds() {
    let snapshot = Snapshot {
        revision: "map-region".into(),
        tempo: 60.0,
        tempo_map: Some(map()),
        length: 3.0,
        tracks: vec![track(Instrument::Sample)],
        notes: vec![],
        regions: vec![AudioRegion {
            id: "region".into(),
            track_id: "track".into(),
            asset_id: "asset".into(),
            start: 1.0,
            duration: 1.0,
            gain: 0.0,
            source_offset: 0.25,
        }],
    };
    let assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 4,
            channels: 1,
            samples: vec![1.0, -1.0, -1.0, 1.0, 1.0],
        },
    )]);

    let rendered = must(render_with_sample_rate(
        &RenderRequest { snapshot, assets },
        4,
    ));
    assert!(rendered.samples[..8].iter().all(|sample| *sample == 0.0));
    assert!(rendered.samples[8] < 0.0);
    assert!(rendered.samples[12] < 0.0);
}
