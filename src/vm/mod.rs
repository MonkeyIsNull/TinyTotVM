pub mod errors;
pub mod opcode;
pub mod value;

// Re-export commonly used types
pub use errors::{VMError, VMResult};
pub use opcode::{OpCode, ProcId, MessagePattern};
pub use value::Value;

// Modules to be created in subsequent tasks
// pub mod machine;
// pub mod stack;
// pub mod memory;