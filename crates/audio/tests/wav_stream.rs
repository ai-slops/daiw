use std::path::Path;

use daiw_audio::{
    FrameLoopRange, FramePosition, MAX_WAV_STREAM_FRAMES, Sha256Digest, WavFileDescriptor,
    WavStreamError, WavStreamOptions, WavStreamState, validate_wav_file,
};
use sha2::{Digest, Sha256};

#[path = "support/must_fail.rs"]
mod must_fail;
#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use must_fail::must_fail;
use support::must;
use temp_dir::TempDir;

fn wav_bytes(samples: &[[i16; 2]]) -> Vec<u8> {
    let payload_len = must(u32::try_from(samples.len() * 4));
    let mut bytes = Vec::with_capacity(44 + samples.len() * 4);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + payload_len).to_le_bytes());
    bytes.extend_from_slice(
        b"WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data",
    );
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    for frame in samples {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    bytes
}

fn fixture(label: &str, samples: &[[i16; 2]]) -> (TempDir, std::path::PathBuf, WavFileDescriptor) {
    let temp = must(TempDir::new(label));
    let path = temp.path().join("input.wav");
    let bytes = wav_bytes(samples);
    must(std::fs::write(&path, &bytes));
    let descriptor = WavFileDescriptor {
        format: "daiw-wav-pcm16-v1",
        sample_rate: 48_000,
        channels: 2,
        frames: samples.len(),
        byte_length: must(u64::try_from(bytes.len())),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    (temp, path, descriptor)
}

#[test]
fn digest_and_frame_types_reject_invalid_boundaries() {
    // Given / When / Then
    assert!(Sha256Digest::parse(&"a".repeat(64)).is_ok());
    assert!(Sha256Digest::parse(&"A".repeat(64)).is_err());
    assert!(Sha256Digest::parse("abc").is_err());
    assert!(FrameLoopRange::new(FramePosition::new(0), FramePosition::new(1), 1).is_ok());
    assert!(FrameLoopRange::new(FramePosition::new(1), FramePosition::new(1), 2).is_err());
    assert!(FrameLoopRange::new(FramePosition::new(0), FramePosition::new(3), 2).is_err());
}

#[test]
fn validator_rejects_relative_paths_and_noncanonical_files() {
    // Given
    let (_temp, path, descriptor) = fixture("wav-stream-invalid", &[[1, 2]]);
    let mut trailing = must(std::fs::read(&path));
    trailing.push(0);
    must(std::fs::write(&path, trailing));

    // When
    let relative = validate_wav_file(Path::new("relative.wav"), &descriptor);
    let malformed = validate_wav_file(&path, &descriptor);

    // Then
    assert!(relative.is_err());
    assert!(malformed.is_err());
}

#[test]
fn validated_open_uses_same_file_and_callback_converts_pcm16() {
    // Given
    let (temp, path, descriptor) = fixture(
        "wav-stream-open",
        &[[i16::MIN, i16::MAX], [0, 16_384], [-16_384, 0]],
    );
    let file = must(validate_wav_file(&path, &descriptor));
    let renamed = temp.path().join("renamed.wav");
    must(std::fs::rename(&path, &renamed));
    let (mut control, mut callback) = must(file.open(WavStreamOptions::default()));
    let mut paused = [1.0_f32; 2];
    callback.fill_output(&mut paused);
    must(control.play());
    let mut output = [0.0_f32; 6];

    // When
    callback.fill_output(&mut output);

    // Then
    assert_eq!(paused, [0.0; 2]);
    assert_eq!(output, [-1.0, 32_767.0 / 32_768.0, 0.0, 0.5, -0.5, 0.0]);
    assert_eq!(control.position(), FramePosition::new(3));
    assert_eq!(control.state(), WavStreamState::Ended);
    must(control.shutdown());
}

#[test]
fn loop_wraps_inside_one_variable_sized_callback() {
    // Given
    let (_temp, path, descriptor) = fixture(
        "wav-stream-loop",
        &[[1_000, 2_000], [3_000, 4_000], [5_000, 6_000]],
    );
    let loop_range = must(FrameLoopRange::new(
        FramePosition::new(1),
        FramePosition::new(3),
        3,
    ));
    let options = WavStreamOptions {
        start_frame: FramePosition::new(1),
        loop_range: Some(loop_range),
    };
    let (mut control, mut callback) =
        must(must(validate_wav_file(&path, &descriptor)).open(options));
    must(control.play());
    let mut output = [0.0_f32; 10];

    // When
    callback.fill_output(&mut output);

    // Then
    let scale = 32_768.0;
    assert_eq!(
        output,
        [
            3_000.0 / scale,
            4_000.0 / scale,
            5_000.0 / scale,
            6_000.0 / scale,
            3_000.0 / scale,
            4_000.0 / scale,
            5_000.0 / scale,
            6_000.0 / scale,
            3_000.0 / scale,
            4_000.0 / scale,
        ]
    );
    assert_eq!(control.position(), FramePosition::new(2));
    must(control.shutdown());
}

#[test]
fn descriptor_digest_must_be_lowercase_and_exact() {
    // Given
    let (_temp, path, mut descriptor) = fixture("wav-stream-digest", &[[0, 0]]);
    descriptor.sha256.make_ascii_uppercase();

    // When
    let error = must_fail(validate_wav_file(&path, &descriptor));

    // Then
    assert!(error.to_string().contains("SHA-256"));
}

#[test]
fn parsed_header_over_frame_limit_is_rejected_before_hashing_sparse_payload() {
    // Given
    let temp = must(TempDir::new("wav-stream-frame-limit"));
    let path = temp.path().join("oversized.wav");
    let frames = MAX_WAV_STREAM_FRAMES + 1;
    let payload_len = must(frames.checked_mul(4).ok_or(WavStreamError::TooManyFrames));
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + payload_len).to_le_bytes());
    header.extend_from_slice(
        b"WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data",
    );
    header.extend_from_slice(&payload_len.to_le_bytes());
    must(std::fs::write(&path, header));
    let byte_length = 44_u64 + u64::from(payload_len);
    must(must(std::fs::OpenOptions::new().write(true).open(&path)).set_len(byte_length));
    let descriptor = WavFileDescriptor {
        format: "daiw-wav-pcm16-v1",
        sample_rate: 48_000,
        channels: 2,
        frames: must(usize::try_from(frames)),
        byte_length,
        sha256: "0".repeat(64),
    };

    // When
    let error = must_fail(validate_wav_file(&path, &descriptor));

    // Then
    assert!(matches!(error, WavStreamError::TooManyFrames));
}
