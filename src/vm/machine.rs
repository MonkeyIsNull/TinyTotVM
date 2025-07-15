use std::collections::HashMap;
use std::time::Duration;
use colored::*;
use crate::vm::{Value, OpCode, VMError, VMResult, ExceptionHandler};
use crate::gc::{GcEngine, GcStats, MarkSweepGc, NoGc};
use crate::profiling::Profiler;
use crate::bytecode::parse_program;

pub struct VM {
    pub stack: Vec<Value>,
    pub instructions: Vec<OpCode>,
    pub ip: usize,                              // instruction pointer
    pub call_stack: Vec<usize>,                 // return addresses for CALL/RET
    pub variables: Vec<HashMap<String, Value>>, // call frame stack
    // Exception handling
    pub try_stack: Vec<ExceptionHandler>,       // stack of try blocks
    // Module system
    pub exports: HashMap<String, Value>,        // exported symbols from this module
    pub loaded_modules: HashMap<String, HashMap<String, Value>>, // module_path -> exports
    pub loading_stack: Vec<String>,             // for circular dependency detection
    // Closure support
    pub lambda_captures: HashMap<String, Value>, // variables captured for current lambda
    // Performance improvements
    pub max_stack_size: usize,                  // Track maximum stack usage
    pub instruction_count: usize,               // Count of executed instructions
    // Debugging support
    pub debug_mode: bool,
    pub breakpoints: Vec<usize>,
    // Garbage Collection
    pub gc_engine: Box<dyn GcEngine>,           // Pluggable GC engine
    pub _gc_stats_enabled: bool,                 // Whether to show GC stats
    // Profiling and Tracing
    pub profiler: Option<Profiler>,             // Optional profiler for performance analysis
    pub trace_enabled: bool,                    // Whether to enable tracing
}

impl VM {
    pub fn new(instructions: Vec<OpCode>) -> Self {
        Self::new_with_gc(instructions, "mark-sweep", false, false)
    }

    #[allow(dead_code)]
    pub fn new_with_debug(instructions: Vec<OpCode>, debug_mode: bool) -> Self {
        Self::new_with_gc(instructions, "mark-sweep", debug_mode, false)
    }

    pub fn new_with_gc(instructions: Vec<OpCode>, gc_type: &str, debug_mode: bool, gc_stats_enabled: bool) -> Self {
        Self::new_with_config(instructions, gc_type, debug_mode, gc_stats_enabled, false, false)
    }

    pub fn new_with_config(instructions: Vec<OpCode>, gc_type: &str, debug_mode: bool, gc_stats_enabled: bool, trace_enabled: bool, profile_enabled: bool) -> Self {
        let gc_engine: Box<dyn GcEngine> = match gc_type {
            "no-gc" => Box::new(NoGc::new()),
            "mark-sweep" => Box::new(MarkSweepGc::new(debug_mode)),
            _ => Box::new(MarkSweepGc::new(debug_mode)), // Default to mark-sweep
        };

        VM {
            stack: Vec::with_capacity(1024), // Pre-allocate stack capacity
            instructions,
            ip: 0,
            call_stack: Vec::with_capacity(64), // Pre-allocate call stack
            variables: vec![HashMap::new()], // global frame
            try_stack: Vec::new(),
            exports: HashMap::new(),
            loaded_modules: HashMap::new(),
            loading_stack: Vec::new(),
            lambda_captures: HashMap::new(),
            max_stack_size: 0,
            instruction_count: 0,
            debug_mode,
            breakpoints: Vec::new(),
            gc_engine,
            _gc_stats_enabled: gc_stats_enabled,
            profiler: if profile_enabled { Some(Profiler::new()) } else { None },
            trace_enabled,
        }
    }

    #[allow(dead_code)]
    pub fn add_breakpoint(&mut self, address: usize) {
        if !self.breakpoints.contains(&address) {
            self.breakpoints.push(address);
            self.breakpoints.sort();
        }
    }

    #[allow(dead_code)]
    pub fn remove_breakpoint(&mut self, address: usize) {
        self.breakpoints.retain(|&x| x != address);
    }

    pub fn get_stats(&self) -> (usize, usize, usize) {
        (self.instruction_count, self.max_stack_size, self.stack.len())
    }

    pub fn get_gc_stats(&self) -> GcStats {
        self.gc_engine.stats()
    }

    #[allow(dead_code)]
    pub fn trigger_gc(&mut self) {
        // Collect roots from stack and variables
        let mut roots: Vec<&Value> = Vec::new();
        
        // Add stack values as roots
        for value in &self.stack {
            roots.push(value);
        }
        
        // Add variables as roots
        for frame in &self.variables {
            for (_name, value) in frame {
                roots.push(value);
            }
        }
        
        // Mark from roots
        self.gc_engine.mark_from_roots(&roots);
        
        // Sweep unreachable objects
        let collected = self.gc_engine.sweep();
        
        if self.debug_mode {
            println!("GC triggered: collected {} objects", collected);
        }
    }

    // Safe stack operations
    fn pop_stack(&mut self, operation: &str) -> VMResult<Value> {
        self.stack.pop().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }

    fn peek_stack(&self, operation: &str) -> VMResult<&Value> {
        self.stack.last().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }

    fn check_stack_size(&self, needed: usize, _operation: &str) -> VMResult<()> {
        if self.stack.len() < needed {
            Err(VMError::InsufficientStackItems { 
                needed, 
                available: self.stack.len() 
            })
        } else {
            Ok(())
        }
    }

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

    fn pop_call_stack(&mut self) -> VMResult<usize> {
        self.call_stack.pop().ok_or(VMError::CallStackUnderflow)
    }

    fn pop_variable_frame(&mut self) -> VMResult<()> {
        if self.variables.len() <= 1 {
            Err(VMError::NoVariableScope)
        } else {
            self.variables.pop();
            Ok(())
        }
    }

    // Exception handling methods
    fn push_exception_handler(&mut self, catch_addr: usize) {
        let handler = ExceptionHandler {
            catch_addr,
            stack_size: self.stack.len(),
            call_stack_size: self.call_stack.len(),
            variable_frames: self.variables.len(),
        };
        self.try_stack.push(handler);
    }

    fn pop_exception_handler(&mut self) -> Option<ExceptionHandler> {
        self.try_stack.pop()
    }

    fn unwind_to_exception_handler(&mut self, handler: &ExceptionHandler) {
        // Unwind stack to the state when try block started
        self.stack.truncate(handler.stack_size);
        
        // Unwind call stack
        self.call_stack.truncate(handler.call_stack_size);
        
        // Unwind variable frames
        self.variables.truncate(handler.variable_frames);
    }

    fn throw_exception(&mut self, exception: Value) -> VMResult<()> {
        if let Some(handler) = self.pop_exception_handler() {
            // Unwind to the try block state
            self.unwind_to_exception_handler(&handler);
            
            // Push the exception onto the stack for the catch block
            self.stack.push(exception);
            
            // Jump to the catch block
            self.ip = handler.catch_addr;
            Ok(())
        } else {
            // No exception handler found, convert to VM error
            match exception {
                Value::Exception { message, .. } => {
                    Err(VMError::ParseError { line: self.ip, instruction: format!("Unhandled exception: {}", message) })
                }
                _ => {
                    Err(VMError::ParseError { line: self.ip, instruction: format!("Unhandled exception: {:?}", exception) })
                }
            }
        }
    }

