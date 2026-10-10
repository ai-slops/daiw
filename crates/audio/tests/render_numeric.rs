use daiw_audio::{AudioError, Instrument, render};

#[path = "support/numeric_fixture.rs"]
mod fixture;
#[path = "support/both.rs"]
mod support;
use fixture::request;
use support::{must, must_fail};

#[test]
fn finite_pitch_overflow_and_underflow_are_explicit_frequency_errors() {
    for pitch in [f64::MAX, f64::MIN] {
        let mut input = request();
        input.snapshot.notes[0].pitch = pitch;
        assert!(matches!(
            must_fail(render(&input)),
            AudioError::InvalidValue {
                field: "frequency",
                ..
            }
        ));
    }
}

#[test]
fn finite_frequency_with_overflowing_phase_is_rejected() {
    let mut input = request();
    input.snapshot.length = 8.0;
    input.snapshot.notes[0].duration = 8.0;
    input.snapshot.notes[0].pitch = 12240.0;
    let frequency = 440.0 * 2.0_f64.powf((12240.0 - 69.0) / 12.0);
    assert!(frequency.is_finite());
    assert!(!(frequency * 4.0).is_finite());
    assert!(matches!(
        must_fail(render(&input)),
        AudioError::InvalidValue { field: "phase", .. }
    ));
}

#[test]
fn gain_conversion_overflow_is_rejected_but_underflow_remains_silence() {
    let mut input = request();
    input.snapshot.tracks[0].gain = f64::MAX;
    assert!(matches!(
        must_fail(render(&input)),
        AudioError::InvalidValue {
            field: "linearGain",
            ..
        }
    ));
    input.snapshot.tracks[0].gain = f64::MIN;
    assert!(
        must(render(&input))
            .samples
            .iter()
            .all(|sample| *sample == 0.0)
    );
}

#[test]
fn finite_gain_that_overflows_f32_mixing_is_rejected() {
    let mut input = request();
    input.snapshot.tracks[0].gain = 800.0;
    assert!(matches!(
        must_fail(render(&input)),
        AudioError::InvalidValue {
            field: "samples",
            ..
        }
    ));
}

#[test]
fn pitch_independent_and_unrendered_notes_do_not_evaluate_frequency() {
    for instrument in [
        Instrument::Kick,
        Instrument::Snare,
        Instrument::Hat,
        Instrument::Sample,
    ] {
        let mut input = request();
        input.snapshot.tracks[0].instrument = instrument;
        input.snapshot.notes[0].pitch = f64::MAX;
        assert!(
            must(render(&input))
                .samples
                .iter()
                .all(|sample| sample.is_finite())
        );
    }
    let mut muted = request();
    muted.snapshot.notes[0].pitch = f64::MAX;
    muted.snapshot.tracks[0].mute = true;
    assert!(
        must(render(&muted))
            .samples
            .iter()
            .all(|sample| *sample == 0.0)
    );
    let mut outside = request();
    outside.snapshot.notes[0].pitch = f64::MAX;
    outside.snapshot.notes[0].start = 2.0;
    assert!(
        must(render(&outside))
            .samples
            .iter()
            .all(|sample| *sample == 0.0)
    );
    outside.snapshot.notes[0].start = 0.0;
    outside.snapshot.notes[0].duration = 0.0;
    assert!(
        must(render(&outside))
            .samples
            .iter()
            .all(|sample| *sample == 0.0)
    );
}

#[test]
fn unrepresentable_note_frame_conversion_is_rejected() {
    let mut input = request();
    input.snapshot.notes[0].duration = f64::MAX;
    assert!(render(&input).is_err());
}

#[test]
fn output_over_the_frame_budget_is_rejected() {
    let mut input = request();
    input.snapshot.length = 362.0;
    assert!(render(&input).is_err());
}

#[test]
fn reused_track_buffer_preserves_independent_track_summation() {
    let mut input = request();
    let single = must(render(&input));
    let mut second_track = input.snapshot.tracks[0].clone();
    second_track.id = "second".into();
    input.snapshot.tracks.push(second_track);
    let mut second_note = input.snapshot.notes[0].clone();
    second_note.id = "second-note".into();
    second_note.instance_id = "second-instance".into();
    second_note.track_id = "second".into();
    input.snapshot.notes.push(second_note);
    let combined = must(render(&input));
    assert!(
        combined
            .samples
            .iter()
            .zip(&single.samples)
            .all(|(sum, one)| *sum == *one + *one)
    );
}

#[test]
fn empty_render_does_not_evaluate_unused_derived_values() {
    let mut input = request();
    input.snapshot.length = 0.0;
    let mut track = input.snapshot.tracks[0].clone();
    track.gain = f64::MAX;
    track.pan = f64::MAX;
    track.cutoff = f64::MAX;
    input.snapshot.tracks = (0..257)
        .map(|ordinal| {
            let mut active = track.clone();
            active.id = format!("track-{ordinal}");
            active
        })
        .collect();
    input.snapshot.notes[0].track_id = "track-0".into();
    input.snapshot.notes[0].pitch = f64::MAX;
    assert!(must(render(&input)).samples.is_empty());
}
