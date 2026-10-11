#![cfg(feature = "native")]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn fixture(label: &str) -> (TempDir, std::path::PathBuf, String) {
    let temp = must(TempDir::new(label));
    let path = temp.path().join("stream.wav");
    let bytes = b"RIFF(\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data\x04\0\0\0\x01\0\x02\0";
    let digest = format!("{:x}", Sha256::digest(bytes));
    must(std::fs::write(&path, bytes));
    (temp, path, digest)
}

fn worker(path: &std::path::Path, digest: &str) -> std::process::Child {
    must(
        Command::new(env!("CARGO_BIN_EXE_daiw-play"))
            .arg("--control-wav")
            .arg(path)
            .args(["--sha256", digest, "--headless-callback-driver"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn(),
    )
}

fn wait_with_open_stdin(mut child: std::process::Child, input: &[u8]) -> std::process::Output {
    must(must(child.stdin.as_mut().ok_or("missing stdin")).write_all(input));
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = must(child.try_wait()) {
            break status;
        }
        if Instant::now() >= deadline {
            must(child.kill());
            let _ = child.wait();
            panic!("control worker did not exit while stdin remained open");
        }
        std::thread::park_timeout(Duration::from_millis(5));
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    must(must(child.stdout.take().ok_or("missing stdout")).read_to_end(&mut stdout));
    must(must(child.stderr.take().ok_or("missing stderr")).read_to_end(&mut stderr));
    std::process::Output {
        status,
        stdout,
        stderr,
    }
}

#[test]
fn shutdown_exits_while_parent_keeps_stdin_open() {
    // Given
    let (_temp, path, digest) = fixture("control-open-stdin-shutdown");

    // When
    let output = wait_with_open_stdin(
        worker(&path, &digest),
        b"{\"v\":1,\"id\":1,\"type\":\"shutdown\"}\n",
    );

    // Then
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn fatal_request_exits_while_parent_keeps_stdin_open() {
    // Given
    let (_temp, path, digest) = fixture("control-open-stdin-fatal");

    // When
    let output = wait_with_open_stdin(worker(&path, &digest), b"{bad-json}\n");

    // Then
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_wav_startup_is_safe_machine_readable_fatal() {
    // Given
    let temp = must(TempDir::new("control-startup-fatal"));
    let missing = temp.path().join("secret-project-name.wav");

    // When
    let output = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-play"))
            .arg("--control-wav")
            .arg(&missing)
            .args(["--sha256", &"0".repeat(64), "--headless-callback-driver"])
            .output(),
    );

    // Then
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let fatal: Value = must(serde_json::from_slice(&output.stdout));
    assert_eq!(fatal["v"], 1);
    assert_eq!(fatal["type"], "fatal");
    assert_eq!(fatal["error"]["code"], "InvalidWav");
    assert!(
        !fatal["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("secret"))
    );
}
