use daiw_audio::{
    AudioError, EffectiveNote, Instrument, Origin, RenderRequest, Snapshot, Track, render,
};

#[path = "support/both.rs"]
mod support;
use support::{must, must_fail};

fn request() -> RenderRequest {
    RenderRequest::new(Snapshot {
        revision: "note-validation".into(),
        tempo: 120.0,
        length: 1.0,
        tracks: vec![Track {
            id: "synth".into(),
            name: "Synth".into(),
            role: "music".into(),
            color: "#fff".into(),
            instrument: Instrument::Synth,
            gain: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            cutoff: 20_000.0,
        }],
        notes: vec![EffectiveNote {
            id: "effective-note".into(),
            start: 0.0,
            duration: 1.0,
            pitch: 60.25,
            velocity: 0.8,
            instance_id: "instance".into(),
            track_id: "synth".into(),
            origin: Origin {
                definition_id: "definition".into(),
                note_id: "source-note".into(),
                modifier_ids: vec![],
            },
        }],
        regions: vec![],
    })
}

#[test]
fn standalone_renderer_rejects_duplicate_effective_ids() {
    let mut input = request();
    input.snapshot.notes.push(input.snapshot.notes[0].clone());
    let error = must_fail(render(&input));
    assert!(matches!(
        error,
        AudioError::InvalidValue { field: "id", .. }
    ));
}

#[test]
fn standalone_renderer_rejects_nan_and_infinity_without_normalization() {
    for pitch in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut input = request();
        input.snapshot.notes[0].pitch = pitch;
        assert!(matches!(
            must_fail(render(&input)),
            AudioError::InvalidValue { field: "pitch", .. }
        ));
    }
}

#[test]
fn ordinary_fractional_pitch_renders_finite_non_silent_audio() {
    let rendered = must(render(&request()));
    assert!(rendered.samples.iter().all(|sample| sample.is_finite()));
    assert!(rendered.samples.iter().any(|sample| sample.abs() > 0.001));
}
