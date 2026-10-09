use std::process::Command;

use serde::Deserialize;

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    format: String,
    frames: u64,
    sample_rate: u32,
    channels: u16,
    byte_length: u64,
    sha256: String,
    peaks: Vec<f32>,
}

fn wav(sample_rate: u32, channels: u16, frames: usize) -> Vec<u8> {
    let sample_count = frames * usize::from(channels);
    let payload_len = must(u32::try_from(sample_count * 2));
    let mut bytes = Vec::with_capacity(44 + payload_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + payload_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    for index in 0..sample_count {
        let sample = must(i16::try_from(index % 101)) - 50;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[test]
fn decode_file_writes_fixed_header_and_bounded_descriptor() {
    // Given
    let temp = must(TempDir::new("decode-file"));
    let input = temp.path().join("input.wav");
    let output = temp.path().join("cache.pcm");
    must(std::fs::write(&input, wav(8_000, 2, 4)));

    // When
    let result = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output(),
    );

    // Then
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.len() <= 16 * 1024);
    let descriptor: Descriptor = must(serde_json::from_slice(&result.stdout));
    assert_eq!(descriptor.format, "daiw-f32le-v1");
    assert_eq!(
        (
            descriptor.frames,
            descriptor.sample_rate,
            descriptor.channels
        ),
        (4, 8_000, 2)
    );
    assert_eq!(descriptor.byte_length, 80 + 4 * 2 * 4);
    assert_eq!(descriptor.sha256.len(), 64);
    assert_eq!(descriptor.peaks.len(), 128);
    let bytes = must(std::fs::read(output));
    assert_eq!(&bytes[0..8], b"DAIWPCM\0");
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1);
    assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 80);
    assert_eq!(&bytes[68..80], &[0; 12]);
}

#[test]
fn decode_file_rejects_existing_output_without_overwriting_it() {
    // Given
    let temp = must(TempDir::new("create-new"));
    let input = temp.path().join("input.wav");
    let output = temp.path().join("cache.pcm");
    must(std::fs::write(&input, wav(8_000, 1, 2)));
    must(std::fs::write(&output, b"keep"));

    // When
    let result = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output(),
    );

    // Then
    assert!(!result.status.success());
    assert_eq!(must(std::fs::read(output)), b"keep");
}

#[test]
fn decode_file_streams_more_than_legacy_sample_cap_with_bounded_stdout() {
    // Given
    let temp = must(TempDir::new("large-decode"));
    let input = temp.path().join("large.wav");
    let output = temp.path().join("large.pcm");
    let frames = 4_000_001;
    must(std::fs::write(&input, wav(48_000, 1, frames)));

    // When
    let result = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output(),
    );

    // Then
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.len() <= 16 * 1024);
    let descriptor: Descriptor = must(serde_json::from_slice(&result.stdout));
    assert_eq!(descriptor.frames, frames as u64);
}

#[test]
fn decode_file_rejects_sample_rate_before_writing_converted_pcm() {
    // Given
    let temp = must(TempDir::new("invalid-rate"));
    let input = temp.path().join("input.wav");
    let output = temp.path().join("cache.pcm");
    must(std::fs::write(&input, wav(7_999, 1, 2)));

    // When
    let result = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output(),
    );

    // Then
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("8000..=192000"));
    assert!(!output.exists());
}

#[test]
fn decode_file_duration_limit_exits_with_resource_code() {
    // Given
    let temp = must(TempDir::new("duration-limit"));
    let input = temp.path().join("input.wav");
    let output = temp.path().join("cache.pcm");
    must(std::fs::write(&input, wav(8_000, 1, 1_440_001)));

    // When
    let result = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output(),
    );

    // Then
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("180 seconds"));
    assert!(!output.exists());
}
