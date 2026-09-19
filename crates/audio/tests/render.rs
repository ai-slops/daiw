use std::collections::BTreeMap;

use daiw_audio::{
    AudioError, AudioRegion, DecodedAsset, EffectiveNote, Instrument, Origin, PcmAsset,
    RenderRequest, RenderedAudio, Snapshot, Track, render, wav,
};

#[path = "support/both.rs"]
mod support;
use support::{must, must_fail};

fn track(id: &str, instrument: Instrument) -> Track {
    Track {
        id: id.into(),
        name: id.into(),
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

fn note(track_id: &str) -> EffectiveNote {
    EffectiveNote {
        id: "note-1".into(),
        start: 0.5,
        duration: 0.5,
        pitch: 69.0,
        velocity: 1.0,
        instance_id: "instance-1".into(),
        track_id: track_id.into(),
        origin: Origin {
            definition_id: "definition-1".into(),
            note_id: "source-note-1".into(),
            modifier_ids: Vec::new(),
        },
    }
}

#[test]
fn renderer_is_deterministic_and_non_silent_when_snapshot_has_note() {
    // Given
    let snapshot = Snapshot {
        revision: "r1".into(),
        tempo: 120.0,
        length: 2.0,
        tracks: vec![track("synth", Instrument::Synth)],
        notes: vec![note("synth")],
        regions: Vec::new(),
    };
    let request = RenderRequest::new(snapshot);

    // When
    let first = must(render(&request));
    let second = must(render(&request));

    // Then
    assert_eq!(first.samples, second.samples);
    assert!(first.samples.iter().any(|sample| sample.abs() > 0.001));
}

#[test]
fn renderer_places_sample_region_at_exact_frame_with_source_offset() {
    // Given
    let region = AudioRegion {
        id: "region-1".into(),
        track_id: "sample".into(),
        asset_id: "asset-1".into(),
        start: 1.0,
        duration: 1.0,
        gain: 0.0,
        source_offset: 0.25,
    };
    let snapshot = Snapshot {
        revision: "r1".into(),
        tempo: 60.0,
        length: 3.0,
        tracks: vec![track("sample", Instrument::Sample)],
        notes: Vec::new(),
        regions: vec![region],
    };
    let mut assets = BTreeMap::new();
    assets.insert(
        "asset-1".into(),
        PcmAsset {
            sample_rate: 4,
            channels: 1,
            samples: vec![0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25],
        },
    );
    let request = RenderRequest { snapshot, assets };

    // When
    let rendered = must(daiw_audio::render_with_sample_rate(&request, 4));

    // Then
    assert!(rendered.samples[..8].iter().all(|sample| *sample == 0.0));
    assert!(rendered.samples[8] > 0.1);
    assert_eq!(rendered.samples[8], rendered.samples[9]);
    assert!(rendered.samples[10] > rendered.samples[8]);
    assert!(rendered.samples[12] > rendered.samples[10]);
}

#[test]
fn zero_db_stereo_region_preserves_channel_difference() {
    // Given
    let snapshot = Snapshot {
        revision: "r1".into(),
        tempo: 60.0,
        length: 1.0,
        tracks: vec![track("sample", Instrument::Sample)],
        notes: Vec::new(),
        regions: vec![AudioRegion {
            id: "region-1".into(),
            track_id: "sample".into(),
            asset_id: "asset-1".into(),
            start: 0.0,
            duration: 1.0,
            gain: 0.0,
            source_offset: 0.0,
        }],
    };
    let mut assets = BTreeMap::new();
    assets.insert(
        "asset-1".into(),
        PcmAsset {
            sample_rate: 4,
            channels: 2,
            samples: vec![1.0, 0.25, 0.75, 0.2, 0.5, 0.15, 0.25, 0.1],
        },
    );

    // When
    let rendered = must(daiw_audio::render_with_sample_rate(
        &RenderRequest { snapshot, assets },
        4,
    ));

    // Then
    assert!(rendered.samples[0] > rendered.samples[1] * 3.0);
    assert!(rendered.samples[0] > 0.5);
}

#[test]
fn missing_region_asset_is_an_explicit_error() {
    // Given
    let snapshot = Snapshot {
        revision: "r1".into(),
        tempo: 120.0,
        length: 1.0,
        tracks: vec![track("sample", Instrument::Sample)],
        notes: Vec::new(),
        regions: vec![AudioRegion {
            id: "region-1".into(),
            track_id: "sample".into(),
            asset_id: "missing".into(),
            start: 0.0,
            duration: 1.0,
            gain: 0.0,
            source_offset: 0.0,
        }],
    };

    // When
    let error = must_fail(render(&RenderRequest::new(snapshot)));

    // Then
    assert!(matches!(error, AudioError::MissingAsset { .. }));
}

#[test]
fn note_with_unknown_track_is_an_explicit_error() {
    // Given
    let snapshot = Snapshot {
        revision: "r1".into(),
        tempo: 120.0,
        length: 1.0,
        tracks: vec![track("synth", Instrument::Synth)],
        notes: vec![note("missing")],
        regions: Vec::new(),
    };

    // When
    let error = must_fail(render(&RenderRequest::new(snapshot)));

    // Then
    assert!(matches!(error, AudioError::UnknownTrack { .. }));
}

#[test]
fn wav_encoder_writes_stereo_pcm_header_and_payload() {
    // Given
    let rendered = daiw_audio::RenderedAudio {
        sample_rate: 48_000,
        channels: 2,
        samples: vec![-1.0, 1.0, 0.0, 0.5],
    };

    // When
    let bytes = must(wav(&rendered));

    // Then
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(bytes.len(), 52);
}

#[test]
fn decode_cli_returns_interleaved_pcm_and_metadata() {
    // Given
    let rendered = RenderedAudio {
        sample_rate: 8_000,
        channels: 2,
        samples: vec![-0.5, 0.5, 0.25, -0.25],
    };
    let path = std::env::temp_dir().join(format!("daiw-decode-{}.wav", std::process::id()));
    must(std::fs::write(&path, must(wav(&rendered))));

    // When
    let output = must(
        std::process::Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .arg("--decode")
            .arg(&path)
            .output(),
    );
    must(std::fs::remove_file(path));
    let decoded: DecodedAsset = must(serde_json::from_slice(&output.stdout));

    // Then
    assert!(output.status.success());
    assert_eq!(decoded.frames, 2);
    assert_eq!(decoded.sample_rate, 8_000);
    assert_eq!(decoded.channels, 2);
    assert_eq!(decoded.samples.len(), 4);
    assert!(decoded.samples[0] < -0.49);
}
