# Architecture Documentation

TinyTotVM uses a clean, modular architecture designed for extensibility, performance, and educational clarity. The codebase is organized into logical modules that separate concerns and enable easy maintenance and extension.

## Modular Architecture Overview

TinyTotVM has been refactored from a monolithic 7,000+ line `main.rs` file into a well-organized, modular structure:

```
src/
├── main.rs                    # Entry point and CLI
├── lib.rs                     # Library interface
├── bytecode.rs                # Unified instruction parsing
├── compiler.rs                # Source compilation
├── lisp_compiler.rs           # Lisp transpilation
├── optimizer.rs               # Optimization passes
├── vm/                        # Virtual machine core
│   ├── machine.rs             # Execution engine
│   ├── value.rs               # Type system
│   ├── opcode.rs              # Instruction definitions
│   ├── stack.rs               # Stack management
│   ├── memory.rs              # Memory management
│   └── errors.rs              # Error handling
├── concurrency/               # BEAM-style concurrency
│   ├── pool.rs                # SMP scheduler
│   ├── process.rs             # Process isolation
│   ├── scheduler.rs           # Individual schedulers
│   ├── registry.rs            # Process registry
│   ├── supervisor.rs          # Supervision trees
│   └── messages.rs            # Message types
├── gc/                        # Garbage collection
│   ├── mark_sweep.rs          # Mark-sweep GC
│   ├── no_gc.rs               # No-op GC
│   └── stats.rs               # GC statistics
├── profiling/                 # Performance analysis
│   ├── profiler.rs            # Profiling engine
│   └── stats.rs               # Statistics
├── testing/                   # Test framework
│   ├── harness.rs             # Test execution
│   └── runner.rs              # Test runner
├── ir/                        # Intermediate Representation
│   ├── mod.rs                 # Core IR structures
│   ├── lowering.rs            # Stack-to-register translation
│   └── vm.rs                  # Register-based execution
└── cli/                       # Command line interface
    ├── args.rs                # Argument parsing
    └── commands.rs            # Command dispatch
```

**Benefits of Modular Architecture:**
- **Maintainability** - Clear separation of concerns
- **Testability** - Individual modules can be tested in isolation
- **Extensibility** - Easy to add new features without affecting existing code
- **Performance** - Optimized compilation and reduced compile times
- **Collaboration** - Multiple developers can work on different modules

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

### Virtual Machine Core (`src/vm/`)
The heart of TinyTotVM, organized into specialized modules:

#### VM Engine (`src/vm/machine.rs`)
- **Stack-based execution model**
- **Instruction dispatch loop**  
- **Module loading and circular dependency detection**
- **Function call handling**
- **Exception processing**
- **Garbage collection integration**
- **Profiling and tracing support**

#### Value System (`src/vm/value.rs`)
- **Dynamic type system**
- **Type coercion and conversion**
- **Value serialization/deserialization**

#### Memory Management (`src/vm/memory.rs`)
- **Stack operations and bounds checking**
- **Variable scoping and frame management**
- **Call stack management**

#### Error Handling (`src/vm/errors.rs`)
- **Comprehensive error types**
- **Stack unwinding and cleanup**
- **Error reporting and context**

#### Instruction Set (`src/vm/opcode.rs`)
- **OpCode definitions**
- **Message patterns for concurrency**
- **Instruction parameter handling**

### Concurrency System (`src/concurrency/`)
BEAM-style actor model with complete fault tolerance:

#### Scheduler Pool (`src/concurrency/pool.rs`)
- **SMP work-stealing scheduler**
- **Multi-core process distribution**
- **Load balancing and fairness**

#### Process Isolation (`src/concurrency/process.rs`)
- **Isolated process state**
- **Message passing and mailboxes**
- **Process monitoring and linking**
- **Supervision tree integration**

#### Process Registry (`src/concurrency/registry.rs`)
- **Named process registration**
- **Process lifecycle management**
- **Name resolution and cleanup**

#### Supervision System (`src/concurrency/supervisor.rs`)
- **Fault tolerance strategies**
- **Automatic process restart**
- **Supervision tree management**

### Garbage Collection (`src/gc/`)
Pluggable garbage collection architecture:

#### Mark-Sweep GC (`src/gc/mark_sweep.rs`)
- **Traditional mark-and-sweep algorithm**
- **Root set identification**
- **Memory compaction**

#### No-Op GC (`src/gc/no_gc.rs`)
- **Disabled garbage collection**
- **Testing and benchmarking**

#### GC Statistics (`src/gc/stats.rs`)
- **Performance metrics**
- **Memory usage tracking**
- **Collection frequency analysis**

### Profiling and Debugging (`src/profiling/`)
Comprehensive performance analysis:

