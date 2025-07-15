use std::collections::HashMap;
use crate::vm::Value;
use crate::gc::{GcEngine, GcRef, GcStats};

// Mark and Sweep Garbage Collector
#[derive(Debug)]
pub struct MarkSweepGc {
    objects: HashMap<usize, (Value, bool)>, // id -> (value, marked)
    next_id: usize,
    stats: GcStats,
    debug_mode: bool,
}

impl MarkSweepGc {
    pub fn new(debug_mode: bool) -> Self {
        MarkSweepGc {
            objects: HashMap::new(),
            next_id: 0,
            stats: GcStats::default(),
            debug_mode,
        }
    }
}

impl GcEngine for MarkSweepGc {
    fn alloc(&mut self, value: Value) -> GcRef {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, (value, false));
        self.stats.total_allocated += 1;
        self.stats.current_allocated += 1;
        
        if self.debug_mode {
            println!("GC: Allocated object {} (total: {})", id, self.stats.current_allocated);
        }
        
        GcRef::new(id)
    }

    fn mark_from_roots(&mut self, _roots: &[&Value]) {
        // Mark all objects for now (simplified marking)
        for (_, (_, marked)) in self.objects.iter_mut() {
            *marked = true;
        }
        
        if self.debug_mode {
            println!("GC: Marked {} objects", self.objects.len());
        }
    }

    fn sweep(&mut self) -> usize {
        let initial_count = self.objects.len();
        self.objects.retain(|id, (_, marked)| {
            if *marked {
                true
            } else {
                if self.debug_mode {
                    println!("GC: Collecting object {}", id);
                }
                false
            }
        });
        
        // Reset marks for next collection
        for (_, (_, marked)) in self.objects.iter_mut() {
            *marked = false;
        }
        
        let collected = initial_count - self.objects.len();
        self.stats.total_freed += collected;
        self.stats.current_allocated -= collected;
        self.stats.collections_performed += 1;
        
        if self.debug_mode {
            println!("GC: Collected {} objects, {} remaining", collected, self.objects.len());
        }
        
        collected
    }

    fn stats(&self) -> GcStats {
        self.stats.clone()
    }
}