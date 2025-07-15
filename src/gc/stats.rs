#[derive(Debug, Clone)]
pub struct GcStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_allocated: usize,
    pub collections_performed: usize,
}

impl Default for GcStats {
    fn default() -> Self {
        Self {
            total_allocated: 0,
            total_freed: 0,
            current_allocated: 0,
            collections_performed: 0,
        }
    }
}

// GC Reference wrapper
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GcRef {
    id: usize,
    generation: usize,
}

impl GcRef {
    pub fn new(id: usize) -> Self {
        GcRef { id, generation: 0 }
    }
}