#### Profiler (`src/profiling/profiler.rs`)
- **Function-level timing**
- **Instruction counting**
- **Call frequency analysis**
- **Memory allocation tracking**

#### Statistics (`src/profiling/stats.rs`)
- **Performance reporting**
- **Color-coded output**
- **Trend analysis**

### Testing Framework (`src/testing/`)
Comprehensive test execution and reporting:

#### Test Harness (`src/testing/harness.rs`)
- **Test discovery and execution**
- **Result collection and aggregation**
- **Progress reporting**

#### Test Runner (`src/testing/runner.rs`)
- **Individual test execution**
- **Error handling and reporting**
- **Test isolation**

### Command Line Interface (`src/cli/`)
User-friendly command line interface:

#### Argument Parsing (`src/cli/args.rs`)
- **Command line option parsing**
- **Configuration validation**
- **Help and usage information**

#### Command Dispatch (`src/cli/commands.rs`)
- **Command routing and execution**
- **Error handling and reporting**
- **User feedback**

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

### 1. Parsing (`src/compiler.rs`)
```
Source Code → Tokenization → AST → Instruction Generation
```

**Features:**
- Symbolic and numeric label resolution
- Parameter validation
- Syntax error reporting
- Instruction optimization hints

### 2. Optimization (`src/optimizer.rs`)
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

### 3. Bytecode Generation (`src/bytecode.rs`)
Binary format for faster loading and unified instruction parsing:

```
Magic Header | Version | Instruction Count | Instructions | Metadata
```

**Key Features:**
- **Unified parsing** - Single parser for all instruction types
- **Label resolution** - Symbolic address resolution  
- **Module imports** - Automatic dependency loading
- **Error reporting** - Detailed parse error messages

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

## Intermediate Representation (IR) System

TinyTotVM includes an experimental register-based execution mode that translates stack-based bytecode to register-based intermediate representation.

### IR Architecture Overview
```rust
// Core IR data structures
pub enum RegInstr {
    Mov(RegId, RegValue),           // Move value to register
    Add(RegId, RegId, RegId),       // dst = src1 + src2
    Sub(RegId, RegId, RegId),       // dst = src1 - src2
    Jmp(usize),                     // Unconditional jump
    Jz(RegId, usize),              // Jump if register is zero
    Print(RegId),                   // Print register value
    Halt,                           // Stop execution
}

pub enum RegValue {
    Const(Value),                   // Immediate constant
    Reg(RegId),                     // Register reference
}

pub struct RegBlock {
    instructions: Vec<RegInstr>,
    register_count: u32,
    entry: usize,
}
```

### Stack-to-Register Translation
The lowering pass converts stack-based operations to register-based operations:

```rust
// Stack-based: PUSH_INT 5; PUSH_INT 3; ADD
// Becomes register-based:
Mov(r0, Const(Value::Int(5)))      // r0 = 5
Mov(r1, Const(Value::Int(3)))      // r1 = 3  
Add(r2, r0, r1)                    // r2 = r0 + r1
```

### Translation Process
1. **First Pass**: Build address mapping from bytecode to IR instructions
2. **Second Pass**: Translate each bytecode instruction to equivalent IR
3. **Stack Simulation**: Maintain virtual stack state using register allocation
4. **Register Allocation**: Allocate registers for intermediate values

### Register-Based Execution Engine
```rust
pub struct RegisterVM {
    registers: Vec<Value>,          // Register file
    variables: HashMap<String, Value>, // Named variables
    ip: usize,                      // Instruction pointer
    block: RegBlock,                // IR program
    halted: bool,                   // Execution state
}
```

### IR Benefits
- **Research Platform**: Experimental register-based execution
- **Performance Potential**: Register operations can be more efficient
- **Educational Value**: Demonstrates register allocation and IR translation
- **Architecture Comparison**: Direct comparison between stack and register execution

### Current Limitations
- **Concurrency Support**: IR mode doesn't support SPAWN, SEND, RECEIVE operations
- **Complex Control Flow**: Some edge cases in jump translation
- **Function Calls**: Limited support for function call semantics
- **Exception Handling**: No support for try/catch in IR mode

### Usage
```bash
# Enable IR execution mode
ttvm --use-ir examples/program.ttvm

# Compare with traditional stack execution
ttvm --no-smp examples/program.ttvm
```

## Design Principles

1. **Simplicity** - Clean, understandable code structure
2. **Modularity** - Separate concerns, pluggable components
3. **Safety** - No crashes, comprehensive error handling
4. **Performance** - Efficient execution with optimization
5. **Extensibility** - Easy to add features and instructions
6. **Educational Value** - Clear demonstration of VM concepts
7. **Cross-Platform** - Pure Rust, runs anywhere
8. **Hybrid Architecture** - Support both stack and register execution modes