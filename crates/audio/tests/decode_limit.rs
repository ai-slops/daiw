use daiw_audio::{AudioError, RenderedAudio, decode_file, wav};

#[path = "support/both.rs"]
mod support;
use support::{must, must_fail};

#[test]
fn decoder_rejects_pcm_expansion_beyond_interleaved_sample_limit() {
    // Given
    let sample_count = daiw_audio::MAX_DECODED_SAMPLES + 2;
    let rendered = RenderedAudio {
        sample_rate: 48_000,
        channels: 2,
        samples: vec![0.0; sample_count],
    };
    let path = std::env::temp_dir().join(format!("daiw-limit-{}.wav", std::process::id()));
    must(std::fs::write(&path, must(wav(&rendered))));

    // When
    let error = must_fail(decode_file(&path));
    must(std::fs::remove_file(path));

    // Then
    assert!(matches!(
        error,
        AudioError::ResourceLimit {
            limit: daiw_audio::MAX_DECODED_SAMPLES,
            ..
        }
    ));
}
