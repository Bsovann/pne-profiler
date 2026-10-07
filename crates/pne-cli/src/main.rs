#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use pne_core::{ExitOutcome, RunStats};
use std::ffi::OsString;
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

fn main() -> ExitCode {
    // On a usage error or --help/--version, clap prints and exits the process
    // itself, so past this line we always have a valid subcommand.
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { cmd } => run(&cmd),
    }
}

/// Runs the target, prints its resource usage, and returns its exit status.
fn run(cmd: &[OsString]) -> ExitCode {
    // First word is the program, the rest are its arguments.
    let (prog, args) = cmd.split_first().expect("clap guarantees at least one");

    // Spawn the target. Inheriting stdio lets it share our terminal, so it
    // reads input and prints output as if run directly.
    let start = Instant::now();
    let spawned = Command::new(prog)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();
    let child = match spawned {
        Ok(child) => child,
        // The program isn't on PATH or doesn't exist.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "pne-profiler: {}: command not found",
                prog.to_string_lossy()
            );
            return ExitCode::from(127);
        }
        // Found but couldn't be executed, e.g. permission denied.
        Err(e) => {
            eprintln!("pne-profiler: {}: {e}", prog.to_string_lossy());
            return ExitCode::from(126);
        }
    };

    // Wait with wait4 rather than Child::wait so the kernel also hands back
    // the target's resource usage. This reaps the child, so `child` must not
    // be waited on again.
    let (outcome, resource_usage) = match pne_perf::wait4(child) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("pne-profiler: waiting for {}: {e}", prog.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    let stats = RunStats {
        wall_time: start.elapsed(),
        resource_usage,
    };
    // stderr keeps the report out of the target's stdout, so piping still works.
    eprint!("{}", summary(&stats));

    // Exit with the target's result so scripts and CI see its failures,
    // using the shell's conventions for the cases with no exit code.
    match outcome {
        // Normal exit: pass its code through.
        ExitOutcome::Exited(code) => ExitCode::from(code as u8),
        // Killed by a signal: report 128 + signal.
        ExitOutcome::Signaled(signal) => ExitCode::from(128 + signal as u8),
    }
}

/// Formats the end-of-run report.
fn summary(stats: &RunStats) -> String {
    let usage = &stats.resource_usage;
    let utilization = match stats.cpu_utilization() {
        Some(u) => format!("{:.0}%", u * 100.0),
        None => "n/a".to_string(),
    };
    format!(
        "\npne-profiler: resource usage\n\
         \x20 wall time   {:.3} s\n\
         \x20 user time   {:.3} s\n\
         \x20 sys time    {:.3} s\n\
         \x20 CPU usage   {utilization}\n\
         \x20 max RSS     {:.1} MiB\n",
        stats.wall_time.as_secs_f64(),
        usage.user_time.as_secs_f64(),
        usage.sys_time.as_secs_f64(),
        usage.max_rss.0 as f64 / (1024.0 * 1024.0),
    )
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
    use pne_core::{Bytes, ResourceUsage};
    use std::time::Duration;

    fn stats(wall_ms: u64, user_ms: u64, sys_ms: u64, max_rss: Bytes) -> RunStats {
        RunStats {
            wall_time: Duration::from_millis(wall_ms),
            resource_usage: ResourceUsage {
                user_time: Duration::from_millis(user_ms),
                sys_time: Duration::from_millis(sys_ms),
                max_rss,
            },
        }
    }

    #[test]
    fn summary_converts_times_percent_and_mib() {
        // 1.5 s of CPU over 2 s of wall time is 75%; 3 MiB + 512 KiB is 3.5 MiB.
        let report = summary(&stats(2000, 1250, 250, Bytes::from_kib(3 * 1024 + 512)));

        assert!(report.contains("wall time   2.000 s"), "{report}");
        assert!(report.contains("user time   1.250 s"), "{report}");
        assert!(report.contains("sys time    0.250 s"), "{report}");
        assert!(report.contains("CPU usage   75%"), "{report}");
        assert!(report.contains("max RSS     3.5 MiB"), "{report}");
    }

    #[test]
    fn summary_shows_parallel_cpu_usage_above_100_percent() {
        let report = summary(&stats(1000, 4000, 0, Bytes(0)));
        assert!(report.contains("CPU usage   400%"), "{report}");
    }

    #[test]
    fn summary_shows_na_for_zero_wall_time() {
        let report = summary(&stats(0, 0, 0, Bytes(0)));
        assert!(report.contains("CPU usage   n/a"), "{report}");
    }
}
