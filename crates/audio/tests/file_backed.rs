use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn snapshot() -> Value {
    json!({
        "revision": "r1", "tempo": 60.0, "length": 0.001,
        "tracks": [{
            "id": "sample", "name": "sample", "role": "music", "color": "#fff",
            "instrument": "sample", "gain": 0.0, "pan": 0.0, "mute": false,
            "solo": false, "cutoff": 20000.0
        }],
        "notes": [],
        "regions": [{
            "id": "region", "trackId": "sample", "assetId": "asset",
            "start": 0.0, "duration": 0.001, "gain": 0.0, "sourceOffset": 0.000125
        }]
    })
}

fn run_with_stdin(arguments: &[&str], input: &Value) -> std::process::Output {
    let mut child = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
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

fn create_cache(temp: &TempDir, cache_id: &str) -> (Value, Vec<f32>) {
    let wav = must(daiw_audio::wav(&daiw_audio::RenderedAudio {
        sample_rate: 8_000,
        channels: 2,
        samples: vec![
            0.1, -0.1, 0.2, -0.2, 0.3, -0.3, 0.4, -0.4, 0.5, -0.5, 0.6, -0.6,
        ],
    }));
    let input = temp.path().join("input.wav");
    let output = temp.path().join(format!("{cache_id}.pcm"));
    must(std::fs::write(&input, wav));
    let decoded = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(input)
            .arg("--output")
            .arg(output)
            .output(),
    );
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let descriptor = must(serde_json::from_slice(&decoded.stdout));
    let bytes = must(std::fs::read(temp.path().join(format!("{cache_id}.pcm"))));
    let samples = bytes[80..]
        .chunks_exact(4)
        .map(|value| f32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        .collect();
    (descriptor, samples)
}

#[test]
fn file_backed_render_is_wav_byte_identical_to_resident_render() {
    // Given
    let temp = must(TempDir::new("file-parity"));
    let cache_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let (descriptor, samples) = create_cache(&temp, cache_id);
    let source = json!({
        "cacheId": cache_id,
        "sha256": descriptor["sha256"],
        "frames": descriptor["frames"],
        "sampleRate": descriptor["sampleRate"],
        "channels": descriptor["channels"],
        "byteLength": descriptor["byteLength"]
    });
    let file_input = json!({"snapshot": snapshot(), "sources": {"asset": source}});
    let resident_input = json!({
        "snapshot": snapshot(),
        "assets": {"asset": {
            "sampleRate": 8000, "channels": 2,
            "samples": samples
        }}
    });

    // When
    let resident = run_with_stdin(&[], &resident_input);
    let root = temp.path().to_string_lossy();
    let backed = run_with_stdin(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &file_input,
    );

    // Then
    assert!(
        resident.status.success(),
        "{}",
        String::from_utf8_lossy(&resident.stderr)
    );
    assert!(
        backed.status.success(),
        "{}",
        String::from_utf8_lossy(&backed.stderr)
    );
    assert_eq!(backed.stdout, resident.stdout);
}

#[test]
fn file_backed_render_rejects_extra_sources_and_trailing_bytes() {
    // Given
    let temp = must(TempDir::new("file-errors"));
    let cache_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let (descriptor, _) = create_cache(&temp, cache_id);
    let source = json!({
        "cacheId": cache_id, "sha256": descriptor["sha256"], "frames": descriptor["frames"],
        "sampleRate": descriptor["sampleRate"], "channels": descriptor["channels"],
        "byteLength": descriptor["byteLength"]
    });
    let input = json!({"snapshot": snapshot(), "sources": {"asset": source, "extra": source}});
    let root = temp.path().to_string_lossy();

    // When
    let result = run_with_stdin(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &input,
    );

    // Then
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("extra"));
}

#[test]
fn core_snapshot_with_meter_metadata_and_nested_origin_renders_through_file_cli() {
    // Given
    let mut project = daiw_core::create_project(Some("audio-pipeline"), true);
    project.meter_map = Some(must(daiw_core::MeterMap::new(vec![
        daiw_core::MeterPoint::new(must(daiw_core::Tick::from_beats(0.0)), 4, 4),
        daiw_core::MeterPoint::new(must(daiw_core::Tick::from_beats(4.0)), 3, 4),
    ])));
    let core_snapshot = must(daiw_core::snapshot(&project, None));
    let mut snapshot = must(serde_json::to_value(core_snapshot));
    let origin = &mut snapshot["notes"][0]["origin"];
    origin["definitionPath"] = json!([{
        "definitionId": origin["definitionId"],
        "definitionRevision": 0,
        "placementId": "child-placement",
        "loopIndex": 0
    }]);
    origin["instanceLoopIndex"] = json!(0);
    let input = json!({"snapshot": snapshot, "sources": {}});
    let temp = must(TempDir::new("core-pipeline"));
    let root = temp.path().to_string_lossy();

    // When
    let output = run_with_stdin(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &input,
    );

    // Then
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(&output.stdout[0..4], b"RIFF");
}

#[test]
fn fractional_source_offset_file_render_matches_legacy_wave_golden() {
    // Given
    let temp = must(TempDir::new("fractional-golden"));
    let cache_id = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let (descriptor, _) = create_cache(&temp, cache_id);
    let source = json!({
        "cacheId": cache_id, "sha256": descriptor["sha256"], "frames": descriptor["frames"],
        "sampleRate": descriptor["sampleRate"], "channels": descriptor["channels"],
        "byteLength": descriptor["byteLength"]
    });
    let mut core = snapshot();
    core["length"] = json!(0.000125);
    core["regions"][0]["duration"] = json!(0.000125);
    core["regions"][0]["sourceOffset"] = json!(0.0000625);
    let input = json!({"snapshot": core, "sources": {"asset": source}});
    let root = temp.path().to_string_lossy();

    // When
    let output = run_with_stdin(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &input,
    );

    // Then
    assert!(output.status.success());
    let wave: Vec<i16> = output.stdout[44..]
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect();
    assert_eq!(
        wave,
        vec![
            4296, -4296, 4609, -4609, 4632, -4632, 4633, -4633, 4634, -4634, 4634, -4634,
        ]
    );
}
