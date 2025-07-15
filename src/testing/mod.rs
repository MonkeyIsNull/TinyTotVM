pub mod runner;
pub mod harness;

// Re-export commonly used types
pub use runner::{TestResult, run_vm_tests, report_test_results, report_gc_stats};