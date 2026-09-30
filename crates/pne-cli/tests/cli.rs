use std::process::Command;

#[test]
fn binary_runs_and_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_pne-profiler"))
        .output()
        .expect("failed to run pne-profiler");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}
