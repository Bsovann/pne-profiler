#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use std::ffi::OsString;

fn main() {
    println!("pne-profiler {}", env!("CARGO_PKG_VERSION"));
    
    
    /*
        Goals:
        1. Parse command line arguments
        2. use clap to parse the command line arguments
        3. print the parsed arguments
    */

    let cli = Cli::parse(); 
    match cli.command {
        Commands::Run { cmd } => {
            let (prog, arg) = cmd.split_first().expect("clap guarantees at least one"); 
            println!("Would run {:?} with {:?}!", prog, arg)
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "prof", version, about= "A lightweight HPC profiler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Run {
        #[arg(required = true, trailing_var_arg = false, allow_hyphen_values = true)]
        cmd: Vec<OsString>,
    }
}
