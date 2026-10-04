use daiw_audio::{
    AudioError, AudioRegion, Instrument, PcmAsset, RenderedAudio, render_with_sample_rate, wav,
};

#[path = "support/numeric_fixture.rs"]
mod fixture;
#[path = "support/both.rs"]
mod support;
use fixture::request;
use support::{must, must_fail};

fn pcm_request() -> daiw_audio::RenderRequest {
    let mut input = request();
    input.snapshot.notes.clear();
    input.snapshot.tempo = 60.0;
    input.snapshot.tracks[0].instrument = Instrument::Sample;
    input.snapshot.regions.push(AudioRegion {
        id: "region".into(),
        track_id: "tone".into(),
        asset_id: "pcm".into(),
        start: 0.0,
        duration: 1.0,
        gain: 0.0,
        source_offset: 0.0,
    });
    input.assets.insert(
        "pcm".into(),
        PcmAsset {
            sample_rate: 4,
            channels: 1,
            samples: vec![f32::MAX; 4],
        },
    );
    input
}

#[test]
fn region_product_overflow_is_rejected_before_master_mix() {
    let mut input = pcm_request();
    input.snapshot.regions[0].gain = 24.0;
    assert!(matches!(
        must_fail(render_with_sample_rate(&input, 4)),
        AudioError::InvalidValue {
            field: "samples",
            ..
        }
    ));
}

#[test]
fn finite_tracks_cannot_overflow_the_master_accumulator() {
    let mut input = pcm_request();
    let one = must(render_with_sample_rate(&input, 4));
    assert!(one.samples.iter().all(|sample| sample.is_finite()));
    let mut second = input.snapshot.tracks[0].clone();
    second.id = "second".into();
    input.snapshot.tracks.push(second);
    let mut region = input.snapshot.regions[0].clone();
    region.id = "second-region".into();
    region.track_id = "second".into();
    input.snapshot.regions.push(region);
    assert!(matches!(
        must_fail(render_with_sample_rate(&input, 4)),
        AudioError::InvalidValue {
            field: "samples",
            ..
        }
    ));
}

#[test]
fn source_end_is_silence_but_unrepresentable_offset_is_an_error() {
    let mut input = pcm_request();
    input.snapshot.regions[0].source_offset = 100.0;
    assert!(
        must(render_with_sample_rate(&input, 4))
            .samples
            .iter()
            .all(|sample| *sample == 0.0)
    );
    input.snapshot.regions[0].source_offset = f64::MAX;
    assert!(render_with_sample_rate(&input, 4).is_err());
}

#[test]
fn wav_rejects_nonfinite_samples_instead_of_silence_or_rails() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let audio = RenderedAudio {
            sample_rate: 48_000,
            channels: 2,
            samples: vec![value, 0.0],
        };
        assert!(matches!(
            must_fail(wav(&audio)),
            AudioError::InvalidValue {
                field: "samples",
                ..
            }
        ));
    }
}

#[test]
fn wav_keeps_existing_finite_clipping_and_quantization() {
    let audio = RenderedAudio {
        sample_rate: 48_000,
        channels: 2,
        samples: vec![1.5, -1.5, f32::MAX, -f32::MAX],
    };
    let encoded = must(wav(&audio));
    let pcm: Vec<_> = encoded[44..]
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    assert_eq!(pcm, vec![i16::MAX, -i16::MAX, i16::MAX, -i16::MAX]);
}

#[test]
fn half_frame_placement_and_source_offset_keep_rounding_up() {
    let mut input = pcm_request();
    input.assets.insert(
        "pcm".into(),
        PcmAsset {
            sample_rate: 4,
            channels: 1,
            samples: vec![0.0, 0.25, 0.5, 0.75],
        },
    );
    input.snapshot.regions[0].start = 0.125;
    input.snapshot.regions[0].source_offset = 0.125;
    input.snapshot.regions[0].duration = 0.5;
    let output = must(render_with_sample_rate(&input, 4));
    assert_eq!(output.samples[0], 0.0);
    assert!(output.samples[2] > 0.1);
    assert!(output.samples[4] > output.samples[2]);
}
