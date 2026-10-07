use std::os::unix::fs::PermissionsExt;
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
    assert_eq!(stdout(&output), "[-x][--help]");
}

#[test]
fn run_double_dash_relative_program() {
    // The documented form: `pne-profiler run -- ./program args`. Build a real
    // ./program in a scratch dir and run pne-profiler from there.
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("double_dash");
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join("program");
    std::fs::write(&program, "#!/bin/sh\nprintf '[%s]' \"$@\"\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pne-profiler"))
        .args(["run", "--", "./program", "-x", "--", "--help"])
        .current_dir(&dir)
        .output()
        .expect("failed to run pne-profiler");

    // clap consumes the first `--`; a later `--` belongs to the target.
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "[-x][--][--help]");
}

#[test]
fn run_target_nonzero_exit_is_propagated() {
    let output = prof(&["run", "sh", "-c", "exit 3"]);

    assert_eq!(output.status.code(), Some(3), "stderr: {}", stderr(&output));
}

#[test]
fn run_target_killed_by_signal_exits_128_plus_signal() {
    let output = prof(&["run", "sh", "-c", "kill -9 $$"]);

    // SIGKILL is 9, so the shell convention gives 137.
    assert_eq!(
        output.status.code(),
        Some(137),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn run_reports_resource_usage_on_stderr() {
    let output = prof(&["run", "true"]);

    let err = stderr(&output);
    for field in ["wall time", "user time", "sys time", "CPU usage", "max RSS"] {
        assert!(err.contains(field), "stderr missing {field:?}: {err}");
    }
    // The report must not mix into the target's stdout.
    assert_eq!(stdout(&output), "");
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
