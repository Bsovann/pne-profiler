use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};

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

/// Replaces each run of digits with `N`, so a report from a real run can be
/// compared as a whole string even though its numbers change every time.
fn mask_numbers(text: &str) -> String {
    let mut masked = String::new();
    for c in text.chars() {
        if !c.is_ascii_digit() {
            masked.push(c);
        } else if !masked.ends_with('N') {
            masked.push('N');
        }
    }
    masked
}

/// The end-of-run report as `mask_numbers` leaves it.
const MASKED_REPORT: &str = "\n\
     pne-profiler: resource usage\n\
     \x20 wall time   N.N s\n\
     \x20 user time   N.N s\n\
     \x20 sys time    N.N s\n\
     \x20 CPU usage   N%\n\
     \x20 max RSS     N.N MiB\n";

#[test]
fn run_reports_resource_usage_on_stderr() {
    let output = prof(&["run", "true"]);

    // The exact numbers are checked by pne-core's report tests; this checks
    // that the real binary prints that report, and nothing else, to stderr.
    assert_eq!(mask_numbers(&stderr(&output)), MASKED_REPORT);
    // The report must not mix into the target's stdout.
    assert_eq!(stdout(&output), "");
}

#[test]
fn run_target_not_executable() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("not_executable");
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join("program");
    std::fs::write(&program, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o644)).unwrap();

    let output = prof(&["run", program.to_str().unwrap()]);

    // 126 is the shell convention for "found but cannot execute".
    assert_eq!(
        output.status.code(),
        Some(126),
        "stderr: {}",
        stderr(&output)
    );
    // The OS's own wording follows, e.g. "Permission denied (os error 13)".
    let expected = format!("pne-profiler: {}: cannot run: ", program.display());
    assert!(
        stderr(&output).starts_with(&expected),
        "stderr should start with {expected:?}: {}",
        stderr(&output)
    );
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
    let expected = "pne-profiler: pne-definitely-not-a-real-program: cannot run: ";
    assert!(
        stderr(&output).starts_with(expected),
        "stderr should start with {expected:?}: {}",
        stderr(&output)
    );
}

/// Runs `script` under pne-profiler and, once the target is running, sends
/// `signal` to the whole process group, as the terminal does for Ctrl-C.
///
/// `script` must print `ready` to stdout first. The profiler installs its
/// handlers before spawning the target, so that line also means the profiler
/// is ready for the signal.
fn prof_interrupted(script: &str, signal: &str) -> Output {
    // A process group of its own keeps the signal away from the test runner.
    let mut child = Command::new(env!("CARGO_BIN_EXE_pne-profiler"))
        .args(["run", "sh", "-c", script])
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run pne-profiler");

    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line, "ready\n");

    // A negative PID addresses the process group; process_group(0) made the
    // profiler its leader, so the group ID is the profiler's PID.
    let status = Command::new("kill")
        .args([&format!("-{signal}"), "--", &format!("-{}", child.id())])
        .status()
        .expect("failed to run kill");
    assert!(status.success(), "kill -{signal} failed");

    child.wait_with_output().unwrap()
}

#[test]
fn run_ctrl_c_still_reports() {
    // exec makes sleep the target, so it gets SIGINT with its default action.
    let output = prof_interrupted("echo ready; exec sleep 30", "INT");

    // 130 = 128 + SIGINT means the target died of the signal, which it only
    // does if its disposition was reset to the default before exec. Getting
    // it from the profiler means the profiler survived to report it.
    assert_eq!(
        output.status.code(),
        Some(130),
        "stderr: {}",
        stderr(&output)
    );
    assert_eq!(mask_numbers(&stderr(&output)), MASKED_REPORT);
}

#[test]
fn run_ctrl_backslash_still_reports() {
    // SIGQUIT dumps core by default; ulimit keeps the test from writing one.
    let output = prof_interrupted("ulimit -c 0; echo ready; exec sleep 30", "QUIT");

    // SIGQUIT is 3, so the shell convention gives 131.
    assert_eq!(
        output.status.code(),
        Some(131),
        "stderr: {}",
        stderr(&output)
    );
    assert_eq!(mask_numbers(&stderr(&output)), MASKED_REPORT);
}
