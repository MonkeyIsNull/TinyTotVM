pub mod errors;
pub mod opcode;
pub mod value;
pub mod stack;
pub mod memory;
pub mod machine;

// Re-export commonly used types
pub use errors::{VMError, VMResult};
pub use opcode::{OpCode, ProcId, MessagePattern};
pub use value::Value;

pub use memory::{ExceptionHandler};
pub use machine::VM;