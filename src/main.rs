use std::collections::HashMap;
use std::fs;
use std::fmt;
mod bytecode;
mod compiler;
mod lisp_compiler;

#[derive(Debug, Clone)]
pub enum VMError {
    StackUnderflow(String),
    TypeMismatch { expected: String, got: String, operation: String },
    UndefinedVariable(String),
    IndexOutOfBounds { index: usize, length: usize },
    CallStackUnderflow,
    NoVariableScope,
    FileError { filename: String, error: String },
    ParseError { line: usize, instruction: String },
    InsufficientStackItems { needed: usize, available: usize },
    UnknownLabel(String),
}

impl fmt::Display for VMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VMError::StackUnderflow(op) => write!(f, "Stack underflow during {}", op),
            VMError::TypeMismatch { expected, got, operation } => 
                write!(f, "{} expects {} but got {}", operation, expected, got),
            VMError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            VMError::IndexOutOfBounds { index, length } => 
                write!(f, "Index {} out of bounds for list of length {}", index, length),
            VMError::CallStackUnderflow => write!(f, "Call stack underflow"),
            VMError::NoVariableScope => write!(f, "No variable scope available"),
            VMError::FileError { filename, error } => 
                write!(f, "File operation failed on {}: {}", filename, error),
            VMError::ParseError { line, instruction } => 
                write!(f, "Parse error on line {}: {}", line, instruction),
            VMError::InsufficientStackItems { needed, available } => 
                write!(f, "Need {} stack items but only {} available", needed, available),
            VMError::UnknownLabel(label) => write!(f, "Unknown label: {}", label),
        }
    }
}

impl std::error::Error for VMError {}

type VMResult<T> = Result<T, VMError>;

