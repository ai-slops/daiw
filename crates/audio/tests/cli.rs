#[path = "support/must.rs"]
mod support;
use support::must;

fn render_command() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_daiw-render"))
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
