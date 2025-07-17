mod bytecode;
mod compiler;
mod lisp_compiler;
mod optimizer;
mod vm;
mod gc;
mod profiling;
mod concurrency;
mod testing;
mod cli;
mod ir;

// Re-export commonly used types from lib.rs for internal use
use tiny_tot_vm::{VMConfig, OutputMode, ProcState};

use std::process;

fn main() {
    if let Err(e) = cli::run_cli() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}