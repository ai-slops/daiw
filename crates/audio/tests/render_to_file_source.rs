use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn run(arguments: &[&str], input: &Value) -> std::process::Output {
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

#[test]
fn file_output_reads_source_across_4096_frame_cache_boundary() {
    // Given
    let temp = must(TempDir::new("render-source-boundary"));
    let cache_id = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let source_samples: Vec<f32> = (0..4097)
        .flat_map(|frame| {
            let value = (frame % 257) as f32 / 256.0 - 0.5;
            [value, -value]
        })
        .collect();
    let source_wave = must(daiw_audio::wav(&daiw_audio::RenderedAudio {
        sample_rate: 48_000,
        channels: 2,
        samples: source_samples,
    }));
    let source_path = temp.path().join("source.wav");
    let cache_path = temp.path().join(format!("{cache_id}.pcm"));
    must(std::fs::write(&source_path, source_wave));
    let decoded = must(
        Command::new(env!("CARGO_BIN_EXE_daiw-render"))
            .args(["--decode-file", "--input"])
            .arg(&source_path)
            .arg("--output")
            .arg(&cache_path)
            .output(),
    );
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let descriptor: Value = must(serde_json::from_slice(&decoded.stdout));
    let snapshot = json!({
        "revision": "source-boundary", "tempo": 60.0, "length": 4097.0 / 48000.0,
        "tracks": [{
            "id": "sample", "name": "sample", "role": "music", "color": "#fff",
            "instrument": "sample", "gain": -1.0, "pan": 0.2, "mute": false,
            "solo": false, "cutoff": 17000.0
        }],
        "notes": [],
        "regions": [{
            "id": "region", "trackId": "sample", "assetId": "asset", "start": 0.0,
            "duration": 4097.0 / 48000.0, "gain": 0.5, "sourceOffset": 0.0
        }]
    });
    let input = json!({
        "snapshot": snapshot,
        "sources": {"asset": {
            "cacheId": cache_id, "sha256": descriptor["sha256"],
            "frames": descriptor["frames"], "sampleRate": descriptor["sampleRate"],
            "channels": descriptor["channels"], "byteLength": descriptor["byteLength"]
        }}
    });
    let output_path = temp.path().join("mix.wav");
    let root = temp.path().to_string_lossy();
    let output_text = output_path.to_string_lossy();

    // When
    let resident = run(
        &["--render-file-backed", "--source-root", root.as_ref()],
        &input,
    );
    let streamed = run(
        &[
            "--render-to-file",
            "--source-root",
            root.as_ref(),
            "--output",
            output_text.as_ref(),
        ],
        &input,
    );

    // Then
    assert!(
        resident.status.success(),
        "{}",
        String::from_utf8_lossy(&resident.stderr)
    );
    assert!(
        streamed.status.success(),
        "{}",
        String::from_utf8_lossy(&streamed.stderr)
    );
    assert_eq!(must(std::fs::read(output_path)), resident.stdout);
}
