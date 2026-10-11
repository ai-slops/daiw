#![cfg(feature = "native")]

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn player_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daiw-play"))
}

fn fixture(label: &str, samples: &[[i16; 2]]) -> (TempDir, std::path::PathBuf, String) {
    let temp = must(TempDir::new(label));
    let path = temp.path().join("stream.wav");
    let payload_bytes = must(u32::try_from(samples.len() * 4));
    let mut bytes = Vec::with_capacity(44 + samples.len() * 4);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(payload_bytes + 36).to_le_bytes());
    bytes.extend_from_slice(
        b"WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data",
    );
    bytes.extend_from_slice(&payload_bytes.to_le_bytes());
    for frame in samples {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    let digest = format!("{:x}", Sha256::digest(&bytes));
    must(std::fs::write(&path, bytes));
    (temp, path, digest)
}

fn worker(path: &std::path::Path, digest: &str) -> std::process::Child {
    must(
        player_command()
            .arg("--control-wav")
            .arg(path)
            .args(["--sha256", digest, "--headless-callback-driver"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn(),
    )
}

fn run_worker(mut child: std::process::Child, input: &[u8]) -> std::process::Output {
    must(must(child.stdin.take().ok_or("missing stdin")).write_all(input));
    must(child.wait_with_output())
}

fn json_lines(output: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(output)
        .lines()
        .map(|line| must(serde_json::from_str(line)))
        .collect()
}

#[test]
fn headless_control_worker_drives_real_callback_transport() {
    // Given
    let (_temp, path, digest) = fixture(
        "native-cli-worker",
        &[[1, 2], [3, 4], [5, 6], [7, 8], [9, 10], [11, 12]],
    );
    let child = worker(&path, &digest);
    let commands = concat!(
        "{\"v\":1,\"id\":1,\"type\":\"play\"}\n",
        "{\"v\":1,\"id\":2,\"type\":\"advance\",\"frames\":2}\n",
        "{\"v\":1,\"id\":3,\"type\":\"pause\"}\n",
        "{\"v\":1,\"id\":4,\"type\":\"seek\",\"frame\":1}\n",
        "{\"v\":1,\"id\":5,\"type\":\"setLoop\",\"loop\":{\"start\":1,\"end\":3}}\n",
        "{\"v\":1,\"id\":6,\"type\":\"play\"}\n",
        "{\"v\":1,\"id\":7,\"type\":\"advance\",\"frames\":4}\n",
        "{\"v\":1,\"id\":8,\"type\":\"stop\"}\n",
        "{\"v\":1,\"id\":9,\"type\":\"shutdown\"}\n",
    );

    // When
    let output = run_worker(child, commands.as_bytes());

    // Then
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let lines = json_lines(&output.stdout);
    assert_eq!(
        lines[0],
        serde_json::json!({"v":1,"type":"ready","frames":6})
    );
    let responses: Vec<&Value> = lines
        .iter()
        .filter(|line| line["type"] == "response")
        .collect();
    assert_eq!(responses.len(), 9);
    assert!(responses.iter().all(|response| response["ok"] == true));
    assert_eq!(responses[1]["state"]["positionFrame"], 2);
    assert_eq!(responses[2]["state"]["status"], "paused");
    assert_eq!(responses[6]["state"]["positionFrame"], 1);
    assert_eq!(responses[6]["state"]["stats"]["underrunCallbacks"], 0);
    assert_eq!(responses[7]["state"]["status"], "stopped");
    assert_eq!(responses[7]["state"]["positionFrame"], 0);
}

#[test]
fn control_worker_rejects_malformed_request_and_cleans_up() {
    // Given
    let (_temp, path, digest) = fixture("native-cli-worker-malformed", &[[1, 2]]);

    // When
    let output = run_worker(worker(&path, &digest), b"{not-json}\n");

    // Then
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let lines = json_lines(&output.stdout);
    assert_eq!(lines[0]["type"], "ready");
    assert_eq!(lines[1]["type"], "fatal");
    assert_eq!(lines[1]["error"]["code"], "InvalidRequest");
}

#[test]
fn control_worker_rejects_unknown_fields_and_overlong_lines() {
    // Given
    let cases = [
        b"{\"v\":1,\"id\":1,\"type\":\"play\",\"extra\":true}\n".to_vec(),
        vec![b'x'; 4097],
    ];

    for (index, input) in cases.iter().enumerate() {
        let (_temp, path, digest) =
            fixture(&format!("native-cli-worker-bounds-{index}"), &[[1, 2]]);

        // When
        let output = run_worker(worker(&path, &digest), input);

        // Then
        assert!(!output.status.success());
        assert!(output.stderr.is_empty());
        let lines = json_lines(&output.stdout);
        assert_eq!(lines[0]["type"], "ready");
        assert_eq!(lines[1]["type"], "fatal");
    }
}

#[test]
fn headless_worker_reports_eof_as_stopped_at_end_frame() {
    // Given
    let (_temp, path, digest) = fixture("native-cli-worker-eof", &[[1, 2]]);
    let input = concat!(
        "{\"v\":1,\"id\":1,\"type\":\"play\"}\n",
        "{\"v\":1,\"id\":2,\"type\":\"advance\",\"frames\":1}\n",
        "{\"v\":1,\"id\":3,\"type\":\"play\"}\n",
        "{\"v\":1,\"id\":4,\"type\":\"seek\",\"frame\":0}\n",
        "{\"v\":1,\"id\":5,\"type\":\"advance\",\"frames\":1}\n",
        "{\"v\":1,\"id\":6,\"type\":\"play\"}\n",
        "{\"v\":1,\"id\":7,\"type\":\"advance\",\"frames\":1}\n",
        "{\"v\":1,\"id\":8,\"type\":\"shutdown\"}\n",
    );

    // When
    let output = run_worker(worker(&path, &digest), input.as_bytes());

    // Then
    assert!(output.status.success());
    let lines = json_lines(&output.stdout);
    let eof = must(
        lines
            .iter()
            .find(|line| line["type"] == "response" && line["id"] == 2)
            .ok_or("missing advance response"),
    );
    assert_eq!(eof["state"]["status"], "stopped");
    assert_eq!(eof["state"]["positionFrame"], 1);
    let responses: Vec<&Value> = lines
        .iter()
        .filter(|line| line["type"] == "response")
        .collect();
    assert_eq!(responses[2]["state"]["status"], "stopped");
    assert_eq!(responses[3]["state"]["status"], "paused");
    assert_eq!(responses[4]["state"]["status"], "paused");
    assert_eq!(responses[4]["state"]["positionFrame"], 0);
    assert_eq!(responses[6]["state"]["status"], "stopped");
    assert_eq!(responses[6]["state"]["positionFrame"], 1);
}
