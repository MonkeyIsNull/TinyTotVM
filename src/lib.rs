// TinyTotVM - A Hybrid Virtual Machine Library
// 
// This library provides both stack-based and register-based virtual machine execution
// modes with BEAM-style concurrency, garbage collection, and comprehensive profiling capabilities.

pub mod vm;
pub mod gc;
pub mod profiling;
pub mod concurrency;
pub mod bytecode;
pub mod ir;
// pub mod testing;  // Temporarily disabled until VM is extracted

// Re-export commonly used types for convenience
pub use vm::{Value, OpCode, VMError, VMResult, ProcId, MessagePattern};
pub use gc::{GcEngine, GcStats, MarkSweepGc, NoGc};
pub use profiling::Profiler;
pub use concurrency::{Message, RestartStrategy, ChildType, Shutdown, ChildSpec, RestartPolicy, SupervisorSpec, ChildState, ProcessRegistry};
pub use ir::{RegInstr, RegValue, RegBlock, RegId};
// pub use testing::{TestResult, run_vm_tests, report_gc_stats};

// Configuration types
#[derive(Clone, Copy, Debug)]
pub enum OutputMode {
    PrettyTable,
    Plain,
}

#[derive(Clone, Debug)]
pub struct VMConfig {
    pub output_mode: OutputMode,
    pub run_tests: bool,
    pub gc_debug: bool,
    pub gc_stats: bool,
    pub debug_mode: bool,
    pub optimize_mode: bool,
    pub gc_type: String,
    pub trace_enabled: bool,
    pub profile_enabled: bool,
    pub smp_enabled: bool,
    pub trace_procs: bool,
    pub profile_procs: bool,
    pub use_ir: bool,
}

impl Default for VMConfig {
    fn default() -> Self {
        VMConfig {
            output_mode: OutputMode::PrettyTable,
            run_tests: false,
            gc_debug: false,
            gc_stats: false,
            debug_mode: false,
            optimize_mode: false,
            gc_type: "mark-sweep".to_string(),
            trace_enabled: false,
            profile_enabled: false,
            smp_enabled: true,
            trace_procs: false,
            profile_procs: false,
            use_ir: false,
        }
    }
}

// Process state enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcState {
    Ready,
    Running,
    Waiting,
    Exited,
}

// Public API traits for extensibility
// These are defined in main.rs and used by various components