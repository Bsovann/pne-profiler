#![forbid(unsafe_code)]
use anyhow::Context;
use clap::{Parser, Subcommand};
use pne_core::{ExitOutcome, RunStats};
use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

fn main() -> ExitCode {
    // On a usage error or --help/--version, clap prints and exits the process
    // itself, so past this line we always have a valid subcommand.
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Run { cmd } => run(&cmd),
    };

    // Not `fn main() -> anyhow::Result<..>`: returning Err from main always
    // exits 1, but a target that can't be started should get the shell's
    // 126/127. `{:#}` prints the whole context chain on one line.
    result.unwrap_or_else(|err| {
        eprintln!("pne-profiler: {err:#}");
        failure_exit_code(&err)
    })
}

/// Runs the target, prints its resource usage, and returns its exit status.
fn run(cmd: &[OsString]) -> anyhow::Result<ExitCode> {
    // First word is the program, the rest are its arguments.
    let (prog, args) = cmd.split_first().expect("clap guarantees at least one");

    // Spawn the target. Inheriting stdio lets it share our terminal, so it
    // reads input and prints output as if run directly.
    let start = Instant::now();
    let child = Command::new(prog)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("{}: cannot run", prog.to_string_lossy()))?;

    // Wait with wait4 rather than Child::wait so the kernel also hands back
    // the target's resource usage. wait4 takes `child` by value because it
    // reaps it, so it can't be waited on again.
    let (outcome, resource_usage) = pne_perf::wait4(child)
        .with_context(|| format!("{}: cannot wait for it", prog.to_string_lossy()))?;
    let stats = RunStats {
        wall_time: start.elapsed(),
        resource_usage,
    };
    // stderr keeps the report out of the target's stdout, so piping still works.
    eprint!("{stats}");

    // Exit with the target's result so scripts and CI see its failures,
    // using the shell's conventions for the cases with no exit code.
    Ok(match outcome {
        // Normal exit: pass its code through.
        ExitOutcome::Exited(code) => ExitCode::from(code as u8),
        // Killed by a signal: report 128 + signal.
        ExitOutcome::Signaled(signal) => ExitCode::from(128 + signal as u8),
    })
}

/// Exit status for an error from `run`, using the shell's conventions when the
/// target couldn't be started.
fn failure_exit_code(err: &anyhow::Error) -> ExitCode {
    // Only a spawn failure is a bare io::Error under its context. A wait4
    // failure is a pne_perf::WaitError, whose io::Error is its source rather
    // than the error itself, so downcast_ref doesn't match it.
    match err.downcast_ref::<io::Error>() {
        // The program isn't on PATH or doesn't exist.
        Some(e) if e.kind() == io::ErrorKind::NotFound => ExitCode::from(127),
        // Found but couldn't be executed, e.g. permission denied.
        Some(_) => ExitCode::from(126),
        None => ExitCode::FAILURE,
    }
}

/// Top-level command line: `pne-profiler <SUBCOMMAND>`.
#[derive(Parser, Debug)]
#[command(version, about = "A lightweight HPC profiler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run a command under the profiler
    Run {
        /// The command to run, followed by its arguments
        // allow_hyphen_values passes flags like `-la` or `--help` through to the
        // target instead of having clap treat them as pne-profiler's own options.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<OsString>,
    },
}
