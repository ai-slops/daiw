use std::io::Write;
use std::process::Stdio;

use serde_json::{Value, json};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn render_command() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_daiw-render"))
}

fn run_with_stdin(arguments: &[&str], input: &Value) -> std::process::Output {
    let mut child = must(
        render_command()
            .args(arguments)
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

fn oversized_snapshot() -> Value {
    json!({
        "revision": "r1", "tempo": 60.0, "length": 181.0,
        "tracks": [], "notes": [], "regions": []
    })
}

#[test]
fn help_prints_usage_and_succeeds_without_stdin() {
    // Given
    let mut command = render_command();
    command.arg("--help");

    // When
    let output = must(command.output());

    // Then
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("daiw-render --decode"));
    assert!(output.stderr.is_empty());
}

#[test]
fn unknown_argument_fails_with_diagnostic() {
    // Given
    let mut command = render_command();
    command.arg("--unknown");

    // When
    let output = must(command.output());

    // Then
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown argument"));
}

#[test]
fn legacy_render_resource_error_keeps_exit_code_one() {
    // Given
    let input = oversized_snapshot();

    // When
    let output = run_with_stdin(&[], &input);

    // Then
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("frame output limit"));
}

#[test]
fn file_backed_render_resource_error_uses_exit_code_two() {
    // Given
    let temp = must(TempDir::new("render-limit"));
    let input = json!({"snapshot": oversized_snapshot(), "sources": {}});
    let root = temp.path().to_string_lossy();

    // When
    let output = run_with_stdin(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &input,
    );

    // Then
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("frame output limit"));
}

#[test]
fn file_backed_source_count_limit_uses_exit_code_two() {
    // Given
    let temp = must(TempDir::new("source-limit"));
    let source = json!({
        "cacheId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "frames": 1, "sampleRate": 8000, "channels": 1, "byteLength": 84
    });
    let sources: serde_json::Map<String, Value> = (0..65)
        .map(|index| (format!("asset-{index}"), source.clone()))
        .collect();
    let input = json!({
        "snapshot": {
            "revision": "r1", "tempo": 60.0, "length": 0.0,
            "tracks": [], "notes": [], "regions": []
        },
        "sources": sources
    });
    let root = temp.path().to_string_lossy();

    // When
    let output = run_with_stdin(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &input,
    );

    // Then
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("at most 64"));
}
