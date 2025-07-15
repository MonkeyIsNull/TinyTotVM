pub mod mark_sweep;
pub mod no_gc;
pub mod stats;

use crate::vm::Value;
pub use stats::{GcStats, GcRef};

pub trait GcEngine: std::fmt::Debug + Send + Sync {
    fn alloc(&mut self, value: Value) -> GcRef;
    fn mark_from_roots(&mut self, roots: &[&Value]);
    fn sweep(&mut self) -> usize; // returns number of objects collected
    fn stats(&self) -> GcStats;
}

// Re-export GC implementations
pub use mark_sweep::MarkSweepGc;
pub use no_gc::NoGc;