#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use pne_core::{ExitOutcome, RunStats};
use pne_perf::WaitError;
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

    // Not `fn main() -> Result<..>`: returning Err from main always exits 1,
    // but a target that can't be started should get the shell's 126/127.
    result.unwrap_or_else(|err| {
        let code = err.exit_code();
        // anyhow's `{:#}` prints the error and each of its sources on one line.
        eprintln!("pne-profiler: {:#}", anyhow::Error::new(err));
        ExitCode::from(code)
    })
}

/// Why `run` couldn't report on the target.
///
/// Each variant gets its exit code in [`RunError::exit_code`], so a new
/// failure mode has to pick one before it compiles.
#[derive(Debug, thiserror::Error)]
enum RunError {
    /// The target couldn't be started.
    #[error("{}: cannot run", .prog.to_string_lossy())]
    Spawn {
        prog: OsString,
        #[source]
        source: io::Error,
    },
    /// The target started, but waiting for it or reading its usage failed.
    #[error("{}: cannot wait for it", .prog.to_string_lossy())]
    Wait {
        prog: OsString,
        #[source]
        source: WaitError,
    },
}

impl RunError {
    /// Exit status for this error, using the shell's conventions when the
    /// target couldn't be started.
    fn exit_code(&self) -> u8 {
        match self {
            // The program isn't on PATH or doesn't exist.
            RunError::Spawn { source, .. } if source.kind() == io::ErrorKind::NotFound => 127,
            // Found but couldn't be executed, e.g. permission denied.
            RunError::Spawn { .. } => 126,
            RunError::Wait { .. } => 1,
        }
    }
}

/// Runs the target, prints its resource usage, and returns its exit status.
fn run(cmd: &[OsString]) -> Result<ExitCode, RunError> {
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
        .map_err(|source| RunError::Spawn {
            prog: prog.clone(),
            source,
        })?;

    // Wait with wait4 rather than Child::wait so the kernel also hands back
    // the target's resource usage. wait4 takes `child` by value because it
    // reaps it, so it can't be waited on again.
    let (outcome, resource_usage) = pne_perf::wait4(child).map_err(|source| RunError::Wait {
        prog: prog.clone(),
        source,
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_error(kind: io::ErrorKind) -> RunError {
        RunError::Spawn {
            prog: "prog".into(),
            source: kind.into(),
        }
    }

    fn wait_error(source: WaitError) -> RunError {
        RunError::Wait {
            prog: "prog".into(),
            source,
        }
    }

    #[test]
    fn spawn_not_found_exits_127() {
        assert_eq!(spawn_error(io::ErrorKind::NotFound).exit_code(), 127);
    }

    #[test]
    fn other_spawn_errors_exit_126() {
        assert_eq!(
            spawn_error(io::ErrorKind::PermissionDenied).exit_code(),
            126
        );
        assert_eq!(spawn_error(io::ErrorKind::Other).exit_code(), 126);
    }

    #[test]
    fn wait_errors_exit_1() {
        // An io::Error under a wait failure says nothing about the target
        // being found, even when it's NotFound.
        let wait_not_found = WaitError::Wait(io::ErrorKind::NotFound.into());
        assert_eq!(wait_error(wait_not_found).exit_code(), 1);
        assert_eq!(
            wait_error(WaitError::PidOutOfRange(u32::MAX)).exit_code(),
            1
        );
    }

    #[test]
    fn message_includes_each_source() {
        // main prints errors this way. The CLI tests can't make wait4 fail,
        // so this is what checks that its context reaches stderr.
        let err = anyhow::Error::new(wait_error(WaitError::PidOutOfRange(u32::MAX)));
        assert_eq!(
            format!("{err:#}"),
            "prog: cannot wait for it: pid 4294967295 does not fit in pid_t"
        );
    }
}
