use crate::vm::Value;
use crate::gc::{GcEngine, GcRef, GcStats};

// No-op Garbage Collector (for testing and comparison)
#[derive(Debug)]
pub struct NoGc {
    next_id: usize,
    stats: GcStats,
}

impl NoGc {
    pub fn new() -> Self {
        NoGc {
            next_id: 0,
            stats: GcStats::default(),
        }
    }
}

impl GcEngine for NoGc {
    fn alloc(&mut self, _value: Value) -> GcRef {
        let id = self.next_id;
        self.next_id += 1;
        self.stats.total_allocated += 1;
        self.stats.current_allocated += 1;
        GcRef::new(id)
    }

    fn mark_from_roots(&mut self, _roots: &[&Value]) {
        // No-op
    }

    fn sweep(&mut self) -> usize {
        self.stats.collections_performed += 1;
        0 // Never collect anything
    }

    fn stats(&self) -> GcStats {
        self.stats.clone()
    }
}