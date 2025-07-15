use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    List(Vec<Value>),
    Object(HashMap<String, Value>),
    Bytes(Vec<u8>),
    Connection(String), // Network connection handle (simplified as string ID)
    Stream(String),     // Data stream handle (simplified as string ID)
    Future(String),     // Async operation handle (simplified as string ID)
    Function { addr: usize, params: Vec<String> },
    Closure { addr: usize, params: Vec<String>, captured: HashMap<String, Value> },
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
            Value::Bytes(bytes) => {
                write!(f, "Bytes({})", bytes.len())
            },
            Value::Connection(id) => {
                write!(f, "Connection({})", id)
            },
            Value::Stream(id) => {
                write!(f, "Stream({})", id)
            },
            Value::Future(id) => {
                write!(f, "Future({})", id)
            },
            Value::Function { addr, params } => {
                write!(f, "function@{} ({})", addr, params.join(", "))
            },
            Value::Closure { addr, params, captured } => {
                write!(f, "closure@{} ({}) [captured: {}]", 
                    addr, 
                    params.join(", "),
                    captured.len()
                )
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