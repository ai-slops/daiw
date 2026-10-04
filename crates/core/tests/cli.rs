use std::io::Write;
use std::process::{Command, Stdio};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daiw-core"))
}

#[test]
fn help_lists_one_shot_commands() {
    // Given
    let mut command = binary();
    command.arg("--help").stdin(Stdio::null());

    // When
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => panic!("failed to run binary: {error}"),
    };

    // Then
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("fixture"));
    assert!(stdout.contains("dry-run"));
}

#[test]
fn fixture_command_emits_project_json() {
    // Given
    let mut command = binary();
    command.args(["fixture", "CLI Demo"]).stdin(Stdio::null());

    // When
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => panic!("failed to run binary: {error}"),
    };
    let project: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(project) => project,
        Err(error) => panic!("invalid fixture JSON: {error}"),
    };

    // Then
    assert!(output.status.success());
    assert_eq!(project["name"], "CLI Demo");
    assert_eq!(project["tracks"].as_array().map(Vec::len), Some(5));
}

fn invoke(args: &[&str], input: &[u8]) -> std::io::Result<std::process::Output> {
    let mut child = binary()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("missing stdin"))?
        .write_all(input)?;
    child.wait_with_output()
}

#[test]
fn cli_exports_and_validates_envelopes_without_changing_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = invoke(&["fixture", "Exchange"], &[])?;
    let raw: serde_json::Value = serde_json::from_slice(&fixture.stdout)?;
    let exported = invoke(&["export"], &fixture.stdout)?;
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&exported.stdout)?;
    assert_eq!(document["content"]["id"], raw["id"]);
    assert!(invoke(&["validate"], &exported.stdout)?.status.success());
    assert!(
        invoke(&["explain", "instance-chorus-a"], &exported.stdout)?
            .status
            .success()
    );
    let mut unsupported = document;
    unsupported["formatVersion"] = serde_json::json!(999);
    let rejected = invoke(&["validate"], &serde_json::to_vec(&unsupported)?)?;
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    Ok(())
}