    pub fn run(&mut self) -> VMResult<()> {
        while self.ip < self.instructions.len() {
            // Performance tracking
            self.instruction_count += 1;
            if self.stack.len() > self.max_stack_size {
                self.max_stack_size = self.stack.len();
            }

            // Profiling support
            if let Some(ref mut profiler) = self.profiler {
                profiler.record_instruction();
                profiler.update_stack_depth(self.stack.len());
            }

            // Tracing support
            if self.trace_enabled {
                let instruction = &self.instructions[self.ip];
                let indent = if let Some(ref profiler) = self.profiler {
                    "  ".repeat(profiler.call_depth)
                } else {
                    String::new()
                };
                println!("{} {}{} @ {}", 
                         "[trace]",
                         indent, 
                         format!("{:?}", instruction),
                         format!("0x{:04X}", self.ip));
            }

            // Debugging support
            if self.debug_mode {
                println!("IP: {}, Instruction: {:?}, Stack size: {}", 
                    self.ip, self.instructions[self.ip], self.stack.len());
            }

            // Breakpoint support
            if self.breakpoints.contains(&self.ip) {
                println!("Breakpoint hit at instruction {}: {:?}", 
                    self.ip, self.instructions[self.ip]);
                println!("Stack: {:?}", self.stack);
                println!("Variables: {:?}", self.variables.last());
                // In a real debugger, we'd wait for user input here
            }

            let instruction = &self.instructions[self.ip].clone();
            
            // Store original IP to detect jumps
            let original_ip = self.ip;
            
            // Check for HALT instruction first
            if matches!(instruction, OpCode::Halt) {
                break;
            }
            
            // Execute instruction and catch VM errors in try blocks
            match self.execute_instruction_safe(instruction) {
                Ok(()) => {
                    // Only increment IP if instruction didn't change it
                    if self.ip == original_ip {
                        self.ip += 1;
                    }
                }
                Err(vm_error) => {
                    // If we're in a try block, convert VM error to exception
                    if !self.try_stack.is_empty() {
                        let exception = Value::Exception {
                            message: vm_error.to_string(),
                            stack_trace: vec![format!("at instruction {}", self.ip)]
                        };
                        self.throw_exception(exception)?;
                        continue;
                    } else {
                        return Err(vm_error);
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_instruction_safe(&mut self, instruction: &OpCode) -> VMResult<()> {
        match instruction {
                OpCode::PushInt(n) => self.stack.push(Value::Int(*n)),
                OpCode::PushFloat(f) => self.stack.push(Value::Float(*f)),
                OpCode::PushStr(s) => self.stack.push(Value::Str(s.clone())),
                OpCode::PushBool(b) => self.stack.push(Value::Bool(*b)),
                OpCode::Add => {
                    let b = self.pop_stack("ADD")?;
                    let a = self.pop_stack("ADD")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x + y)),
                        // Type coercion: int + float = float
                        (Value::Int(x), Value::Float(y)) => self.stack.push(Value::Float(*x as f64 + y)),
                        (Value::Float(x), Value::Int(y)) => self.stack.push(Value::Float(x + *y as f64)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x + y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two numbers (int or float)".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "ADD".to_string() 
                        }),
                    }
                }
                OpCode::AddF => {
                    let b = self.pop_stack("ADD_F")?;
                    let a = self.pop_stack("ADD_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x + y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "ADD_F".to_string() 
                        }),
                    }
                }
                OpCode::SubF => {
                    let b = self.pop_stack("SUB_F")?;
                    let a = self.pop_stack("SUB_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x - y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "SUB_F".to_string() 
                        }),
                    }
                }
                OpCode::MulF => {
                    let b = self.pop_stack("MUL_F")?;
                    let a = self.pop_stack("MUL_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x * y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "MUL_F".to_string() 
                        }),
                    }
                }
                OpCode::DivF => {
                    let b = self.pop_stack("DIV_F")?;
                    let a = self.pop_stack("DIV_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            if *y == 0.0 {
                                return Err(VMError::TypeMismatch { 
                                    expected: "non-zero divisor".to_string(), 
                                    got: "zero".to_string(), 
                                    operation: "DIV_F".to_string() 
                                });
                            }
                            self.stack.push(Value::Float(x / y));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "DIV_F".to_string() 
                        }),
                    }
                }
                OpCode::Concat => {
                    let b = self.pop_stack("CONCAT")?;
                    let a = self.pop_stack("CONCAT")?;
                    match (&a, &b) {
                        (Value::Str(x), Value::Str(y)) => self.stack.push(Value::Str(x.clone() + y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two strings".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "CONCAT".to_string() 
                        }),
                    }
                }
                OpCode::Print => {
                    let val = self.pop_stack("PRINT")?;
                    println!("{}", val);
                }
                OpCode::Jmp(target) => {
                    self.ip = *target;
                }
                OpCode::Jz(target) => {
                    let val = self.pop_stack("JZ")?;
                    let is_zero = match val {
                        Value::Int(0) => true,
                        Value::Bool(false) => true,
                        Value::Null => true,
                        _ => false,
                    };
                    if is_zero {
                        self.ip = *target;
                    }
                }
                OpCode::Call{ addr, params } => {
                    self.check_stack_size(params.len(), "CALL")?;
                    
                    // Create function name for profiling/tracing
                    let function_name = format!("fn@0x{:04X}", addr);
                    
                    // Function call tracing
                    if self.trace_enabled {
                        let indent = if let Some(ref profiler) = self.profiler {
                            "  ".repeat(profiler.call_depth)
                        } else {
                            String::new()
                        };
                        println!("{} {}CALL {} with {} params", 
                                 "[trace]".bright_blue(),
                                 indent, 
                                 function_name.yellow(),
                                 format!("{}", params.len()).green());
                    }
                    
                    // Function profiling
                    if let Some(ref mut profiler) = self.profiler {
                        profiler.start_function(function_name);
                    }
                    
                    self.call_stack.push(self.ip + 1);
                    let mut frame = HashMap::new();
                    for name in params.iter().rev() {
                        let value = self.pop_stack("CALL")?;
                        frame.insert(name.clone(), value);
                    }
                    self.variables.push(frame);
                    self.ip = *addr;
                }
                OpCode::Ret => {
                    // Function return tracing and profiling
                    if let Some(ref mut profiler) = self.profiler {
                        if let Some(function_name) = profiler.end_function() {
                            if self.trace_enabled {
                                let indent = "  ".repeat(profiler.call_depth);
                                let return_value = if !self.stack.is_empty() {
                                    format!(" → {:?}", self.stack.last().unwrap())
                                } else {
                                    String::new()
                                };
                                println!("{} {}RETURN from {}{}", 
                                         "[trace]".bright_blue(),
                                         indent, 
                                         function_name.yellow(),
                                         return_value.green());
                            }
                        }
                    }
                    
                    self.pop_variable_frame()?;
                    self.ip = self.pop_call_stack()?;
                }
                OpCode::Sub => {
                    let b = self.pop_stack("SUB")?;
                    let a = self.pop_stack("SUB")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x - y)),
                        // Type coercion: mixed int/float = float
                        (Value::Int(x), Value::Float(y)) => self.stack.push(Value::Float(*x as f64 - y)),
                        (Value::Float(x), Value::Int(y)) => self.stack.push(Value::Float(x - *y as f64)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x - y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two numbers (int or float)".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "SUB".to_string() 
                        }),
                    }
                }
                OpCode::Mul => {
                    let b = self.pop_stack("MUL")?;
                    let a = self.pop_stack("MUL")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x * y)),
                        (Value::Int(x), Value::Float(y)) => self.stack.push(Value::Float(*x as f64 * y)),
                        (Value::Float(x), Value::Int(y)) => self.stack.push(Value::Float(x * *y as f64)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x * y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two numbers (int or float)".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "MUL".to_string() 
                        }),
                    }
                }
                OpCode::Div => {
                    let b = self.pop_stack("DIV")?;
                    let a = self.pop_stack("DIV")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if *y == 0 {
                                return Err(VMError::DivisionByZero);
                            }
                            self.stack.push(Value::Int(x / y));
                        },
                        (Value::Int(x), Value::Float(y)) => {
                            if *y == 0.0 {
                                return Err(VMError::DivisionByZero);
                            }
                            self.stack.push(Value::Float(*x as f64 / y));
                        },
                        (Value::Float(x), Value::Int(y)) => {
                            if *y == 0 {
                                return Err(VMError::DivisionByZero);
                            }
                            self.stack.push(Value::Float(x / *y as f64));
                        },
                        (Value::Float(x), Value::Float(y)) => {
                            if *y == 0.0 {
                                return Err(VMError::DivisionByZero);
                            }
                            self.stack.push(Value::Float(x / y));
                        },
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two numbers (int or float)".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "DIV".to_string() 
                        }),
                    }
                }
                OpCode::Dup => {
                    let val = self.peek_stack("DUP")?.clone();
                    self.stack.push(val);
                }
                OpCode::Store(name) => {
                    let val = self.pop_stack("STORE")?;
                    self.set_variable(name.clone(), val)?;
                }
                OpCode::Load(name) => {
                    let val = self.get_variable(&name)?;
                    self.stack.push(val);
                }
                OpCode::Delete(name) => {
                    let removed = self
                        .variables
                        .last_mut()
                        .ok_or(VMError::NoVariableScope)?
                        .remove(name);
                    if removed.is_none() {
                        eprintln!("Warning: tried to DELETE unknown variable '{}'", name);
                    }
                }
                OpCode::Eq => {
                    let b = self.pop_stack("EQ")?;
                    let a = self.pop_stack("EQ")?;
                    let result = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => x == y,
                        (Value::Float(x), Value::Float(y)) => (x - y).abs() < f64::EPSILON,
                        (Value::Str(x), Value::Str(y)) => x == y,
                        (Value::Bool(x), Value::Bool(y)) => x == y,
                        (Value::Null, Value::Null) => true,
                        (Value::Function { addr: addr1, params: params1 }, Value::Function { addr: addr2, params: params2 }) => {
                            addr1 == addr2 && params1 == params2
                        },
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "values of the same type".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "EQ".to_string() 
                        }),
                    };
                    self.stack.push(Value::Int(if result { 1 } else { 0 }));
                }
                OpCode::Gt => {
                    let b = self.pop_stack("GT")?;
                    let a = self.pop_stack("GT")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x > y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GT".to_string() 
                        }),
                    }
                }
                OpCode::Lt => {
                    let b = self.pop_stack("LT")?;
                    let a = self.pop_stack("LT")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x < y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LT".to_string() 
                        }),
                    }
                }
                OpCode::Ne => {
                    let b = self.pop_stack("NE")?;
                    let a = self.pop_stack("NE")?;
                    let result = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => x != y,
                        (Value::Float(x), Value::Float(y)) => (x - y).abs() >= f64::EPSILON,
                        (Value::Str(x), Value::Str(y)) => x != y,
                        (Value::Bool(x), Value::Bool(y)) => x != y,
                        (Value::Null, Value::Null) => false,
                        (Value::Function { addr: addr1, params: params1 }, Value::Function { addr: addr2, params: params2 }) => {
                            addr1 != addr2 || params1 != params2
                        },
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "values of the same type".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "NE".to_string() 
                        }),
                    };
                    self.stack.push(Value::Int(if result { 1 } else { 0 }));
                }
                OpCode::Ge => {
                    let b = self.pop_stack("GE")?;
                    let a = self.pop_stack("GE")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x >= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GE".to_string() 
                        }),
                    }
                }
                OpCode::Le => {
                    let b = self.pop_stack("LE")?;
                    let a = self.pop_stack("LE")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x <= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LE".to_string() 
                        }),
                    }
                }
                OpCode::EqF => {
                    let b = self.pop_stack("EQ_F")?;
                    let a = self.pop_stack("EQ_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if (x - y).abs() < f64::EPSILON { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "EQ_F".to_string() 
                        }),
                    }
                }
                OpCode::NeF => {
                    let b = self.pop_stack("NE_F")?;
                    let a = self.pop_stack("NE_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if (x - y).abs() >= f64::EPSILON { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "NE_F".to_string() 
                        }),
                    }
                }
                OpCode::GtF => {
                    let b = self.pop_stack("GT_F")?;
                    let a = self.pop_stack("GT_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x > y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GT_F".to_string() 
                        }),
                    }
                }
                OpCode::LtF => {
                    let b = self.pop_stack("LT_F")?;
                    let a = self.pop_stack("LT_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x < y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LT_F".to_string() 
                        }),
                    }
                }
                OpCode::GeF => {
                    let b = self.pop_stack("GE_F")?;
                    let a = self.pop_stack("GE_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x >= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GE_F".to_string() 
                        }),
                    }
                }
                OpCode::LeF => {
                    let b = self.pop_stack("LE_F")?;
                    let a = self.pop_stack("LE_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x <= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LE_F".to_string() 
                        }),
                    }
                }
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::False => self.stack.push(Value::Bool(false)),
                OpCode::Not => {
                    let val = self.pop_stack("NOT")?;
                    let result = match val {
                        Value::Bool(b) => !b,
                        Value::Int(i) => i == 0,
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "Bool or Int".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "NOT".to_string() 
                        }),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::And => {
                    let b = self.pop_stack("AND")?;
                    let a = self.pop_stack("AND")?;
                    let result = match (&a, &b) {
                        (Value::Bool(x), Value::Bool(y)) => *x && *y,
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two Booleans".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "AND".to_string() 
                        }),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::Or => {
                    let b = self.pop_stack("OR")?;
                    let a = self.pop_stack("OR")?;
                    let result = match (&a, &b) {
                        (Value::Bool(x), Value::Bool(y)) => *x || *y,
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two Booleans".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "OR".to_string() 
                        }),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::Null => {
                    self.stack.push(Value::Null);
                }
                OpCode::MakeList(n) => {
                    self.check_stack_size(*n, "MAKE_LIST")?;
                    let mut list = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        list.push(self.pop_stack("MAKE_LIST")?);
                    }
                    list.reverse();
                    self.stack.push(Value::List(list));
                }
                OpCode::Len => {
                    let val = self.pop_stack("LEN")?;
                    match val {
                        Value::List(l) => self.stack.push(Value::Int(l.len() as i64)),
                        Value::Object(o) => self.stack.push(Value::Int(o.len() as i64)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a list or object".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "LEN".to_string() 
                        }),
                    }
                }
                OpCode::Index => {
                    let index = match self.pop_stack("INDEX")? {
                        Value::Int(i) => i as usize,
                        val => return Err(VMError::TypeMismatch { 
                            expected: "an integer index".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "INDEX".to_string() 
                        }),
                    };
                    let list = match self.pop_stack("INDEX")? {
                        Value::List(l) => l,
                        val => return Err(VMError::TypeMismatch { 
                            expected: "a list".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "INDEX".to_string() 
                        }),
                    };
                    if index >= list.len() {
                        return Err(VMError::IndexOutOfBounds { index, length: list.len() });
                    }
                    self.stack.push(list[index].clone());
                }
                OpCode::MakeObject => {
                    let obj = HashMap::new();
                    self.stack.push(Value::Object(obj));
                }
                OpCode::SetField(field_name) => {
                    let value = self.pop_stack("SET_FIELD")?;
                    let obj = self.pop_stack("SET_FIELD")?;
                    match obj {
                        Value::Object(mut map) => {
                            map.insert(field_name.clone(), value);
                            self.stack.push(Value::Object(map));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "SET_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::GetField(field_name) => {
                    let obj = self.pop_stack("GET_FIELD")?;
                    match obj {
                        Value::Object(map) => {
                            let value = map.get(field_name).cloned().unwrap_or(Value::Null);
                            self.stack.push(value);
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "GET_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::HasField(field_name) => {
                    let obj = self.pop_stack("HAS_FIELD")?;
                    match obj {
                        Value::Object(map) => {
                            let has_field = map.contains_key(field_name);
                            self.stack.push(Value::Int(if has_field { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "HAS_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::DeleteField(field_name) => {
                    let obj = self.pop_stack("DELETE_FIELD")?;
                    match obj {
                        Value::Object(mut map) => {
                            map.remove(field_name);
                            self.stack.push(Value::Object(map));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "DELETE_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::Keys => {
                    let obj = self.pop_stack("KEYS")?;
                    match obj {
                        Value::Object(map) => {
                            let keys: Vec<Value> = map.keys().map(|k| Value::Str(k.clone())).collect();
                            self.stack.push(Value::List(keys));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "KEYS".to_string() 
                        }),
                    }
                }
                OpCode::MakeFunction { addr, params } => {
                    let function = Value::Function { addr: *addr, params: params.clone() };
                    self.stack.push(function);
                }
                OpCode::MakeLambda { addr, params } => {
                    // Create closure with currently captured variables
                    let closure = Value::Closure { 
                        addr: *addr, 
                        params: params.clone(),
                        captured: self.lambda_captures.clone() 
                    };
                    self.stack.push(closure);
                    
                    // Clear captures for next lambda
                    self.lambda_captures.clear();
                }
                OpCode::Capture(var_name) => {
                    // Capture current value of variable for lambda
                    let value = self.get_variable(var_name)?.clone();
                    self.lambda_captures.insert(var_name.clone(), value);
                }
                OpCode::CallFunction => {
                    let function = self.pop_stack("CALL_FUNCTION")?;
                    match function {
                        Value::Function { addr, params } => {
                            // Check if we have enough arguments on the stack
                            self.check_stack_size(params.len(), "CALL_FUNCTION")?;
                            
                            // Save return address
                            self.call_stack.push(self.ip + 1);
                            
                            // Create new variable frame for function parameters
                            let mut frame = HashMap::new();
                            for name in params.iter().rev() {
                                let value = self.pop_stack("CALL_FUNCTION")?;
                                frame.insert(name.clone(), value);
                            }
                            self.variables.push(frame);
                            
                            // Jump to function
                            self.ip = addr;
                        }
                        Value::Closure { addr, params, captured } => {
                            // Check if we have enough arguments on the stack
                            self.check_stack_size(params.len(), "CALL_FUNCTION")?;
                            
                            // Save return address
                            self.call_stack.push(self.ip + 1);
                            
                            // Create new variable frame with captured variables and parameters
                            let mut frame = captured; // Start with captured environment
                            for name in params.iter().rev() {
                                let value = self.pop_stack("CALL_FUNCTION")?;
                                frame.insert(name.clone(), value); // Parameters override captured vars
                            }
                            self.variables.push(frame);
                            
                            // Jump to closure body
                            self.ip = addr;
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a function or closure".to_string(), 
                            got: format!("{:?}", function), 
                            operation: "CALL_FUNCTION".to_string() 
                        }),
                    }
                }
                OpCode::ReadFile => {
                    let val = self.pop_stack("read_file")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::read_to_string(&filename) {
                                Ok(content) => self.stack.push(Value::Str(content)),
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a string filename".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "READ_FILE".to_string() 
                        }),
                    }
                }
                OpCode::WriteFile => {
                    let filename = self.pop_stack("WRITE_FILE")?;
                    let content = self.pop_stack("WRITE_FILE")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(body)) => {
                            if let Err(e) = std::fs::write(&fname, &body) {
                                return Err(VMError::FileError { 
                                    filename: fname, 
                                    error: e.to_string() 
                                });
                            }
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (filename, content)".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "WRITE_FILE".to_string() 
                        }),
                    }
                }
                // Enhanced I/O operations
                OpCode::ReadLine => {
                    use std::io::{self, BufRead};
                    let stdin = io::stdin();
                    let mut line = String::new();
                    match stdin.lock().read_line(&mut line) {
                        Ok(_) => {
                            // Remove trailing newline
                            if line.ends_with('\n') {
                                line.pop();
                                if line.ends_with('\r') {
                                    line.pop();
                                }
                            }
                            self.stack.push(Value::Str(line));
                        }
                        Err(e) => return Err(VMError::FileError { 
                            filename: "stdin".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::ReadChar => {
                    use std::io::{self, Read};
                    let mut stdin = io::stdin();
                    let mut buffer = [0; 1];
                    match stdin.read_exact(&mut buffer) {
                        Ok(_) => {
                            let ch = buffer[0] as char;
                            self.stack.push(Value::Str(ch.to_string()));
                        }
                        Err(e) => return Err(VMError::FileError { 
                            filename: "stdin".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::ReadInput => {
                    use std::io::{self, Read};
                    let mut stdin = io::stdin();
                    let mut buffer = String::new();
                    match stdin.read_to_string(&mut buffer) {
                        Ok(_) => self.stack.push(Value::Str(buffer)),
                        Err(e) => return Err(VMError::FileError { 
                            filename: "stdin".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::AppendFile => {
                    let filename = self.pop_stack("APPEND_FILE")?;
                    let content = self.pop_stack("APPEND_FILE")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(body)) => {
                            use std::fs::OpenOptions;
                            use std::io::Write;
                            match OpenOptions::new().create(true).append(true).open(&fname) {
                                Ok(mut file) => {
                                    if let Err(e) = file.write_all(body.as_bytes()) {
                                        return Err(VMError::FileError { 
                                            filename: fname, 
                                            error: e.to_string() 
                                        });
                                    }
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: fname, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (filename, content)".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "APPEND_FILE".to_string() 
                        }),
                    }
                }
                OpCode::FileExists => {
                    let val = self.pop_stack("FILE_EXISTS")?;
                    match val {
                        Value::Str(filename) => {
                            let exists = std::path::Path::new(&filename).exists();
                            self.stack.push(Value::Bool(exists));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "FILE_EXISTS".to_string() 
                        }),
                    }
                }
                OpCode::FileSize => {
                    let val = self.pop_stack("FILE_SIZE")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::metadata(&filename) {
                                Ok(metadata) => self.stack.push(Value::Int(metadata.len() as i64)),
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "FILE_SIZE".to_string() 
                        }),
                    }
                }
                OpCode::DeleteFile => {
                    let val = self.pop_stack("DELETE_FILE")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::remove_file(&filename) {
                                Ok(_) => {}
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DELETE_FILE".to_string() 
                        }),
                    }
                }
                OpCode::ListDir => {
                    let val = self.pop_stack("LIST_DIR")?;
                    match val {
                        Value::Str(dirname) => {
                            match std::fs::read_dir(&dirname) {
                                Ok(entries) => {
                                    let mut files = Vec::new();
                                    for entry in entries {
                                        match entry {
                                            Ok(entry) => {
                                                if let Some(name) = entry.file_name().to_str() {
                                                    files.push(Value::Str(name.to_string()));
                                                }
                                            }
                                            Err(e) => return Err(VMError::FileError { 
                                                filename: dirname, 
                                                error: e.to_string() 
                                            }),
                                        }
                                    }
                                    self.stack.push(Value::List(files));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: dirname, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (directory name)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "LIST_DIR".to_string() 
                        }),
                    }
                }
                OpCode::ReadBytes => {
                    let val = self.pop_stack("read_bytes")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::read(&filename) {
                                Ok(bytes) => self.stack.push(Value::Bytes(bytes)),
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "read_bytes".to_string() 
                        }),
                    }
                }
                OpCode::WriteBytes => {
                    let filename = self.pop_stack("WRITE_BYTES")?;
                    let content = self.pop_stack("WRITE_BYTES")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Bytes(bytes)) => {
                            if let Err(e) = std::fs::write(&fname, &bytes) {
                                return Err(VMError::FileError { 
                                    filename: fname, 
                                    error: e.to_string() 
                                });
                            }
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "string (filename) and bytes".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "WRITE_BYTES".to_string() 
                        }),
                    }
                }
                // Environment and system operations
                OpCode::GetEnv => {
                    let val = self.pop_stack("GET_ENV")?;
                    match val {
                        Value::Str(var_name) => {
                            match std::env::var(&var_name) {
                                Ok(value) => self.stack.push(Value::Str(value)),
                                Err(_) => self.stack.push(Value::Null),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (variable name)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "GET_ENV".to_string() 
                        }),
                    }
                }
                OpCode::SetEnv => {
                    let value = self.pop_stack("SET_ENV")?;
                    let var_name = self.pop_stack("SET_ENV")?;
                    match (var_name, value) {
                        (Value::Str(name), Value::Str(val)) => {
                            std::env::set_var(&name, &val);
                        }
                        (n, v) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (variable name, value)".to_string(), 
                            got: format!("{:?}, {:?}", n, v), 
                            operation: "SET_ENV".to_string() 
                        }),
                    }
                }
                OpCode::GetArgs => {
                    let args: Vec<Value> = std::env::args()
                        .map(|arg| Value::Str(arg))
                        .collect();
                    self.stack.push(Value::List(args));
                }
                OpCode::Exec => {
                    let args = self.pop_stack("EXEC")?;
                    let command = self.pop_stack("EXEC")?;
                    match (command, args) {
                        (Value::Str(cmd), Value::List(arg_list)) => {
                            use std::process::Command;
                            let mut cmd_obj = Command::new(&cmd);
                            for arg in arg_list {
                                if let Value::Str(arg_str) = arg {
                                    cmd_obj.arg(arg_str);
                                } else {
                                    return Err(VMError::TypeMismatch { 
                                        expected: "list of strings (arguments)".to_string(), 
                                        got: format!("{:?}", arg), 
                                        operation: "EXEC".to_string() 
                                    });
                                }
                            }
                            match cmd_obj.status() {
                                Ok(status) => {
                                    let exit_code = status.code().unwrap_or(-1);
                                    self.stack.push(Value::Int(exit_code as i64));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: cmd, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (c, a) => return Err(VMError::TypeMismatch { 
                            expected: "string (command) and list (arguments)".to_string(), 
                            got: format!("{:?}, {:?}", c, a), 
                            operation: "EXEC".to_string() 
                        }),
                    }
                }
                OpCode::ExecCapture => {
                    let args = self.pop_stack("EXEC_CAPTURE")?;
                    let command = self.pop_stack("EXEC_CAPTURE")?;
                    match (command, args) {
                        (Value::Str(cmd), Value::List(arg_list)) => {
                            use std::process::Command;
                            let mut cmd_obj = Command::new(&cmd);
                            for arg in arg_list {
                                if let Value::Str(arg_str) = arg {
                                    cmd_obj.arg(arg_str);
                                } else {
                                    return Err(VMError::TypeMismatch { 
                                        expected: "list of strings (arguments)".to_string(), 
                                        got: format!("{:?}", arg), 
                                        operation: "EXEC_CAPTURE".to_string() 
                                    });
                                }
                            }
                            match cmd_obj.output() {
                                Ok(output) => {
                                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                    let exit_code = output.status.code().unwrap_or(-1);
                                    
                                    // Create result object
                                    let mut result = HashMap::new();
                                    result.insert("stdout".to_string(), Value::Str(stdout));
                                    result.insert("stderr".to_string(), Value::Str(stderr));
                                    result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
                                    self.stack.push(Value::Object(result));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: cmd, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (c, a) => return Err(VMError::TypeMismatch { 
                            expected: "string (command) and list (arguments)".to_string(), 
                            got: format!("{:?}, {:?}", c, a), 
                            operation: "EXEC_CAPTURE".to_string() 
                        }),
                    }
                }
                OpCode::Exit => {
                    let val = self.pop_stack("EXIT")?;
                    match val {
                        Value::Int(code) => {
                            std::process::exit(code as i32);
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int (exit code)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "EXIT".to_string() 
                        }),
                    }
                }
                // Time operations
                OpCode::GetTime => {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    match SystemTime::now().duration_since(UNIX_EPOCH) {
                        Ok(duration) => {
                            self.stack.push(Value::Int(duration.as_secs() as i64));
                        }
                        Err(e) => return Err(VMError::FileError { 
                            filename: "system_time".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::Sleep => {
                    let val = self.pop_stack("SLEEP")?;
                    match val {
                        Value::Int(millis) => {
                            let duration = std::time::Duration::from_millis(millis as u64);
                            std::thread::sleep(duration);
                        }
                        Value::Float(millis) => {
                            let duration = std::time::Duration::from_millis(millis as u64);
                            std::thread::sleep(duration);
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int or float (milliseconds)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "SLEEP".to_string() 
                        }),
                    }
                }
                OpCode::FormatTime => {
                    let format_str = self.pop_stack("FORMAT_TIME")?;
                    let timestamp = self.pop_stack("FORMAT_TIME")?;
                    match (timestamp, format_str) {
                        (Value::Int(ts), Value::Str(_format)) => {
                            // Simplified time formatting - just return ISO format
                            use std::time::UNIX_EPOCH;
                            let _system_time = UNIX_EPOCH + Duration::from_secs(ts as u64);
                            // For simplicity, just return the timestamp as string
                            // In a real implementation, we'd use chrono or similar for formatting
                            self.stack.push(Value::Str(format!("{}", ts)));
                        }
                        (t, f) => return Err(VMError::TypeMismatch { 
                            expected: "int (timestamp) and string (format)".to_string(), 
                            got: format!("{:?}, {:?}", t, f), 
                            operation: "FORMAT_TIME".to_string() 
                        }),
                    }
                }
                // Network operations
                OpCode::HttpGet => {
                    let val = self.pop_stack("HTTP_GET")?;
                    match val {
                        Value::Str(url) => {
                            // Simplified HTTP GET using std library (in real implementation would use reqwest)
                            // For now, just return a placeholder response
                            let response = format!("HTTP response from {}", url);
                            self.stack.push(Value::Str(response));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (URL)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "HTTP_GET".to_string() 
                        }),
                    }
                }
                OpCode::HttpPost => {
                    let data = self.pop_stack("HTTP_POST")?;
                    let url = self.pop_stack("HTTP_POST")?;
                    match (url, data) {
                        (Value::Str(url_str), Value::Str(data_str)) => {
                            // Simplified HTTP POST (in real implementation would use reqwest)
                            let response = format!("HTTP POST to {} with data: {}", url_str, data_str);
                            self.stack.push(Value::Str(response));
                        }
                        (u, d) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (URL, data)".to_string(), 
                            got: format!("{:?}, {:?}", u, d), 
                            operation: "HTTP_POST".to_string() 
                        }),
                    }
                }
                OpCode::TcpConnect => {
                    let port = self.pop_stack("TCP_CONNECT")?;
                    let host = self.pop_stack("TCP_CONNECT")?;
                    match (host, port) {
                        (Value::Str(host_str), Value::Int(port_num)) => {
                            // Simplified TCP connect - in real implementation would create actual socket
                            use std::net::TcpStream;
                            let address = format!("{}:{}", host_str, port_num);
                            match TcpStream::connect(&address) {
                                Ok(_stream) => {
                                    // In real implementation, we'd store the stream
                                    // For now, just return a connection ID
                                    let conn_id = format!("tcp://{}:{}", host_str, port_num);
                                    self.stack.push(Value::Connection(conn_id));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: address, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (h, p) => return Err(VMError::TypeMismatch { 
                            expected: "string (host) and int (port)".to_string(), 
                            got: format!("{:?}, {:?}", h, p), 
                            operation: "TCP_CONNECT".to_string() 
                        }),
                    }
                }
                OpCode::TcpListen => {
                    let val = self.pop_stack("TCP_LISTEN")?;
                    match val {
                        Value::Int(port) => {
                            // Simplified TCP listen - in real implementation would bind and listen
                            use std::net::TcpListener;
                            let address = format!("127.0.0.1:{}", port);
                            match TcpListener::bind(&address) {
                                Ok(_listener) => {
                                    // In real implementation, we'd store the listener
                                    let conn_id = format!("tcp://listener:{}", port);
                                    self.stack.push(Value::Connection(conn_id));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: address, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int (port)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "TCP_LISTEN".to_string() 
                        }),
                    }
                }
                OpCode::TcpSend => {
                    let data = self.pop_stack("TCP_SEND")?;
                    let conn = self.pop_stack("TCP_SEND")?;
                    match (conn, data) {
                        (Value::Connection(conn_id), Value::Str(data_str)) => {
                            // Simplified TCP send - in real implementation would send via actual socket
                            println!("TCP Send to {}: {}", conn_id, data_str);
                            self.stack.push(Value::Int(data_str.len() as i64));
                        }
                        (Value::Connection(conn_id), Value::Bytes(data_bytes)) => {
                            // Send binary data
                            println!("TCP Send to {}: {} bytes", conn_id, data_bytes.len());
                            self.stack.push(Value::Int(data_bytes.len() as i64));
                        }
                        (c, d) => return Err(VMError::TypeMismatch { 
                            expected: "connection and string/bytes".to_string(), 
                            got: format!("{:?}, {:?}", c, d), 
                            operation: "TCP_SEND".to_string() 
                        }),
                    }
                }
                OpCode::TcpRecv => {
                    let size = self.pop_stack("TCP_RECV")?;
                    let conn = self.pop_stack("TCP_RECV")?;
                    match (conn, size) {
                        (Value::Connection(conn_id), Value::Int(buffer_size)) => {
                            // Simplified TCP recv - in real implementation would receive from actual socket
                            let received_data = format!("Data from {}", conn_id);
                            if buffer_size > 0 {
                                self.stack.push(Value::Str(received_data));
                            } else {
                                self.stack.push(Value::Bytes(vec![1, 2, 3, 4])); // Mock binary data
                            }
                        }
                        (c, s) => return Err(VMError::TypeMismatch { 
                            expected: "connection and int (buffer size)".to_string(), 
                            got: format!("{:?}, {:?}", c, s), 
                            operation: "TCP_RECV".to_string() 
                        }),
                    }
                }
                OpCode::UdpBind => {
                    let val = self.pop_stack("UDP_BIND")?;
                    match val {
                        Value::Int(port) => {
                            // Simplified UDP bind - in real implementation would bind UDP socket
                            use std::net::UdpSocket;
                            let address = format!("127.0.0.1:{}", port);
                            match UdpSocket::bind(&address) {
                                Ok(_socket) => {
                                    let conn_id = format!("udp://bind:{}", port);
                                    self.stack.push(Value::Connection(conn_id));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: address, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int (port)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "UDP_BIND".to_string() 
                        }),
                    }
                }
                OpCode::UdpSend => {
                    let data = self.pop_stack("UDP_SEND")?;
                    let port = self.pop_stack("UDP_SEND")?;
                    let host = self.pop_stack("UDP_SEND")?;
                    let socket = self.pop_stack("UDP_SEND")?;
                    match (socket, host, port, data) {
                        (Value::Connection(conn_id), Value::Str(host_str), Value::Int(port_num), Value::Str(data_str)) => {
                            // Simplified UDP send
                            println!("UDP Send from {} to {}:{}: {}", conn_id, host_str, port_num, data_str);
                            self.stack.push(Value::Int(data_str.len() as i64));
                        }
                        (s, h, p, d) => return Err(VMError::TypeMismatch { 
                            expected: "connection, string (host), int (port), string (data)".to_string(), 
                            got: format!("{:?}, {:?}, {:?}, {:?}", s, h, p, d), 
                            operation: "UDP_SEND".to_string() 
                        }),
                    }
                }
                OpCode::UdpRecv => {
                    let size = self.pop_stack("UDP_RECV")?;
                    let socket = self.pop_stack("UDP_RECV")?;
                    match (socket, size) {
                        (Value::Connection(_conn_id), Value::Int(_buffer_size)) => {
                            // Simplified UDP recv - return mock data and sender info
                            let mut result = HashMap::new();
                            result.insert("data".to_string(), Value::Str("UDP packet data".to_string()));
                            result.insert("sender_host".to_string(), Value::Str("192.168.1.100".to_string()));
                            result.insert("sender_port".to_string(), Value::Int(12345));
                            self.stack.push(Value::Object(result));
                        }
                        (s, sz) => return Err(VMError::TypeMismatch { 
                            expected: "connection and int (buffer size)".to_string(), 
                            got: format!("{:?}, {:?}", s, sz), 
                            operation: "UDP_RECV".to_string() 
                        }),
                    }
                }
                OpCode::DnsResolve => {
                    let val = self.pop_stack("DNS_RESOLVE")?;
                    match val {
                        Value::Str(hostname) => {
                            // Simplified DNS resolution using std library
                            use std::net::ToSocketAddrs;
                            let address_with_port = format!("{}:80", hostname); // Add dummy port for resolution
                            match address_with_port.to_socket_addrs() {
                                Ok(mut addrs) => {
                                    if let Some(addr) = addrs.next() {
                                        let ip = addr.ip().to_string();
                                        self.stack.push(Value::Str(ip));
                                    } else {
                                        self.stack.push(Value::Null);
                                    }
                                }
                                Err(_) => {
                                    // Return mock IP for demonstration
                                    self.stack.push(Value::Str("192.168.1.1".to_string()));
                                }
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (hostname)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DNS_RESOLVE".to_string() 
                        }),
                    }
                }
                // Advanced I/O operations
                OpCode::AsyncRead => {
                    let val = self.pop_stack("ASYNC_READ")?;
                    match val {
                        Value::Str(filename) => {
                            // Simplified async read - in real implementation would use tokio or async-std
                            let future_id = format!("async_read:{}", filename);
                            self.stack.push(Value::Future(future_id));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "ASYNC_READ".to_string() 
                        }),
                    }
                }
                OpCode::AsyncWrite => {
                    let filename = self.pop_stack("ASYNC_WRITE")?;
                    let content = self.pop_stack("ASYNC_WRITE")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(data)) => {
                            // Simplified async write - encode filename and content in future ID
                            let future_id = format!("async_write:{}:{}", fname, data);
                            self.stack.push(Value::Future(future_id));
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "string (filename) and string (content)".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "ASYNC_WRITE".to_string() 
                        }),
                    }
                }
                OpCode::Await => {
                    let val = self.pop_stack("AWAIT")?;
                    match val {
                        Value::Future(future_id) => {
                            // Simplified await - simulate completion
                            if future_id.starts_with("async_read:") {
                                let filename = future_id.strip_prefix("async_read:").unwrap_or("unknown");
                                // Simulate reading file
                                match std::fs::read_to_string(filename) {
                                    Ok(content) => self.stack.push(Value::Str(content)),
                                    Err(e) => return Err(VMError::FileError { 
                                        filename: filename.to_string(), 
                                        error: e.to_string() 
                                    }),
                                }
                            } else if future_id.starts_with("async_write:") {
                                // Parse the async_write future format: "async_write:filename:content"
                                let content_part = future_id.strip_prefix("async_write:").unwrap_or("");
                                if let Some(separator_index) = content_part.find(':') {
                                    let filename = &content_part[..separator_index];
                                    let data = &content_part[separator_index + 1..];
                                    match std::fs::write(filename, data) {
                                        Ok(()) => self.stack.push(Value::Bool(true)),
                                        Err(e) => return Err(VMError::FileError { 
                                            filename: filename.to_string(), 
                                            error: e.to_string() 
                                        }),
                                    }
                                } else {
                                    self.stack.push(Value::Bool(true));
                                }
                            } else {
                                self.stack.push(Value::Null);
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "future".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "AWAIT".to_string() 
                        }),
                    }
                }
                OpCode::StreamCreate => {
                    let val = self.pop_stack("STREAM_CREATE")?;
                    match val {
                        Value::Str(stream_type) => {
                            let stream_id = format!("stream:{}:{}", stream_type, self.instruction_count);
                            self.stack.push(Value::Stream(stream_id));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (stream type)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "STREAM_CREATE".to_string() 
                        }),
                    }
                }
                OpCode::StreamRead => {
                    let size = self.pop_stack("STREAM_READ")?;
                    let stream = self.pop_stack("STREAM_READ")?;
                    match (stream, size) {
                        (Value::Stream(_stream_id), Value::Int(read_size)) => {
                            // Simplified stream read
                            let data = format!("stream_data_{}", read_size);
                            self.stack.push(Value::Str(data));
                        }
                        (s, sz) => return Err(VMError::TypeMismatch { 
                            expected: "stream and int (size)".to_string(), 
                            got: format!("{:?}, {:?}", s, sz), 
                            operation: "STREAM_READ".to_string() 
                        }),
                    }
                }
                OpCode::StreamWrite => {
                    let data = self.pop_stack("STREAM_WRITE")?;
                    let stream = self.pop_stack("STREAM_WRITE")?;
                    match (stream, data) {
                        (Value::Stream(_stream_id), Value::Str(write_data)) => {
                            // Simplified stream write
                            self.stack.push(Value::Int(write_data.len() as i64));
                        }
                        (Value::Stream(_stream_id), Value::Bytes(write_bytes)) => {
                            // Write binary data to stream
                            self.stack.push(Value::Int(write_bytes.len() as i64));
                        }
                        (s, d) => return Err(VMError::TypeMismatch { 
                            expected: "stream and string/bytes".to_string(), 
                            got: format!("{:?}, {:?}", s, d), 
                            operation: "STREAM_WRITE".to_string() 
                        }),
                    }
                }
                OpCode::StreamClose => {
                    let val = self.pop_stack("STREAM_CLOSE")?;
                    match val {
                        Value::Stream(_stream_id) => {
                            // Simplified stream close
                            self.stack.push(Value::Bool(true));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "stream".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "STREAM_CLOSE".to_string() 
                        }),
                    }
                }
                OpCode::JsonParse => {
                    let val = self.pop_stack("JSON_PARSE")?;
                    match val {
                        Value::Str(json_str) => {
                            // Simplified JSON parsing - in real implementation would use serde_json
                            if json_str.starts_with('{') && json_str.ends_with('}') {
                                let mut obj = HashMap::new();
                                obj.insert("parsed".to_string(), Value::Bool(true));
                                obj.insert("data".to_string(), Value::Str("json_data".to_string()));
                                self.stack.push(Value::Object(obj));
                            } else if json_str.starts_with('[') && json_str.ends_with(']') {
                                let list = vec![
                                    Value::Str("item1".to_string()),
                                    Value::Str("item2".to_string()),
                                ];
                                self.stack.push(Value::List(list));
                            } else {
                                self.stack.push(Value::Str(json_str));
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (JSON)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "JSON_PARSE".to_string() 
                        }),
                    }
                }
                OpCode::JsonStringify => {
                    let val = self.pop_stack("JSON_STRINGIFY")?;
                    match val {
                        Value::Object(_) => {
                            // Simplified JSON stringification
                            self.stack.push(Value::Str("{\"key\":\"value\"}".to_string()));
                        }
                        Value::List(_) => {
                            self.stack.push(Value::Str("[\"item1\",\"item2\"]".to_string()));
                        }
                        Value::Str(s) => {
                            self.stack.push(Value::Str(format!("\"{}\"", s)));
                        }
                        Value::Int(n) => {
                            self.stack.push(Value::Str(n.to_string()));
                        }
                        Value::Bool(b) => {
                            self.stack.push(Value::Str(b.to_string()));
                        }
                        Value::Null => {
                            self.stack.push(Value::Str("null".to_string()));
                        }
                        _ => {
                            self.stack.push(Value::Str("{}".to_string()));
                        }
                    }
                }
                OpCode::CsvParse => {
                    let val = self.pop_stack("CSV_PARSE")?;
                    match val {
                        Value::Str(csv_str) => {
                            // Simplified CSV parsing
                            let rows: Vec<Value> = csv_str.lines()
                                .map(|line| {
                                    let columns: Vec<Value> = line.split(',')
                                        .map(|col| Value::Str(col.trim().to_string()))
                                        .collect();
                                    Value::List(columns)
                                })
                                .collect();
                            self.stack.push(Value::List(rows));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (CSV)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "CSV_PARSE".to_string() 
                        }),
                    }
                }
                OpCode::CsvWrite => {
                    let val = self.pop_stack("CSV_WRITE")?;
                    match val {
                        Value::List(rows) => {
                            // Simplified CSV writing
                            let mut csv_output = String::new();
                            for (i, row) in rows.iter().enumerate() {
                                if i > 0 {
                                    csv_output.push('\n');
                                }
                                if let Value::List(columns) = row {
                                    for (j, col) in columns.iter().enumerate() {
                                        if j > 0 {
                                            csv_output.push(',');
                                        }
                                        csv_output.push_str(&format!("{}", col));
                                    }
                                }
                            }
                            self.stack.push(Value::Str(csv_output));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "list of lists (CSV data)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "CSV_WRITE".to_string() 
                        }),
                    }
                }
                OpCode::Compress => {
                    let val = self.pop_stack("COMPRESS")?;
                    match val {
                        Value::Str(data) => {
                            // Simplified compression - in real implementation would use flate2
                            let compressed = format!("compressed({})", data.len());
                            self.stack.push(Value::Bytes(compressed.into_bytes()));
                        }
                        Value::Bytes(data) => {
                            let compressed = format!("compressed({})", data.len());
                            self.stack.push(Value::Bytes(compressed.into_bytes()));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string or bytes".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "COMPRESS".to_string() 
                        }),
                    }
                }
                OpCode::Decompress => {
                    let val = self.pop_stack("DECOMPRESS")?;
                    match val {
                        Value::Bytes(data) => {
                            // Simplified decompression
                            let decompressed = format!("decompressed:{}", String::from_utf8_lossy(&data));
                            self.stack.push(Value::Str(decompressed));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "bytes (compressed data)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DECOMPRESS".to_string() 
                        }),
                    }
                }
                OpCode::Encrypt => {
                    let key = self.pop_stack("ENCRYPT")?;
                    let data = self.pop_stack("ENCRYPT")?;
                    match (data, key) {
                        (Value::Str(plaintext), Value::Str(encryption_key)) => {
                            // Simplified encryption - in real implementation would use proper crypto
                            let encrypted = format!("encrypted:{}:key:{}", plaintext.len(), encryption_key.len());
                            self.stack.push(Value::Bytes(encrypted.into_bytes()));
                        }
                        (d, k) => return Err(VMError::TypeMismatch { 
                            expected: "string (data) and string (key)".to_string(), 
                            got: format!("{:?}, {:?}", d, k), 
                            operation: "ENCRYPT".to_string() 
                        }),
                    }
                }
                OpCode::Decrypt => {
                    let key = self.pop_stack("DECRYPT")?;
                    let data = self.pop_stack("DECRYPT")?;
                    match (data, key) {
                        (Value::Bytes(ciphertext), Value::Str(_decryption_key)) => {
                            // Simplified decryption
                            let decrypted = format!("decrypted:{}", String::from_utf8_lossy(&ciphertext));
                            self.stack.push(Value::Str(decrypted));
                        }
                        (d, k) => return Err(VMError::TypeMismatch { 
                            expected: "bytes (encrypted data) and string (key)".to_string(), 
                            got: format!("{:?}, {:?}", d, k), 
                            operation: "DECRYPT".to_string() 
                        }),
                    }
                }
                OpCode::Hash => {
                    let val = self.pop_stack("HASH")?;
                    match val {
                        Value::Str(data) => {
                            // Simplified hashing - in real implementation would use sha2, md5, etc.
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut hasher = DefaultHasher::new();
                            data.hash(&mut hasher);
                            let hash_value = hasher.finish();
                            self.stack.push(Value::Str(format!("{:x}", hash_value)));
                        }
                        Value::Bytes(data) => {
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut hasher = DefaultHasher::new();
                            data.hash(&mut hasher);
                            let hash_value = hasher.finish();
                            self.stack.push(Value::Str(format!("{:x}", hash_value)));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string or bytes".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "HASH".to_string() 
                        }),
                    }
                }
                OpCode::DbConnect => {
                    let val = self.pop_stack("DB_CONNECT")?;
                    match val {
                        Value::Str(connection_string) => {
                            // Simplified database connection - in real implementation would use sqlx, rusqlite, etc.
                            let db_id = format!("db:{}", connection_string);
                            self.stack.push(Value::Connection(db_id));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (connection string)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DB_CONNECT".to_string() 
                        }),
                    }
                }
                OpCode::DbQuery => {
                    let query = self.pop_stack("DB_QUERY")?;
                    let db = self.pop_stack("DB_QUERY")?;
                    match (db, query) {
                        (Value::Connection(_db_id), Value::Str(_sql_query)) => {
                            // Simplified database query
                            let mut result = HashMap::new();
                            result.insert("rows".to_string(), Value::Int(3));
                            result.insert("columns".to_string(), Value::List(vec![
                                Value::Str("id".to_string()),
                                Value::Str("name".to_string()),
                            ]));
                            result.insert("data".to_string(), Value::List(vec![
                                Value::List(vec![Value::Int(1), Value::Str("Alice".to_string())]),
                                Value::List(vec![Value::Int(2), Value::Str("Bob".to_string())]),
                                Value::List(vec![Value::Int(3), Value::Str("Charlie".to_string())]),
                            ]));
                            self.stack.push(Value::Object(result));
                        }
                        (d, q) => return Err(VMError::TypeMismatch { 
                            expected: "connection and string (SQL query)".to_string(), 
                            got: format!("{:?}, {:?}", d, q), 
                            operation: "DB_QUERY".to_string() 
                        }),
                    }
                }
                OpCode::DbExec => {
                    let command = self.pop_stack("DB_EXEC")?;
                    let db = self.pop_stack("DB_EXEC")?;
                    match (db, command) {
                        (Value::Connection(_db_id), Value::Str(_sql_command)) => {
                            // Simplified database execution
                            self.stack.push(Value::Int(1)); // affected rows
                        }
                        (d, c) => return Err(VMError::TypeMismatch { 
                            expected: "connection and string (SQL command)".to_string(), 
                            got: format!("{:?}, {:?}", d, c), 
                            operation: "DB_EXEC".to_string() 
                        }),
                    }
                }
                OpCode::DumpScope => {
                    println!("Current scope: {:?}", self.variables.last());
                }
                // Exception handling opcodes
                OpCode::Try { catch_addr } => {
                    self.push_exception_handler(*catch_addr);
                }
                OpCode::Catch => {
                    // The exception should already be on the stack from throw_exception
                    // Nothing to do here, just mark that we're in the catch block
                }
                OpCode::Throw => {
                    let exception_value = self.pop_stack("THROW")?;
                    
                    // Convert value to exception if it's not already one
                    let exception = match exception_value {
                        Value::Exception { .. } => exception_value,
                        Value::Str(msg) => Value::Exception { 
                            message: msg,
                            stack_trace: vec![format!("at instruction {}", self.ip)]
                        },
                        other => Value::Exception {
                            message: format!("Thrown value: {:?}", other),
                            stack_trace: vec![format!("at instruction {}", self.ip)]
                        }
                    };
                    
                    self.throw_exception(exception)?;
                }
                OpCode::EndTry => {
                    // Pop the exception handler when exiting try block normally
                    self.pop_exception_handler();
                }
                OpCode::Import(path) => {
                    self.import_module(path)?;
                }
                OpCode::Export(name) => {
                    self.export_symbol(name)?;
                }
                OpCode::Spawn => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("SPAWN not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Receive => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("RECEIVE not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Yield => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("YIELD not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Send(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("SEND not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Monitor(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("MONITOR not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Demonitor(_monitor_ref) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("DEMONITOR not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Link(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("LINK not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Unlink(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("UNLINK not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::TrapExit => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("TRAP_EXIT not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::ReceiveMatch(_patterns) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("RECEIVE_MATCH not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Register(_name) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("REGISTER not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Unregister(_name) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("UNREGISTER not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Whereis(_name) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("WHEREIS not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::SendNamed(_name) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("SENDNAMED not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::StartSupervisor => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("STARTSUPERVISOR not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::SuperviseChild(_name) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("SUPERVISECHILD not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::RestartChild(_name) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("RESTARTCHILD not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Halt => {
                    // This should never be reached since HALT is handled in run()
                    unreachable!("HALT instruction should be handled in run() method")
                }
            }
            Ok(())
        }

    fn import_module(&mut self, path: &str) -> VMResult<()> {
        // Check for circular dependencies using global loading stack
        if self.loading_stack.contains(&path.to_string()) {
            return Err(VMError::CircularDependency(path.to_string()));
        }

        // Check if module is already loaded
        if let Some(exports) = self.loaded_modules.get(path).cloned() {
            // Module already loaded, import its exports into current scope
            for (name, value) in exports {
                self.set_variable(name, value)?;
            }
            return Ok(());
        }

        // Add to loading stack to detect circular dependencies
        self.loading_stack.push(path.to_string());

        // Load and parse the module
        let module_instructions = parse_program(path)?;
        
        // Merge module instructions into main VM's instruction space
        let base_addr = self.instructions.len();
        let mut adjusted_exports = HashMap::new();
        
        // Adjust addresses in module instructions and append to main instruction space
        let adjusted_instructions = module_instructions.iter()
            .map(|inst| self.adjust_instruction_addresses(inst, base_addr))
            .collect::<Vec<_>>();
        self.instructions.extend(adjusted_instructions);
        
        // Create a new VM with the module instructions to get exports
        // Share the loading context to detect circular dependencies
        let mut module_vm = VM::new(module_instructions);
        module_vm.debug_mode = self.debug_mode;
        module_vm.loading_stack = self.loading_stack.clone(); // Share loading context
        module_vm.loaded_modules = self.loaded_modules.clone(); // Share loaded modules
        
        // Run the module to generate exports
        module_vm.run()?;
        
        // Update our loaded modules with any new modules the sub-module loaded
        self.loaded_modules.extend(module_vm.loaded_modules);
        
        // Adjust function addresses in exports to point to merged instruction space
        for (name, value) in module_vm.exports {
            let adjusted_value = self.adjust_value_addresses(value, base_addr);
            adjusted_exports.insert(name, adjusted_value);
        }
        
        // Cache the loaded module
        self.loaded_modules.insert(path.to_string(), adjusted_exports.clone());
        
        // Import the exports into current scope
        if self.debug_mode {
            println!("Importing {} exports from module {}", adjusted_exports.len(), path);
        }
        for (name, value) in adjusted_exports {
            if self.debug_mode {
                println!("Importing export: {} = {:?}", name, value);
            }
            self.set_variable(name, value)?;
        }
        
        // Remove from loading stack
        self.loading_stack.pop();
        
        Ok(())
    }

    fn export_symbol(&mut self, name: &str) -> VMResult<()> {
        // Get the value from current scope
        let value = self.get_variable(name)?.clone();
        
        // Add to exports
        self.exports.insert(name.to_string(), value);
        
        Ok(())
    }

    fn adjust_value_addresses(&self, value: Value, base_addr: usize) -> Value {
        match value {
            Value::Function { addr, params } => {
                Value::Function { 
                    addr: addr + base_addr,
                    params 
                }
            }
            Value::Closure { addr, params, captured } => {
                // Recursively adjust addresses in captured environment
                let adjusted_captured = captured.into_iter()
                    .map(|(name, val)| (name, self.adjust_value_addresses(val, base_addr)))
                    .collect();
                
                Value::Closure { 
                    addr: addr + base_addr,
                    params,
                    captured: adjusted_captured
                }
            }
            Value::List(items) => {
                let adjusted_items = items.into_iter()
                    .map(|item| self.adjust_value_addresses(item, base_addr))
                    .collect();
                Value::List(adjusted_items)
            }
            Value::Object(map) => {
                let adjusted_map = map.into_iter()
                    .map(|(key, val)| (key, self.adjust_value_addresses(val, base_addr)))
                    .collect();
                Value::Object(adjusted_map)
            }
            // Other value types don't contain addresses
            other => other
        }
    }

    fn adjust_instruction_addresses(&self, instruction: &OpCode, base_addr: usize) -> OpCode {
        match instruction {
            OpCode::Jmp(addr) => OpCode::Jmp(addr + base_addr),
            OpCode::Jz(addr) => OpCode::Jz(addr + base_addr),
            OpCode::Call { addr, params } => OpCode::Call { 
                addr: addr + base_addr, 
                params: params.clone() 
            },
            OpCode::MakeFunction { addr, params } => OpCode::MakeFunction { 
                addr: addr + base_addr, 
                params: params.clone() 
            },
            OpCode::MakeLambda { addr, params } => OpCode::MakeLambda { 
                addr: addr + base_addr, 
                params: params.clone() 
            },
            OpCode::Try { catch_addr } => OpCode::Try { 
                catch_addr: catch_addr + base_addr 
            },
            // All other instructions don't contain addresses
            other => other.clone()
        }
    }
}

