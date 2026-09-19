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
