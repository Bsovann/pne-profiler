use std::process::{Command, Output};

/// Runs pne-profiler with `args` and captures its exit status, stdout and stderr.
fn prof(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pne-profiler"))
        .args(args)
        .output()
        .expect("failed to run pne-profiler")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn binary_runs_and_prints_version() {
    let output = prof(&["--version"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn run_target_success() {
    // printf echoes each argument back in brackets, which also checks that
    // hyphenated args reach the target instead of being parsed by clap.
    let output = prof(&["run", "printf", "[%s]", "-x", "--help"]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("[-x][--help]"),
        "stdout: {}",
        stdout(&output)
    );
}

#[test]
fn run_target_nonzero_exit_is_propagated() {
    let output = prof(&["run", "sh", "-c", "exit 3"]);

    assert_eq!(output.status.code(), Some(3), "stderr: {}", stderr(&output));
}

#[test]
fn run_target_command_not_found() {
    let output = prof(&["run", "pne-definitely-not-a-real-program"]);

    // 127 is the shell convention for "command not found".
    assert_eq!(
        output.status.code(),
        Some(127),
        "stderr: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("pne-definitely-not-a-real-program"),
        "stderr should name the missing program: {}",
        stderr(&output)
    );
}
