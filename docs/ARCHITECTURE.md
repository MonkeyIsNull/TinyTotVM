# Architecture Documentation

TinyTotVM uses a clean, modular architecture designed for extensibility, performance, and educational clarity.

## High-Level Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Source Code   │───▶│    Compiler     │───▶│   Bytecode      │
│  (.ttvm/.lisp)  │    │   (parser)      │    │    (.ttb)       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Optimizer     │◀───│      VM         │◀───│    Loader       │
│  (8 passes)     │    │  (execution)    │    │  (bytecode)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                               │
                               ▼
                    ┌─────────────────┐
                    │ Standard Library │
                    │   (modules)      │
                    └─────────────────┘
```

## Core Components

### VM Engine (main.rs)
The heart of TinyTotVM, implementing:

- **Stack-based execution model**
- **Instruction dispatch loop**
- **Memory management**
- **Function call handling**
- **Exception processing**
- **Garbage collection integration**
- **Profiling and tracing support**

### Value System
```rust
enum Value {
    Int(i64),
    Float(f64), 
    Str(String),
    Bool(bool),
    Null,
    List(Vec<Value>),
    Object(HashMap<String, Value>),
    Bytes(Vec<u8>),
    Connection(String),
    Stream(String),
    Future(String),
    Function { addr: usize, params: Vec<String> },
    Closure { addr: usize, params: Vec<String>, captured: HashMap<String, Value> },
    Exception { message: String, stack_trace: Vec<String> },
}
```

### Runtime Stacks
- **Main Stack** - Value storage with 1024 pre-allocated slots
- **Call Stack** - Return addresses with 64 pre-allocated slots  
- **Variable Frames** - Lexical scoping with frame stack
- **Exception Stack** - Try/catch blocks with unwinding support

## Memory Management

### Stack Architecture
```
┌─────────────────┐ ← Top
│     Value N     │
├─────────────────┤
│     Value 2     │
├─────────────────┤
│     Value 1     │ ← Bottom
└─────────────────┘
Main Stack (1024 slots)

┌─────────────────┐ ← Current Call
│   Return Addr   │
├─────────────────┤
│   Return Addr   │ ← Previous Call
└─────────────────┘
Call Stack (64 slots)
```

### Variable Scoping
```rust
variables: Vec<HashMap<String, Value>>
//         │   └─ Variables in scope
//         └─ Frame stack (one per function call)
```

### Garbage Collection
Pluggable GC architecture:

```rust
trait GcEngine {
    fn alloc(&mut self, value: Value) -> GcRef;
    fn mark_from_roots(&mut self, roots: &[&Value]);
    fn sweep(&mut self) -> usize;
    fn stats(&self) -> GcStats;
}
```

**Available Engines:**
- **MarkSweepGc** - Traditional mark & sweep
- **NoGc** - Disabled garbage collection
- **Future:** Reference counting, generational GC

## Compilation Pipeline

### 1. Parsing (compiler.rs)
```
Source Code → Tokenization → AST → Instruction Generation
```

**Features:**
- Symbolic and numeric label resolution
- Parameter validation
- Syntax error reporting
- Instruction optimization hints

### 2. Optimization (optimizer.rs)
8-pass optimization engine:

```rust
pub struct Optimizer {
    stats: OptimizationStats,
    // Pass implementations
}

impl Optimizer {
    pub fn optimize(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        // 8 optimization passes
    }
}
```

**Optimization Passes:**
1. Constant folding
2. Constant propagation  
3. Dead code elimination
4. Peephole optimizations
5. Instruction combining
6. Jump threading
7. Tail call optimization
8. Memory layout optimization

### 3. Bytecode Generation (bytecode.rs)
Binary format for faster loading:

```
Magic Header | Version | Instruction Count | Instructions | Metadata
```

## Execution Engine

### Instruction Dispatch
```rust
fn run(&mut self) -> VMResult<()> {
    while self.ip < self.instructions.len() {
        let instruction = &self.instructions[self.ip];
        
        // Profiling hooks
        if let Some(ref mut profiler) = self.profiler {
            profiler.record_instruction();
        }
        
        // Tracing hooks
        if self.trace_enabled {
            println!("[trace] {:?} @ 0x{:04X}", instruction, self.ip);
        }
        
        // Execute instruction
        match instruction {
            OpCode::PushInt(n) => self.stack.push(Value::Int(*n)),
            OpCode::Add => { /* arithmetic implementation */ },
            // ... other instructions
        }
        
        self.ip += 1;
    }
}
```

### Function Calls
```rust
// Function call mechanism
OpCode::Call { addr, params } => {
    // Save return address
    self.call_stack.push(self.ip + 1);
    
    // Create new variable frame
    let mut frame = HashMap::new();
    for param_name in params.iter().rev() {
        let value = self.pop_stack("CALL")?;
        frame.insert(param_name.clone(), value);
    }
    self.variables.push(frame);
    
    // Jump to function
    self.ip = *addr;
}
```

### Exception Handling
```rust
struct ExceptionHandler {
    catch_addr: usize,
    stack_size: usize,
    call_stack_size: usize,
    variable_frames: usize,
}

