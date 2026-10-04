#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    // On a usage error or --help/--version, clap prints and exits the process
    // itself, so past this line we always have a valid subcommand.
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { cmd } => {
            // First word is the program, the rest are its arguments.
            let (prog, args) = cmd.split_first().expect("clap guarantees at least one");

            // Spawn the target and wait for it. Inheriting stdio lets it share
            // our terminal, so it reads input and prints output as if run directly.
            let status = Command::new(prog)
                .args(args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            // Exit with the target's result so scripts and CI see its failures,
            // using the shell's conventions for the cases with no exit code.
            match status {
                Ok(s) => match s.code() {
                    // Normal exit: pass its code through.
                    Some(code) => ExitCode::from(code as u8),
                    // No code means it was killed by a signal: report 128 + signal.
                    None => ExitCode::from(128 + s.signal().unwrap_or(0) as u8),
                },
                // The program isn't on PATH or doesn't exist.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!(
                        "pne-profiler: {}: command not found",
                        prog.to_string_lossy()
                    );
                    ExitCode::from(127)
                }
                // Found but couldn't be executed, e.g. permission denied.
                Err(e) => {
                    eprintln!("pne-profiler: {}: {e}", prog.to_string_lossy());
                    ExitCode::from(126)
                }
            }
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
