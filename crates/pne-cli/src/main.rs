#![forbid(unsafe_code)]
use anyhow::Context;
use clap::{Parser, Subcommand};
use pne_core::{ExitOutcome, RunStats};
use signal_hook::consts::{SIGINT, SIGQUIT};
use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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

    // Before spawning, so there's no window where Ctrl-C kills us but not the target.
    survive_terminal_interrupts();

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

/// Keeps Ctrl-C (SIGINT) and Ctrl-\ (SIGQUIT) from killing the profiler, so
/// it can still report on a target the user interrupted. The terminal sends
/// these to the whole foreground process group, so the target gets them too
/// and decides for itself whether to stop.
///
/// Like `time` and `system(3)`, the profiler ignores them, but by catching
/// them with a handler that does nothing rather than with `SIG_IGN`. An
/// ignored signal stays ignored across `execve`, so the target would inherit
/// it. A caught signal is reset to its default action, so the target handles
/// Ctrl-C normally without any setup in the child before exec.
///
/// The handlers stay installed until the profiler exits. Failing to install
/// them only costs the report on Ctrl-C, so it warns instead of failing.
fn survive_terminal_interrupts() {
    // The flag is only there because signal-hook needs an action to run; nothing reads it.
    let caught = Arc::new(AtomicBool::new(false));
    for signal in [SIGINT, SIGQUIT] {
        if let Err(err) = signal_hook::flag::register(signal, Arc::clone(&caught)) {
            eprintln!("pne-profiler: warning: cannot catch signal {signal}: {err}");
        }
    }
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
