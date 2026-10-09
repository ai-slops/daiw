use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

const CACHE_ID: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

struct Fixture {
    temp: TempDir,
    input: Value,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let temp = must(TempDir::new(label));
        let wav = must(daiw_audio::wav(&daiw_audio::RenderedAudio {
            sample_rate: 8_000,
            channels: 2,
            samples: vec![0.25, -0.25, 0.5, -0.5],
        }));
        let wav_path = temp.path().join("input.wav");
        let cache_path = temp.path().join(format!("{CACHE_ID}.pcm"));
        must(std::fs::write(&wav_path, wav));
        let decoded = must(
            Command::new(env!("CARGO_BIN_EXE_daiw-render"))
                .args(["--decode-file", "--input"])
                .arg(wav_path)
                .arg("--output")
                .arg(cache_path)
                .output(),
        );
        assert!(decoded.status.success());
        let descriptor: Value = must(serde_json::from_slice(&decoded.stdout));
        let snapshot = json!({
            "revision": "r1", "tempo": 60.0, "length": 0.001,
            "tracks": [{"id": "sample", "name": "sample", "role": "music",
                "color": "#fff", "instrument": "sample", "gain": 0.0, "pan": 0.0,
                "mute": false, "solo": false, "cutoff": 20000.0}],
            "notes": [],
            "regions": [{"id": "region", "trackId": "sample", "assetId": "asset",
                "start": 0.0, "duration": 0.001, "gain": 0.0, "sourceOffset": 0.0}]
        });
        let source = json!({
            "cacheId": CACHE_ID, "sha256": descriptor["sha256"],
            "frames": descriptor["frames"], "sampleRate": descriptor["sampleRate"],
            "channels": descriptor["channels"], "byteLength": descriptor["byteLength"]
        });
        Self {
            temp,
            input: json!({"snapshot": snapshot, "sources": {"asset": source}}),
        }
    }

    fn cache_path(&self) -> std::path::PathBuf {
        self.temp.path().join(format!("{CACHE_ID}.pcm"))
    }

    fn run(&self) -> std::process::Output {
        let root = self.temp.path().to_string_lossy();
        let mut child = must(
            Command::new(env!("CARGO_BIN_EXE_daiw-render"))
                .args(["--render-file-backed", "--source-root", root.as_ref()])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn(),
        );
        let Some(stdin) = child.stdin.as_mut() else {
            panic!("renderer stdin must exist");
        };
        must(stdin.write_all(&must(serde_json::to_vec(&self.input))));
        must(child.wait_with_output())
    }
}

fn diagnostic(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn file_backed_cli_rejects_truncated_source() {
    // Given
    let fixture = Fixture::new("truncated");
    let file = must(
        std::fs::OpenOptions::new()
            .write(true)
            .open(fixture.cache_path()),
    );
    must(file.set_len(83));

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("length"));
}

#[test]
fn file_backed_cli_rejects_trailing_source_bytes() {
    // Given
    let fixture = Fixture::new("trailing");
    let mut file = must(
        std::fs::OpenOptions::new()
            .append(true)
            .open(fixture.cache_path()),
    );
    must(file.write_all(&[0]));

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("length"));
}

#[test]
fn file_backed_cli_rejects_payload_hash_mismatch() {
    // Given
    let fixture = Fixture::new("payload-hash");
    let mut bytes = must(std::fs::read(fixture.cache_path()));
    bytes[80] ^= 1;
    must(std::fs::write(fixture.cache_path(), bytes));

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("payload SHA-256"));
}

#[test]
fn file_backed_library_returns_typed_payload_hash_error() {
    // Given
    let fixture = Fixture::new("typed-payload-hash");
    let mut bytes = must(std::fs::read(fixture.cache_path()));
    bytes[80] ^= 1;
    must(std::fs::write(fixture.cache_path(), bytes));
    let input: daiw_audio::FileRenderInput = must(serde_json::from_value(fixture.input.clone()));

    // When
    let result = daiw_audio::render_file_backed(input, fixture.temp.path());

    // Then
    assert!(matches!(
        result,
        Err(daiw_audio::AudioError::PcmPayloadHashMismatch)
    ));
}

#[test]
fn file_backed_cli_rejects_nonfinite_payload() {
    // Given
    let fixture = Fixture::new("nonfinite");
    let mut bytes = must(std::fs::read(fixture.cache_path()));
    bytes[80..84].copy_from_slice(&f32::NAN.to_le_bytes());
    must(std::fs::write(fixture.cache_path(), bytes));

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("non-finite"));
}

#[test]
fn file_backed_cli_rejects_descriptor_metadata_mismatch() {
    // Given
    let mut fixture = Fixture::new("metadata");
    fixture.input["sources"]["asset"]["sampleRate"] = json!(48_000);

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("metadata"));
}

#[test]
fn file_backed_cli_rejects_nonzero_reserved_header() {
    // Given
    let fixture = Fixture::new("header");
    let mut bytes = must(std::fs::read(fixture.cache_path()));
    bytes[68] = 1;
    must(std::fs::write(fixture.cache_path(), bytes));

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("header"));
}

#[test]
fn file_backed_cli_rejects_unknown_outer_fields() {
    // Given
    let mut fixture = Fixture::new("strict-outer");
    fixture.input["unknownPath"] = json!("/tmp/escape");

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("unknown field"));
}

#[test]
fn file_backed_cli_rejects_unknown_source_descriptor_fields() {
    // Given
    let mut fixture = Fixture::new("strict-source");
    fixture.input["sources"]["asset"]["projectPath"] = json!("/tmp/escape");

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert!(diagnostic(&output).contains("unknown field"));
}

#[test]
fn file_backed_cli_rejects_cache_ids_that_are_paths() {
    // Given
    let mut fixture = Fixture::new("cache-path");
    fixture.input["sources"]["asset"]["cacheId"] = json!("../outside");

    // When
    let output = fixture.run();

    // Then
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert!(diagnostic(&output).contains("cacheId"));
}
