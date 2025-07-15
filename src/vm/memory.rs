use std::collections::HashMap;
use crate::vm::{Value, VMError, VMResult};

/// Variable frame management for lexical scoping
pub trait VariableFrame {
    /// Get a variable from the current scope
    fn get_variable(&self, name: &str) -> VMResult<Value>;
    
    /// Set a variable in the current scope
    fn set_variable(&mut self, name: String, value: Value) -> VMResult<()>;
    
    /// Push a new variable frame for function calls
    fn push_variable_frame(&mut self);
    
    /// Pop a variable frame when returning from function
    fn pop_variable_frame(&mut self) -> VMResult<()>;
    
    /// Get the current frame depth
    fn frame_depth(&self) -> usize;
}

/// Memory management implementation for VM
pub struct VariableManager {
    pub variables: Vec<HashMap<String, Value>>, // call frame stack
}

impl VariableManager {
    pub fn new() -> Self {
        Self {
            variables: vec![HashMap::new()], // global frame
        }
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        let mut variables = Vec::with_capacity(capacity);
        variables.push(HashMap::new()); // global frame
        Self { variables }
    }
}

impl VariableFrame for VariableManager {
    fn get_variable(&self, name: &str) -> VMResult<Value> {
        self.variables
            .last()
            .ok_or(VMError::NoVariableScope)?
            .get(name)
            .cloned()
            .ok_or_else(|| VMError::UndefinedVariable(name.to_string()))
    }
    
    fn set_variable(&mut self, name: String, value: Value) -> VMResult<()> {
        self.variables
            .last_mut()
            .ok_or(VMError::NoVariableScope)?
            .insert(name, value);
        Ok(())
    }
    
    fn push_variable_frame(&mut self) {
        self.variables.push(HashMap::new());
    }
    
    fn pop_variable_frame(&mut self) -> VMResult<()> {
        if self.variables.len() <= 1 {
            return Err(VMError::RuntimeError("Cannot pop global variable frame".to_string()));
        }
        self.variables.pop();
        Ok(())
    }
    
    fn frame_depth(&self) -> usize {
        self.variables.len()
    }
}

/// Exception handling support
#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    pub catch_addr: usize,           // address to jump to on exception
    pub stack_size: usize,           // stack size when try block started
    pub call_stack_size: usize,      // call stack size when try block started
    pub variable_frames: usize,      // number of variable frames when try block started
}

/// Call stack management
pub struct CallStack {
    pub stack: Vec<usize>, // return addresses for CALL/RET
}

impl CallStack {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(64), // Pre-allocate call stack
        }
    }
    
    pub fn push(&mut self, addr: usize) {
        self.stack.push(addr);
    }
    
    pub fn pop(&mut self) -> VMResult<usize> {
        self.stack.pop().ok_or(VMError::CallStackUnderflow)
    }
    
    pub fn len(&self) -> usize {
        self.stack.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}