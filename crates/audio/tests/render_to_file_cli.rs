use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn snapshot(length: f64) -> Value {
    json!({
        "revision": "render-to-file", "tempo": 60.0, "length": length,
        "tracks": [], "notes": [], "regions": []
    })
}

fn run(root: &std::path::Path, output: &std::path::Path, input: &Value) -> std::process::Output {
    let mut child = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--render-to-file", "--source-root"])
            .arg(root)
            .arg("--output")
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn(),
    );
    let Some(stdin) = child.stdin.as_mut() else {
        panic!("renderer stdin must exist");
    };
    must(stdin.write_all(&must(serde_json::to_vec(input))));
    must(child.wait_with_output())
}

#[test]
fn zero_frame_render_writes_exact_header_and_bounded_descriptor() {
    // Given
    let temp = must(TempDir::new("render-zero"));
    let output_path = temp.path().join("zero.wav");
    let input = json!({"snapshot": snapshot(0.0), "sources": {}});

    // When
    let output = run(temp.path(), &output_path, &input);

    // Then
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 16 * 1024);
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let descriptor: Value = must(serde_json::from_slice(&output.stdout));
    let bytes = must(std::fs::read(output_path));
    assert_eq!(bytes.len(), 44);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(descriptor["format"], "daiw-wav-pcm16-v1");
    assert_eq!(descriptor["sampleRate"], 48_000);
    assert_eq!(descriptor["channels"], 2);
    assert_eq!(descriptor["frames"], 0);
    assert_eq!(descriptor["byteLength"], 44);
    assert_eq!(descriptor["sha256"], format!("{:x}", Sha256::digest(bytes)));
}

#[test]
fn existing_output_is_refused_without_modification() {
    // Given
    let temp = must(TempDir::new("render-exists"));
    let output_path = temp.path().join("existing.wav");
    must(std::fs::write(&output_path, b"keep"));
    let input = json!({"snapshot": snapshot(0.0), "sources": {}});

    // When
    let output = run(temp.path(), &output_path, &input);

    // Then
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(must(std::fs::read(output_path)), b"keep");
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn descriptor_flush_failure_removes_new_output_and_uses_io_exit_code() {
    // Given
    let full_path = std::path::Path::new("/dev/full");
    if !full_path.exists() {
        return;
    }
    let temp = must(TempDir::new("render-descriptor-failure"));
    let output_path = temp.path().join("unpublished.wav");
    let input = json!({"snapshot": snapshot(0.0), "sources": {}});
    let full = must(std::fs::OpenOptions::new().write(true).open(full_path));
    let mut child = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--render-to-file", "--source-root"])
            .arg(temp.path())
            .arg("--output")
            .arg(&output_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(full))
            .stderr(Stdio::piped())
            .spawn(),
    );
    let Some(stdin) = child.stdin.as_mut() else {
        panic!("renderer stdin must exist");
    };
    must(stdin.write_all(&must(serde_json::to_vec(&input))));

    // When
    let result = must(child.wait_with_output());

    // Then
    assert_eq!(result.status.code(), Some(3));
    assert!(!output_path.exists());
}

#[test]
fn numeric_render_failure_removes_incomplete_output() {
    // Given
    let temp = must(TempDir::new("render-cleanup"));
    let output_path = temp.path().join("incomplete.wav");
    let mut core = snapshot(0.01);
    core["tracks"] = json!([{
        "id": "tone", "name": "tone", "role": "music", "color": "#fff",
        "instrument": "synth", "gain": 800.0, "pan": 0.0, "mute": false,
        "solo": false, "cutoff": 20000.0
    }]);
    core["notes"] = json!([{
        "id": "note", "start": 0.0, "duration": 0.01, "pitch": 69.0,
        "velocity": 1.0, "instanceId": "instance", "trackId": "tone",
        "origin": {"definitionId": "definition", "noteId": "note", "modifierIds": []}
    }]);
    let input = json!({"snapshot": core, "sources": {}});

    // When
    let output = run(temp.path(), &output_path, &input);

    // Then
    assert_eq!(output.status.code(), Some(1));
    assert!(!output_path.exists());
    assert!(output.stdout.is_empty());
}

#[test]
fn resource_and_output_io_failures_have_distinct_exit_codes() {
    // Given
    let temp = must(TempDir::new("render-exits"));
    let too_large = temp.path().join("too-large.wav");
    let missing_parent = temp.path().join("missing").join("io.wav");
    let resource_input = json!({"snapshot": snapshot(181.0), "sources": {}});
    let valid_input = json!({"snapshot": snapshot(0.0), "sources": {}});

    // When
    let resource = run(temp.path(), &too_large, &resource_input);
    let io = run(temp.path(), &missing_parent, &valid_input);

    // Then
    assert_eq!(resource.status.code(), Some(2));
    assert_eq!(io.status.code(), Some(3));
    assert!(!too_large.exists());
    assert!(!missing_parent.exists());
}

#[test]
fn maximum_render_streams_exact_bounded_wave_and_small_descriptor() {
    // Given
    let temp = must(TempDir::new("render-maximum"));
    let output_path = temp.path().join("maximum.wav");
    let input = json!({"snapshot": snapshot(180.0), "sources": {}});

    // When
    let output = run(temp.path(), &output_path, &input);

    // Then
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 16 * 1024);
    let descriptor: Value = must(serde_json::from_slice(&output.stdout));
    assert_eq!(descriptor["frames"], 8_640_000);
    assert_eq!(descriptor["byteLength"], 34_560_044);
    assert_eq!(must(std::fs::metadata(output_path)).len(), 34_560_044);
}
