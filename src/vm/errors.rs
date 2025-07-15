use std::fmt;

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
    UnsupportedOperation(String),
    RuntimeError(String),
    TypeError(String),
    DivisionByZero,
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
            VMError::UnsupportedOperation(op) => write!(f, "Unsupported operation: {}", op),
            VMError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            VMError::TypeError(msg) => write!(f, "Type error: {}", msg),
            VMError::DivisionByZero => write!(f, "Division by zero"),
        }
    }
}

impl std::error::Error for VMError {}

pub type VMResult<T> = Result<T, VMError>;