// Exception unwinding
fn unwind_to_handler(&mut self, exception: Value) {
    if let Some(handler) = self.try_stack.pop() {
        // Restore stack state
        self.stack.truncate(handler.stack_size);
        self.call_stack.truncate(handler.call_stack_size);
        self.variables.truncate(handler.variable_frames);
        
        // Jump to catch block
        self.stack.push(exception);
        self.ip = handler.catch_addr;
    }
}
```

## Module System

### Module Loading
```rust
fn import_module(&mut self, module_path: &str) -> VMResult<()> {
    // Circular dependency detection
    if self.loading_stack.contains(&module_path.to_string()) {
        return Err(VMError::FileError {
            filename: module_path.to_string(),
            error: "Circular dependency detected".to_string(),
        });
    }
    
    // Load and execute module
    self.loading_stack.push(module_path.to_string());
    let module_vm = VM::new(module_instructions);
    module_vm.run()?;
    
    // Import exports
    self.loaded_modules.insert(module_path.to_string(), module_vm.exports);
    self.loading_stack.pop();
}
```

### Address Resolution
```rust
fn adjust_instruction_addresses(&self, instruction: &OpCode, base_addr: usize) -> OpCode {
    match instruction {
        OpCode::Call { addr, params } => OpCode::Call { 
            addr: addr + base_addr, 
            params: params.clone() 
        },
        OpCode::Jmp(addr) => OpCode::Jmp(addr + base_addr),
        // ... other address-containing instructions
    }
}
```

## Profiling and Tracing System

### Profiler Architecture
```rust
struct Profiler {
    function_timings: HashMap<String, Duration>,
    instruction_counts: HashMap<String, usize>,
    call_counts: HashMap<String, usize>,
    current_function_stack: Vec<(String, FunctionProfiler)>,
    // ... other metrics
}

struct FunctionProfiler {
    start_time: Instant,
    instruction_count: usize,
}
```

### Tracing Integration
```rust
// Instruction-level tracing
if self.trace_enabled {
    let indent = "  ".repeat(self.call_depth);
    println!("[trace] {}{:?} @ 0x{:04X}", indent, instruction, self.ip);
}

// Function call tracing
if self.trace_enabled {
    println!("[trace] {}CALL {} with {} params", indent, function_name, params.len());
}
```

## Error Handling

### Error Types
```rust
#[derive(Debug)]
pub enum VMError {
    StackUnderflow(String),
    TypeMismatch { expected: String, got: String, operation: String },
    UndefinedVariable(String),
    IndexOutOfBounds { index: i64, length: usize },
    FileError { filename: String, error: String },
    ParseError { line: usize, instruction: String },
    CallStackUnderflow,
    NoVariableScope,
    UnknownLabel(String),
    InsufficientStackItems { needed: usize, available: usize },
}
```

### Error Recovery
- **Graceful Degradation** - No crashes or panics
- **Detailed Messages** - Clear error descriptions with context
- **Stack Preservation** - Safe stack unwinding
- **Resource Cleanup** - Automatic cleanup on errors

## Extension Points

### Adding New Instructions
```rust
// 1. Add to OpCode enum
#[derive(Debug, Clone)]
pub enum OpCode {
    // ... existing instructions
    NewInstruction(String),
}

// 2. Add execution logic
match instruction {
    // ... existing cases
    OpCode::NewInstruction(param) => {
        // Implementation
    }
}

// 3. Add parsing support
"NEW_INSTRUCTION" => {
    let param = parts[1].to_string();
    OpCode::NewInstruction(param)
}
```

### Adding New Value Types
```rust
// 1. Add to Value enum
#[derive(Debug, Clone)]
pub enum Value {
    // ... existing types
    NewType(CustomData),
}

// 2. Add type-specific operations
// 3. Add serialization support
// 4. Add GC integration if needed
```

### Custom Standard Library Modules
```assembly
; custom_module.ttvm
LABEL custom_function
; Implementation
RET

MAKE_FUNCTION custom_function params
STORE custom_function
EXPORT custom_function
```

## Performance Characteristics

### Time Complexity
- **Instruction Execution**: O(1) per instruction
- **Function Calls**: O(1) call overhead
- **Variable Access**: O(1) with hash map lookup
- **Exception Handling**: O(n) stack unwinding
- **Garbage Collection**: O(n) mark & sweep

### Space Complexity
- **Stack Space**: O(n) program stack depth
- **Call Stack**: O(d) call depth
- **Variable Storage**: O(v) total variables
- **Module Cache**: O(m) number of modules

### Optimization Impact
- **Constant Folding**: 37% instruction reduction
- **Dead Code Elimination**: 71% instruction reduction  
- **Combined Optimizations**: Up to 46% overall improvement
- **Memory Access**: Reduced redundant operations

## Design Principles

1. **Simplicity** - Clean, understandable code structure
2. **Modularity** - Separate concerns, pluggable components
3. **Safety** - No crashes, comprehensive error handling
4. **Performance** - Efficient execution with optimization
5. **Extensibility** - Easy to add features and instructions
6. **Educational Value** - Clear demonstration of VM concepts
7. **Cross-Platform** - Pure Rust, runs anywhere