#[derive(Debug, Clone)]
enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    List(Vec<Value>),
    Object(HashMap<String, Value>),
    Function { addr: usize, params: Vec<String> },
    Exception { message: String, stack_trace: Vec<String> },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            },
            Value::Object(map) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in map {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}: {}", key, value)?;
                    first = false;
                }
                write!(f, "}}")
            },
            Value::Function { addr, params } => {
                write!(f, "function@{} ({})", addr, params.join(", "))
            },
            Value::Exception { message, stack_trace } => {
                write!(f, "Exception: {}", message)?;
                if !stack_trace.is_empty() {
                    write!(f, "\nStack trace:")?;
                    for trace in stack_trace {
                        write!(f, "\n  {}", trace)?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
enum OpCode {
    PushInt(i64),
    PushFloat(f64),
    PushStr(String),
    Add,
    AddF,
    Sub,
    SubF,
    MulF,
    DivF,
    Concat,
    Print,
    Halt,
    Jmp(usize),
    Jz(usize),
    Call { addr: usize, params: Vec<String> },
    Ret,
    Dup,
    Store(String),
    Load(String),
    Delete(String),
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    EqF,
    NeF,
    GtF,
    LtF,
    GeF,
    LeF,
    True,
    False,
    Not,
    And,
    Or,
    Null,
    MakeList(usize), // operand: how many items to pop
    Len,
    Index,
    DumpScope,
    ReadFile,
    WriteFile,
    // Object operations
    MakeObject,
    SetField(String),   // field name
    GetField(String),   // field name
    HasField(String),   // field name
    DeleteField(String), // field name
    Keys,              // get all keys as a list
    // Function operations
    MakeFunction { addr: usize, params: Vec<String> }, // create function pointer
    CallFunction,      // call function from stack
    // Exception handling
    Try { catch_addr: usize },  // start try block, jump to catch_addr on exception
    Catch,             // start catch block (exception is on stack)
    Throw,             // throw exception from stack
    EndTry,            // end try block
}

#[derive(Debug, Clone)]
struct ExceptionHandler {
    catch_addr: usize,           // address to jump to on exception
    stack_size: usize,           // stack size when try block started
    call_stack_size: usize,      // call stack size when try block started
    variable_frames: usize,      // number of variable frames when try block started
}

struct VM {
    stack: Vec<Value>,
    instructions: Vec<OpCode>,
    ip: usize,                              // instruction pointer
    call_stack: Vec<usize>,                 // return addresses for CALL/RET
    variables: Vec<HashMap<String, Value>>, // call frame stack
    // Exception handling
    try_stack: Vec<ExceptionHandler>,       // stack of try blocks
    // Performance improvements
    max_stack_size: usize,                  // Track maximum stack usage
    instruction_count: usize,               // Count of executed instructions
    // Debugging support
    debug_mode: bool,
    breakpoints: Vec<usize>,
}

impl VM {
    fn new(instructions: Vec<OpCode>) -> Self {
        VM {
            stack: Vec::with_capacity(1024), // Pre-allocate stack capacity
            instructions,
            ip: 0,
            call_stack: Vec::with_capacity(64), // Pre-allocate call stack
            variables: vec![HashMap::new()], // global frame
            try_stack: Vec::new(),
            max_stack_size: 0,
            instruction_count: 0,
            debug_mode: false,
            breakpoints: Vec::new(),
        }
    }

    fn new_with_debug(instructions: Vec<OpCode>, debug_mode: bool) -> Self {
        let mut vm = Self::new(instructions);
        vm.debug_mode = debug_mode;
        vm
    }

    #[allow(dead_code)]
    fn add_breakpoint(&mut self, address: usize) {
        if !self.breakpoints.contains(&address) {
            self.breakpoints.push(address);
            self.breakpoints.sort();
        }
    }

    #[allow(dead_code)]
    fn remove_breakpoint(&mut self, address: usize) {
        self.breakpoints.retain(|&x| x != address);
    }

    fn get_stats(&self) -> (usize, usize, usize) {
        (self.instruction_count, self.max_stack_size, self.stack.len())
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

    fn run(&mut self) -> VMResult<()> {
        while self.ip < self.instructions.len() {
            // Performance tracking
            self.instruction_count += 1;
            if self.stack.len() > self.max_stack_size {
                self.max_stack_size = self.stack.len();
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
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a function".to_string(), 
                            got: format!("{:?}", function), 
                            operation: "CALL_FUNCTION".to_string() 
                        }),
                    }
                }
                OpCode::ReadFile => {
                    let val = self.pop_stack("READ_file")?;
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
                OpCode::Halt => {
                    // This should never be reached since HALT is handled in run()
                    unreachable!("HALT instruction should be handled in run() method")
                }
            }
            Ok(())
        }
}

fn parse_program(path: &str) -> VMResult<Vec<OpCode>> {
    let content = fs::read_to_string(path).map_err(|e| VMError::FileError { 
        filename: path.to_string(), 
        error: e.to_string() 
    })?;

    let mut label_map: HashMap<String, usize> = HashMap::new();
    let mut instructions_raw: Vec<(usize, &str)> = Vec::new();

    // First pass: build label -> index map
    for (line_num, line) in content.lines().enumerate() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("LABEL ") {
            let label_name = line[6..].trim();
            label_map.insert(label_name.to_string(), instructions_raw.len());
        } else {
            instructions_raw.push((line_num + 1, line)); // save for second pass
        }
    }

    // Second pass: convert raw instructions to OpCode using label map
    let mut program: Vec<OpCode> = Vec::new();
    for (line_num, line) in instructions_raw {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let opcode = match parts[0] {
            "PUSH_INT" => {
                let n = parts[1].parse::<i64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid integer: {}", parts[1]) 
                })?;
                OpCode::PushInt(n)
            }
            "PUSH_FLOAT" => {
                let f = parts[1].parse::<f64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid float: {}", parts[1]) 
                })?;
                OpCode::PushFloat(f)
            }
            "PUSH_STR" => {
                let s = parts[1].trim_matches('"').to_string();
                OpCode::PushStr(s)
            }
            "ADD" => OpCode::Add,
            "ADD_F" => OpCode::AddF,
            "SUB" => OpCode::Sub,
            "SUB_F" => OpCode::SubF,
            "MUL_F" => OpCode::MulF,
            "DIV_F" => OpCode::DivF,
            "DUP" => OpCode::Dup,
            "CONCAT" => OpCode::Concat,
            "PRINT" => OpCode::Print,
            "HALT" => OpCode::Halt,
            "CALL" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: "CALL requires at least a target".to_string() });
                }
                let call_parts: Vec<&str> = parts[1].split_whitespace().collect();
                let label = call_parts[0];
                let params: Vec<String> = call_parts[1..].iter().map(|s| s.to_string()).collect();
                
                // Try parsing as number first, then as label
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Call { addr: target, params }
            }
            "JMP" => {
                let label = parts[1].trim();
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Jmp(target)
            }
            "JZ" => {
                let label = parts[1].trim();
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Jz(target)
            }
            "RET" => OpCode::Ret,
            "STORE" => {
                let var = parts[1].trim().to_string();
                OpCode::Store(var)
            }
            "DELETE" => {
                let var = parts[1].trim().to_string();
                OpCode::Delete(var)
            }
            "LOAD" => {
                let var = parts[1].trim().to_string();
                OpCode::Load(var)
            }
            "EQ" => OpCode::Eq,
            "GT" => OpCode::Gt,
            "LT" => OpCode::Lt,
            "NE" => OpCode::Ne,
            "GE" => OpCode::Ge,
            "LE" => OpCode::Le,
            "EQ_F" => OpCode::EqF,
            "GT_F" => OpCode::GtF,
            "LT_F" => OpCode::LtF,
            "NE_F" => OpCode::NeF,
            "GE_F" => OpCode::GeF,
            "LE_F" => OpCode::LeF,
            "TRUE" => OpCode::True,
            "FALSE" => OpCode::False,
            "NOT" => OpCode::Not,
            "AND" => OpCode::And,
            "OR" => OpCode::Or,
            "NULL" => OpCode::Null,
            "MAKE_LIST" => {
                let n = parts[1].parse::<usize>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid list size: {}", parts[1]) 
                })?;
                OpCode::MakeList(n)
            }
            "LEN" => OpCode::Len,
            "INDEX" => OpCode::Index,
            "DUMP_SCOPE" => OpCode::DumpScope,
            "MAKE_OBJECT" => OpCode::MakeObject,
            "SET_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::SetField(field)
            }
            "GET_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::GetField(field)
            }
            "HAS_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::HasField(field)
            }
            "DELETE_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::DeleteField(field)
            }
            "KEYS" => OpCode::Keys,
            "MAKE_FUNCTION" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: "MAKE_FUNCTION requires at least a target".to_string() });
                }
                let func_parts: Vec<&str> = parts[1].split_whitespace().collect();
                let label = func_parts[0];
                let params: Vec<String> = func_parts[1..].iter().map(|s| s.to_string()).collect();
                
                // Try parsing as number first, then as label
                let addr = if let Ok(address) = label.parse::<usize>() {
                    address
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::MakeFunction { addr, params }
            }
            "CALL_FUNCTION" => OpCode::CallFunction,
            "TRY" => {
                let catch_label = parts[1].trim();
                let catch_addr = *label_map.get(catch_label).ok_or_else(|| VMError::UnknownLabel(catch_label.to_string()))?;
                OpCode::Try { catch_addr }
            }
            "CATCH" => OpCode::Catch,
            "THROW" => OpCode::Throw,
            "END_TRY" => OpCode::EndTry,
            "READ_FILE" => OpCode::ReadFile,
            "WRITE_FILE" => OpCode::WriteFile,
            _ => return Err(VMError::ParseError { line: line_num, instruction: line.to_string() }),
        };
        program.push(opcode);
    }

    Ok(program)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: tinytotvm [--debug] <program.ttvm|program.ttb>");
        eprintln!("       tinytotvm compile <input.ttvm> <output.ttb>");
        eprintln!("       tinytotvm compile-lisp <input.lisp> <output.ttvm>");
        std::process::exit(1);
    }

    let mut debug_mode = false;
    let mut file_index = 1;

    // Check for debug flag
    if args.len() > 2 && args[1] == "--debug" {
        debug_mode = true;
        file_index = 2;
    }

    if args.len() >= 2 && args[1] == "compile" {
        if args.len() != 4 {
            eprintln!("Usage: tinytotvm compile <input.ttvm> <output.ttb>");
            std::process::exit(1);
        }
        let input = &args[2];
        let output = &args[3];
        compiler::compile(input, output).expect("Compilation failed");
        println!("Compiled to {}", output);
        return;
    }

    if args[file_index].ends_with(".ttb") {
        let program = bytecode::load_bytecode(&args[file_index]).expect("Failed to load bytecode");
        let mut vm = VM::new_with_debug(program, debug_mode);
        if let Err(e) = vm.run() {
            eprintln!("VM runtime error: {}", e);
            std::process::exit(1);
        }
        if debug_mode {
            let (instructions, max_stack, final_stack) = vm.get_stats();
            println!("Performance stats - Instructions: {}, Max stack: {}, Final stack: {}", 
                instructions, max_stack, final_stack);
        }
        return;
    }

    if args[1] == "compile-lisp" {
        if args.len() != 4 {
            eprintln!("Usage: tinytotvm compile-lisp <input.lisp> <output.ttvm>");
            std::process::exit(1);
        }
        let input = &args[2];
        let output = &args[3];
        lisp_compiler::compile_lisp(input, output);
        println!("Compiled Lisp to {}", output);
        return;
    }

    let program = match parse_program(&args[file_index]) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    let mut vm = VM::new_with_debug(program, debug_mode);
    if let Err(e) = vm.run() {
        eprintln!("VM runtime error: {}", e);
        std::process::exit(1);
    }
    if debug_mode {
        let (instructions, max_stack, final_stack) = vm.get_stats();
        println!("Performance stats - Instructions: {}, Max stack: {}, Final stack: {}", 
            instructions, max_stack, final_stack);
    }
}
