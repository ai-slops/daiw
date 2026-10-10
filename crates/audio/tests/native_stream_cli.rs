#![cfg(feature = "native")]

use std::process::Command;

use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

const EMPTY_SHA256: &str = "78bb7e4b50b6f3cbe680cf899e249a94db38535e46f28d5538d4df408eacb00b";

fn player_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daiw-play"))
}

fn canonical_wav(samples: &[[i16; 2]]) -> Vec<u8> {
    let payload_bytes = must(u32::try_from(samples.len() * 4));
    let mut bytes = Vec::with_capacity(44 + samples.len() * 4);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(payload_bytes + 36).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&48_000_u32.to_le_bytes());
    bytes.extend_from_slice(&192_000_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&payload_bytes.to_le_bytes());
    for frame in samples {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    bytes
}

fn fixture(label: &str, samples: &[[i16; 2]]) -> (TempDir, std::path::PathBuf, String) {
    let temp = must(TempDir::new(label));
    let path = temp.path().join("stream.wav");
    let bytes = canonical_wav(samples);
    let digest = format!("{:x}", Sha256::digest(&bytes));
    must(std::fs::write(&path, bytes));
    (temp, path, digest)
}

#[test]
fn help_describes_stream_validation_without_opening_a_device() {
    // Given
    let mut command = player_command();
    command.arg("--help");

    // When
    let output = must(command.output());

    // Then
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--stream-wav"));
    assert!(stdout.contains("--validate-only"));
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_only_prints_descriptor_and_never_requires_an_audio_device() {
    // Given
    let (_temp, path, digest) = fixture("native-cli-valid", &[[1, -1]]);

    // When
    let output = must(
        player_command()
            .arg("--stream-wav")
            .arg(path)
            .args(["--sha256", &digest, "--validate-only"])
            .output(),
    );

    // Then
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let descriptor: Value = must(serde_json::from_slice(&output.stdout));
    assert_eq!(descriptor["frames"], 1);
    assert_eq!(descriptor["sampleRate"], 48_000);
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_only_rejects_hash_mismatch_without_stdout() {
    // Given
    let (_temp, path, _digest) = fixture("native-cli-hash", &[[0, 0]]);

    // When
    let output = must(
        player_command()
            .arg("--stream-wav")
            .arg(path)
            .args(["--sha256", EMPTY_SHA256, "--validate-only"])
            .output(),
    );

    // Then
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not match"));
}

#[test]
fn validate_only_rejects_start_past_empty_wave_without_stdout() {
    // Given
    let (_temp, path, digest) = fixture("native-cli-empty", &[]);

    // When
    let output = must(
        player_command()
            .arg("--stream-wav")
            .arg(path)
            .args(["--sha256", &digest, "--start-frame", "1", "--validate-only"])
            .output(),
    );

    // Then
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("start frame"));
}

#[test]
fn malformed_stream_flags_fail_with_clear_diagnostics() {
    // Given
    let cases: &[&[&str]] = &[
        &["--stream-wav"],
        &["--stream-wav", "/tmp/a.wav", "--unknown"],
        &[
            "--stream-wav",
            "/tmp/a.wav",
            "--sha256",
            EMPTY_SHA256,
            "--sha256",
            EMPTY_SHA256,
        ],
        &[
            "--stream-wav",
            "/tmp/a.wav",
            "--sha256",
            EMPTY_SHA256,
            "--loop-start",
            "0",
        ],
    ];

    for arguments in cases {
        // When
        let output = must(player_command().args(*arguments).output());

        // Then
        assert!(!output.status.success(), "accepted {arguments:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("invalid command line"),
            "unclear error for {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
