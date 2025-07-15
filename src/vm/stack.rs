use crate::vm::{Value, VMError, VMResult};

/// Stack operations for the VM
pub trait StackOps {
    /// Pop a value from the stack
    fn pop_stack(&mut self, operation: &str) -> VMResult<Value>;
    
    /// Peek at the top value on the stack without removing it
    fn peek_stack(&self, operation: &str) -> VMResult<&Value>;
    
    /// Check if the stack has enough items for an operation
    fn check_stack_size(&self, needed: usize, operation: &str) -> VMResult<()>;
    
    /// Get current stack size
    fn stack_size(&self) -> usize;
    
    /// Push a value onto the stack
    fn push_stack(&mut self, value: Value);
}

/// Stack utility functions
pub fn check_stack_items(available: usize, needed: usize, operation: &str) -> VMResult<()> {
    if available < needed {
        Err(VMError::InsufficientStackItems { 
            needed, 
            available 
        })
    } else {
        Ok(())
    }
}

/// Safe stack operations implementation
pub struct SafeStackOps<'a> {
    pub stack: &'a mut Vec<Value>,
}

impl<'a> SafeStackOps<'a> {
    pub fn new(stack: &'a mut Vec<Value>) -> Self {
        Self { stack }
    }
}

impl<'a> StackOps for SafeStackOps<'a> {
    fn pop_stack(&mut self, operation: &str) -> VMResult<Value> {
        self.stack.pop().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }
    
    fn peek_stack(&self, operation: &str) -> VMResult<&Value> {
        self.stack.last().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }
    
    fn check_stack_size(&self, needed: usize, operation: &str) -> VMResult<()> {
        check_stack_items(self.stack.len(), needed, operation)
    }
    
    fn stack_size(&self) -> usize {
        self.stack.len()
    }
    
    fn push_stack(&mut self, value: Value) {
        self.stack.push(value);
    }
}