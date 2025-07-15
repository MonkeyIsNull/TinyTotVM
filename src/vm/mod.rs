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
pub use stack::{StackOps, SafeStackOps, check_stack_items};
pub use memory::{VariableFrame, VariableManager, ExceptionHandler, CallStack};
pub use machine::VM;