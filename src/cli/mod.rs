pub mod args;
pub mod commands;

use std::process;
use args::CliArgs;
use commands::execute_command;

/// Main entry point for the CLI application
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let args = match CliArgs::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    if let Err(e) = execute_command(&args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    Ok(())
}