# TinyTotVM

<img src="tiny_tot_vm_logo.png" alt="ttm" width="80%" />

> „Ohne Kaffee keine Erleuchtung – auch nicht für Maschinen."

TinyTotVM is a tiny, stack-based virtual machine written in Rust.

This repo is, in essence, a toy-box for my experiments in writing a VM. It's not expected to be used for production usage in anything. That said, if you want to play around with it, have at it. At some point I'll update my long-term goals for it and post them someplace here, at least so I remember them.


## Braggart Version
**TinyTotVM** is a comprehensive, stack-based virtual machine written in Rust with advanced language features including float arithmetic, objects, function pointers, closures & lambdas, exception handling, module system, standard library, and comprehensive debugging support.

## Features

TinyTotVM provides a **complete functional programming runtime** with advanced capabilities:

**Core Runtime**: Stack-based VM with dynamic typing, automatic memory management, and comprehensive error handling  
**Data Types**: 64-bit integers, IEEE 754 floats, strings, booleans, dynamic lists, objects (HashMaps)  
**Functions**: First-class functions, closures with variable capture, higher-order programming  
**Modules**: Import/export system with circular dependency detection and cross-module closure support  
**Exception Handling**: Structured try/catch/throw with stack unwinding and nested exception support  
**Standard Library**: Comprehensive utility modules for math, strings, lists, I/O, and type conversion  
**Debugging**: Step-by-step execution tracing, performance metrics, breakpoint infrastructure  
**Performance**: Pre-allocated stacks, instruction counting, bytecode compilation, memory tracking  

### **Complete Feature Overview**

TinyTotVM is a **toy virtual machine** with features found in modern programming languages:

- **9 Built-in Data Types** - Complete type system with automatic memory management
- **55+ Core Instructions** - Full instruction set for all programming paradigms  
- **100+ Standard Library Functions** - Math, string, list, I/O, and conversion utilities
- **Advanced Function System** - First-class functions, closures, higher-order programming
- **Robust Module System** - Import/export with dependency management and closure support
- **Exception Handling** - Structured error handling with stack unwinding
- **Advanced Optimization Engine** - 8 optimization passes for performance
- **Memory Safety** - No crashes, comprehensive error handling, automatic cleanup
- **Cross-Platform** - Pure Rust implementation, runs anywhere Rust compiles
- **Educational Value** - Clean architecture demonstrating VM design principles
- **Extensible Design** - Easy to add new instructions, data types, and standard library modules  

### **Core Language Features**
- **Stack-based Architecture** - Efficient execution with pre-allocated memory
- **Dynamic Typing** - Int, Float, String, Bool, Null, List, Object, Bytes, Connection, Function, Closure values
- **Type Coercion** - Automatic conversion between compatible types (int <-> float)
- **Memory Management** - Automatic cleanup via Rust's ownership system
- **Error Handling** - Comprehensive Result-based error system instead of crashes

### **Advanced Data Types**
- **64-bit Integers** - Full arithmetic and comparison operations
- **IEEE 754 Floats** - Complete floating-point support with epsilon-based equality
- **Dynamic Objects** - JavaScript-like objects with nested data structures
- **Dynamic Lists** - Heterogeneous arrays supporting any value type
- **First-Class Functions** - Function pointers enabling functional programming

### **Object System**
```assembly
MAKE_OBJECT
PUSH_STR "John Doe"
SET_FIELD name
PUSH_INT 25
SET_FIELD age
```
- **Field Access** - `GET_FIELD`, `SET_FIELD`, `HAS_FIELD`, `DELETE_FIELD`
- **Object Introspection** - `KEYS` (list all keys), `LEN` (field count)
- **Nested Objects** - Unlimited depth object composition
- **Mixed Types** - Objects can contain any combination of value types

### **Function Pointers & Higher-Order Programming**
```assembly
MAKE_FUNCTION double x    ; Create function pointer
STORE doubleFunc         ; Store in variable
PUSH_FLOAT 5.0          ; Push argument
LOAD doubleFunc         ; Load function
CALL_FUNCTION           ; Dynamic dispatch
```
- **Dynamic Dispatch** - Runtime function calls via stored pointers
- **Parameter Binding** - Automatic parameter mapping
- **Functions as Values** - Store in variables, objects, and lists
- **Higher-Order Functions** - Functions that take/return other functions

### **Exception Handling**
```assembly
TRY
    PUSH_INT 10
    PUSH_INT 0
    DIV_F                ; May throw divide-by-zero
CATCH error
    LOAD error
    PRINT               ; Handle the exception
END_TRY
```
- **Structured Exception Handling** - try/catch/throw mechanism
- **Stack Unwinding** - Automatic cleanup during exception propagation
- **Nested Exceptions** - Multiple exception handling levels
- **Exception Values** - Exceptions carry data for detailed error reporting

### **Module System**
```assembly
; math_module.ttvm
MAKE_FUNCTION add_func x y
STORE add
EXPORT add

; main.ttvm  
IMPORT examples/math_module.ttvm
PUSH_INT 5
PUSH_INT 3
LOAD add
CALL_FUNCTION           ; Outputs: 8
```
- **Module Import/Export** - `IMPORT` modules, `EXPORT` symbols
- **Function Sharing** - Export and import functions between modules
- **Namespace Isolation** - Modules have separate variable scopes
- **Circular Dependency Detection** - Prevents infinite import loops
- **Address Resolution** - Automatic function address adjustment for imports
- **Caching** - Modules loaded once and cached for performance
- **Cross-Module Closures** - Closures work correctly across module boundaries

### **Closures & Lambdas**
```assembly
; Capture variables for closure
PUSH_INT 10
STORE base
CAPTURE base
MAKE_LAMBDA add_lambda x
STORE adder

; Use the closure
PUSH_INT 5
LOAD adder
CALL_FUNCTION           ; Outputs: 15
```
- **Anonymous Functions** - `MAKE_LAMBDA` creates closures without explicit naming
- **Variable Capture** - `CAPTURE` binds variables into closure environment
- **Lexical Scoping** - Closures capture variables from creation context
- **Captured Environment** - Variables captured by value at closure creation time
- **Higher-Order Programming** - Closures can be passed as parameters and returned
- **Environment Isolation** - Each closure maintains its own captured variable state
- **Cross-Module Support** - Closures exported from modules work correctly after import
- **Nested Closures** - Support for complex multi-level closure factories

### **Standard Library**
```assembly
; Import the prelude for commonly used functions
IMPORT std/prelude.ttvm

; Math functions
PUSH_FLOAT 25.0
LOAD sqrt
CALL_FUNCTION          ; Outputs: 5.0

; String utilities  
PUSH_STR "Hello"
PUSH_STR " World"
LOAD strcat
CALL_FUNCTION          ; Outputs: "Hello World"

; List operations
PUSH_INT 1
PUSH_INT 2  
PUSH_INT 3
MAKE_LIST 3
LOAD sum
CALL_FUNCTION          ; Outputs: sum of elements
```
- **Modular Architecture** - Organized into focused utility modules
- **Math Library** - Constants (PI, E), functions (sqrt, abs, max, min, factorial)
- **String Utilities** - Concatenation, repetition, case conversion, trimming
- **List Operations** - Length, first/last, higher-order functions (map, filter, reduce)
- **Type Conversion** - Convert between types with safety checks
- **I/O Utilities** - Comprehensive I/O operations including file, environment, and process control
- **Prelude Module** - Common functions imported together for convenience

### **Advanced Optimization Engine**
```bash
# Run with optimizations enabled
cargo run -- --optimize examples/program.ttvm

# Optimize and save program
cargo run -- optimize input.ttvm optimized.ttvm
```
TinyTotVM features a sophisticated 8-pass optimization engine that provides significant performance improvements:

**Optimization Passes:**
- **Constant Folding** - Evaluates arithmetic and boolean expressions at compile time
- **Constant Propagation** - Replaces variable loads with known constant values
- **Dead Code Elimination** - Removes unreachable code after jumps and branches
- **Peephole Optimizations** - Optimizes small instruction sequences (identity operations, double negation)
- **Instruction Combining** - Combines multiple instructions into efficient forms
- **Jump Threading** - Optimizes chains of jumps to jump directly to final targets
- **Tail Call Optimization** - Converts tail recursive calls to jumps
- **Memory Layout Optimization** - Optimizes redundant memory access patterns

**Performance Results:**
- **Up to 71% instruction reduction** in dead code elimination
- **46% instruction reduction** in comprehensive optimization tests
- **37% instruction reduction** in constant folding tests
- **Detailed optimization statistics** with granular reporting per optimization type

### **Control Flow & Functions**
- **Conditional Jumps** - `JMP`, `JZ` (jump if zero/false/null)
- **Function Calls** - `CALL` with parameter passing and `RET`
- **Lexical Scoping** - Proper variable scoping with call frames
- **Label System** - Support for both numeric addresses and symbolic labels

### **Performance & Debugging**
```bash
cargo run -- --debug examples/program.ttvm
```
- **Debug Mode** - Step-by-step execution tracing with `--debug` flag
- **Performance Stats** - Instruction count, max stack usage, memory tracking
- **Breakpoint Infrastructure** - Built-in debugging support
- **Error Handling** - Comprehensive error messages instead of crashes
- **Memory Optimization** - Pre-allocated stacks for better performance

### **Language Interoperability**
- **Native Assembly** - Direct `.ttvm` instruction format
- **Lisp Transpiler** - Compile Lisp code to TinyTotVM assembly
- **Bytecode Compilation** - Binary `.ttb` format for faster loading
- **Multiple Parsers** - Support for both numeric and symbolic addressing

## Instruction Set

**55+ Core Instructions** plus **comprehensive standard library** covering all aspects of modern programming language execution:

### **Arithmetic & Logic**
```
ADD, SUB             ; Integer arithmetic with type coercion
ADD_F, SUB_F, MUL_F, DIV_F  ; Float arithmetic
EQ, NE, GT, LT, GE, LE      ; Integer comparisons
EQ_F, NE_F, GT_F, LT_F, GE_F, LE_F  ; Float comparisons
AND, OR, NOT         ; Boolean operations
```

### **Stack Operations**
```
PUSH_INT 42          ; Push integer literal
PUSH_FLOAT 3.14      ; Push float literal
PUSH_STR "hello"     ; Push string literal
TRUE, FALSE, NULL    ; Push boolean/null constants
DUP                  ; Duplicate top stack value
```

### **Variables & Scoping**
```
STORE varname        ; Store value in current scope
LOAD varname         ; Load variable value
DELETE varname       ; Remove variable from scope
DUMP_SCOPE          ; Debug: print current scope
```

### **Objects & Collections**
```
MAKE_OBJECT         ; Create empty object
SET_FIELD name      ; Set object field
GET_FIELD name      ; Get object field
HAS_FIELD name      ; Check if field exists
DELETE_FIELD name   ; Remove object field
KEYS               ; Get all field names as list

MAKE_LIST 3        ; Create list from top 3 stack items
LEN                ; Get length of list/object
INDEX              ; Access list element by index
```

### **Functions & Control Flow**
```
CALL label param1 param2    ; Call function with parameters
RET                         ; Return from function
MAKE_FUNCTION label x y     ; Create function pointer
CALL_FUNCTION              ; Call function from stack

MAKE_LAMBDA label x y       ; Create anonymous function (closure)
CAPTURE varname             ; Capture variable for closure

JMP label          ; Unconditional jump
JZ label           ; Jump if zero/false/null
```

### **Exception Handling**
```
TRY                ; Start exception handling block
CATCH varname      ; Catch exceptions in variable
THROW              ; Throw exception from stack
END_TRY           ; End exception handling block
```

### **Module System**
```
IMPORT path        ; Import module by file path
EXPORT name        ; Export variable/function by name
```

### **I/O & Debugging**
```
PRINT              ; Print top stack value
READ_FILE          ; Read file contents to string
WRITE_FILE         ; Write string to file
READ_LINE          ; Read line from stdin
READ_CHAR          ; Read single character from stdin
READ_INPUT         ; Read all input until EOF from stdin
APPEND_FILE        ; Append content to file
FILE_EXISTS        ; Check if file exists
FILE_SIZE          ; Get file size in bytes
DELETE_FILE        ; Delete a file
LIST_DIR           ; List directory contents
READ_BYTES         ; Read file as byte array
WRITE_BYTES        ; Write byte array to file
GET_ENV            ; Get environment variable
SET_ENV            ; Set environment variable
GET_ARGS           ; Get command line arguments
EXEC               ; Execute external command
EXEC_CAPTURE       ; Execute command and capture output
EXIT               ; Exit with status code
GET_TIME           ; Get current Unix timestamp
SLEEP              ; Sleep for specified milliseconds
FORMAT_TIME        ; Format timestamp to string
HTTP_GET           ; HTTP GET request
HTTP_POST          ; HTTP POST request
TCP_CONNECT        ; Connect to TCP server
TCP_LISTEN         ; Listen on TCP port
TCP_SEND           ; Send data over TCP connection
TCP_RECV           ; Receive data from TCP connection
UDP_BIND           ; Bind UDP socket to port
UDP_SEND           ; Send UDP packet
UDP_RECV           ; Receive UDP packet
DNS_RESOLVE        ; Resolve hostname to IP address
HALT               ; Stop execution
```

## Usage Examples

### **Basic Program**
```assembly
PUSH_STR "Hello, "
PUSH_STR "TinyTot world!"
CONCAT
PRINT

PUSH_INT 2
PUSH_INT 3
ADD
PRINT

HALT
```

### **Object-Oriented Programming**
```assembly
; Create person object
MAKE_OBJECT
PUSH_STR "Alice"
SET_FIELD name
PUSH_INT 30
SET_FIELD age

; Create address object
MAKE_OBJECT
PUSH_STR "123 Main St"
SET_FIELD street
PUSH_STR "Boston"
SET_FIELD city

; Nest address in person
SET_FIELD address

; Access nested data
DUP
GET_FIELD address
GET_FIELD city
PRINT              ; Outputs: Str("Boston")
```

### **Functional Programming**
```assembly
; Define a function
LABEL multiply
LOAD x
LOAD y
MUL_F
RET

; Create and use function pointer
MAKE_FUNCTION multiply x y
STORE mult_func

PUSH_FLOAT 3.0
PUSH_FLOAT 4.0
LOAD mult_func
CALL_FUNCTION      ; Outputs: Float(12.0)
```

### **Exception Handling**
```assembly
TRY
    PUSH_STR "test.txt"
    READ_FILE
    PRINT
CATCH error
    PUSH_STR "File not found: "
    LOAD error
    CONCAT
    PRINT
END_TRY
```

### **Closures & Anonymous Functions**
```assembly
; Create a closure factory
PUSH_INT 5
STORE increment
CAPTURE increment
MAKE_LAMBDA add_lambda x
STORE adder

; Use the closure
PUSH_INT 10
LOAD adder
CALL_FUNCTION          ; Outputs: 15

; Closures maintain captured state
PUSH_INT 999
STORE increment        ; Change original variable

PUSH_INT 3
LOAD adder            ; Closure still uses captured value (5)
CALL_FUNCTION         ; Outputs: 8
```

## Quick Start

### **Simple Example with Standard Library**
```assembly
; Import standard library prelude
IMPORT std/prelude.ttvm

; Mathematical computation
PUSH_FLOAT 25.0
LOAD sqrt
CALL_FUNCTION
PRINT                  ; Outputs: 5.0

; String manipulation
PUSH_STR "Hello"
PUSH_STR " TinyTotVM"
LOAD strcat
CALL_FUNCTION
PRINT                  ; Outputs: "Hello TinyTotVM"

; List processing with higher-order functions
PUSH_INT 1
PUSH_INT 2
PUSH_INT 3
MAKE_LIST 3
LOAD sum
CALL_FUNCTION
PRINT                  ; Outputs: 6

; Type conversion and logging
PUSH_INT 42
LOAD to_string
CALL_FUNCTION
LOAD log_info
CALL_FUNCTION          ; Outputs: "INFO: converted_to_string"

HALT
```

## Comprehensive Testing

TinyTotVM includes extensive test suites to verify all functionality. Run these tests to validate the VM's capabilities:

### **Core VM Tests**
```bash
# Basic functionality tests
cargo run examples/showcase.ttvm
cargo run examples/float_test.ttvm
cargo run examples/object_test.ttvm

# Function and closure tests  
cargo run examples/function_pointer_test.ttvm
cargo run examples/closure_test.ttvm
cargo run examples/nested_closure_test.ttvm

# Module system tests
cargo run examples/module_test.ttvm
cargo run examples/comprehensive_module_test.ttvm
cargo run examples/closure_module_test.ttvm

# Exception handling tests
cargo run examples/exception_test.ttvm
```

### **Standard Library Tests**
```bash
# Individual library tests
cargo run examples/stdlib_test.ttvm
cargo run examples/stdlib_string_test.ttvm
cargo run examples/stdlib_prelude_test.ttvm

# Comprehensive standard library test
cargo run examples/stdlib_comprehensive_test.ttvm
```

### **Optimization Engine Tests**
```bash
# Basic optimizations
cargo run -- --optimize examples/constant_folding_test.ttvm
cargo run -- --optimize examples/dead_code_test.ttvm
cargo run -- --optimize examples/tail_call_test.ttvm

# Advanced optimizations
cargo run -- --optimize examples/advanced_optimization_test.ttvm
cargo run -- --optimize examples/safe_advanced_optimization_test.ttvm

# Complete optimization showcase
cargo run -- --optimize examples/complete_optimization_showcase.ttvm

# Save optimized programs
cargo run -- optimize examples/constant_folding_test.ttvm /tmp/optimized.ttvm
cargo run /tmp/optimized.ttvm
```

### **Debug and Analysis**
```bash
# Debug mode with step-by-step tracing
cargo run -- --debug examples/showcase.ttvm

# Combined debug and optimization
cargo run -- --debug --optimize examples/program.ttvm

# Optimization analysis only
cargo run -- optimize examples/program.ttvm /tmp/optimized.ttvm
```

### **Performance Benchmarks**
```bash
# Compare performance with and without optimizations
time cargo run examples/comprehensive_optimization_test.ttvm
time cargo run -- --optimize examples/comprehensive_optimization_test.ttvm

# Large program optimization
time cargo run -- --optimize examples/stdlib_comprehensive_test.ttvm
```

### **Expected Results**
When running optimization tests, you should see results like:
```
=== Optimization Results ===
Instructions: 49 -> 31 (18)
Constants folded: 18
Dead instructions removed: 0
Tail calls optimized: 0
Memory operations optimized: 0
Peephole optimizations: 0
Constants propagated: 0
Instructions combined: 0
Jumps threaded: 0
```

### **Validation Commands**
```bash
# Run all core tests
for test in examples/*.ttvm; do echo "Testing $test"; cargo run "$test" || echo "FAILED: $test"; done

# Run all optimization tests  
for test in examples/*optimization*.ttvm; do echo "Optimizing $test"; cargo run -- --optimize "$test" || echo "FAILED: $test"; done

# Comprehensive validation
cargo run examples/showcase.ttvm && \
cargo run examples/stdlib_comprehensive_test.ttvm && \
cargo run -- --optimize examples/complete_optimization_showcase.ttvm && \
echo "All tests passed!"
```

## Getting Started

### **Installation**
```bash
git clone https://github.com/MonkeyIsNull/TinyTotVM
cd TinyTotVM
cargo build --release
```

### **Running Programs**
```bash
# Execute assembly directly
cargo run examples/program.ttvm

# Debug mode with step-by-step tracing
cargo run -- --debug examples/program.ttvm

# Run with optimizations enabled
cargo run -- --optimize examples/program.ttvm

# Optimize and save program
cargo run -- optimize input.ttvm optimized.ttvm

# Compile to bytecode and run
cargo run -- compile examples/program.ttvm program.ttb
cargo run program.ttb
```

### **Lisp Interoperability**
```bash
# Compile Lisp to TinyTotVM assembly
cargo run -- compile-lisp examples/program.lisp program.ttvm

# Run the compiled assembly
cargo run program.ttvm
```

## Performance Features

TinyTotVM is designed for efficiency and experimental use:

- **Pre-allocated Stacks** - 1024-item stack, 64-item call stack for optimal performance
- **Instruction Counting** - Performance metrics and profiling with execution statistics
- **Memory Tracking** - Stack usage monitoring and memory optimization
- **Bytecode Compilation** - Binary `.ttb` format for faster program loading
- **Error Recovery** - Graceful error handling without crashes or panics
- **Efficient Function Calls** - Optimized call stack and parameter passing
- **Smart Address Resolution** - Automatic function pointer adjustment in modules
- **Closure Optimization** - Efficient captured environment management
- **Advanced Optimization Engine** - 8 optimization passes including constant folding, dead code elimination, peephole optimizations, constant propagation

## Example Programs

The `examples/` directory contains comprehensive test programs:

- **`showcase.ttvm`** - Complete feature demonstration
- **`float_test.ttvm`** - Float arithmetic and comparisons
- **`object_test.ttvm`** - Object manipulation and introspection
- **`function_pointer_test.ttvm`** - Dynamic function calls
- **`higher_order_test.ttvm`** - Functional programming patterns
- **`nested_object_test.ttvm`** - Complex data structures
- **`exception_test.ttvm`** - Exception handling examples
- **`math_module.ttvm`** - Module system demonstration with math functions
- **`module_test.ttvm`** - Basic module import and usage
- **`comprehensive_module_test.ttvm`** - Advanced module usage patterns
- **`circular_a.ttvm` / `circular_b.ttvm`** - Circular dependency detection test
- **`simple_closure_test.ttvm`** - Basic closure and lambda functionality
- **`closure_test.ttvm`** - Advanced closure testing with variable capture
- **`nested_closure_test.ttvm`** - Nested closures and higher-order functions
- **`lambda_test.ttvm`** - Anonymous functions and lambda expressions
- **`closure_module.ttvm`** - Module exporting closure factories
- **`closure_module_test.ttvm`** - Cross-module closure functionality
- **`complex_closure_module.ttvm`** - Nested closures across modules
- **`complex_closure_test.ttvm`** - Advanced cross-module closure testing
- **`stdlib_test.ttvm`** - Math library demonstration
- **`stdlib_string_test.ttvm`** - String utilities demonstration
- **`stdlib_prelude_test.ttvm`** - Prelude module demonstration  
- **`stdlib_comprehensive_test.ttvm`** - Complete standard library showcase
- **`optimization_test.ttvm`** - General optimization capabilities demonstration
- **`constant_folding_test.ttvm`** - Constant folding optimization examples
- **`dead_code_test.ttvm`** - Dead code elimination demonstration
- **`tail_call_test.ttvm`** - Tail call optimization examples
- **`memory_optimization_test.ttvm`** - Memory access pattern optimization
- **`comprehensive_optimization_test.ttvm`** - Combined optimization techniques
- **`advanced_optimization_test.ttvm`** - Advanced optimization features test
- **`safe_advanced_optimization_test.ttvm`** - Safe advanced optimization demonstration
- **`complete_optimization_showcase.ttvm`** - Complete demonstration of all 8 optimization passes
- **`io_comprehensive_test.ttvm`** - Comprehensive test of all new I/O operations
- **`io_interactive_test.ttvm`** - Interactive I/O demonstration (stdin/stdout)
- **`io_simple_test.ttvm`** - Simple I/O test for basic file operations
- **`stdlib_enhanced_io_test.ttvm`** - Enhanced I/O standard library demonstration
- **`network_simple_test.ttvm`** - Basic network operations test
- **`network_tcp_test.ttvm`** - TCP socket operations demonstration
- **`network_udp_test.ttvm`** - UDP socket operations demonstration
- **`stdlib_network_test.ttvm`** - Network standard library demonstration

## Architecture

TinyTotVM uses a clean, modular architecture:

- **`main.rs`** - Core VM implementation and execution engine
- **`compiler.rs`** - Assembly to bytecode compilation
- **`bytecode.rs`** - Binary format loading and processing
- **`lisp_compiler.rs`** - Lisp to assembly transpilation
- **`optimizer.rs`** - Advanced optimization engine with multiple optimization passes
- **`std/`** - Standard library modules for enhanced functionality
- **`docs/`** - Comprehensive documentation including optimization guide

### **Value System**
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
    Function { addr: usize, params: Vec<String> },
    Closure { addr: usize, params: Vec<String>, captured: HashMap<String, Value> },
    Exception { message: String, stack_trace: Vec<String> },
}
```

### **Error Handling**
Comprehensive error types with detailed messages:
- `StackUnderflow`, `TypeMismatch`, `UndefinedVariable`
- `IndexOutOfBounds`, `FileError`, `ParseError`
- `CallStackUnderflow`, `NoVariableScope`

## Capability Matrix

| **Feature Category** | **Capabilities** | **Status** |
|---|---|---|
| **Data Types** | Int64, Float64, String, Bool, Null, List, Object, Bytes, Connection, Function, Closure |  Complete |
| **Arithmetic** | Full integer/float arithmetic with type coercion |  Complete |
| **Control Flow** | Jumps, conditionals, function calls, returns |  Complete |
| **Functions** | First-class functions, parameters, dynamic dispatch |  Complete |
| **Closures** | Variable capture, lexical scoping, nested closures |  Complete |
| **Objects** | Dynamic objects, field access, introspection |  Complete |
| **Modules** | Import/export, circular dependency detection |  Complete |
| **Exceptions** | Structured try/catch/throw with stack unwinding |  Complete |
| **Debugging** | Step-by-step tracing, breakpoints, performance metrics |  Complete |
| **I/O** | File operations, stdin/stdout, environment access, process execution, time operations, network I/O |  Complete |
| **Memory** | Automatic management, efficient stack allocation |  Complete |
| **Standard Library** | Math, string, list, I/O, and conversion utilities |  Complete |

## Educational Value

TinyTotVM serves as an excellent learning resource for:
- **Virtual Machine Design** - Stack-based execution model with modern features
- **Language Implementation** - Complete runtime with parsing, compilation, and execution
- **Type Systems** - Dynamic typing with coercion rules and type safety
- **Memory Management** - Scoping, stack management, and automatic cleanup
- **Functional Programming** - First-class functions, closures, and higher-order programming
- **Exception Handling** - Structured error handling with proper stack unwinding
- **Module Systems** - Code organization and dependency management

## What the heck is this thing good for? 

- **Educational Use** - Complete implementation of modern VM concepts with rich functionality
- **Research Projects** - Extensible architecture for language research with ready-to-use utilities
- **Embedded Scripting** - Lightweight runtime with full standard library for applications
- **Prototyping** - Rapid development of domain-specific languages with built-in utilities
- **Experimental Applications** - Runtime with math, string, list, and I/O operations

**Key Features:**
- **Standard Library** - 100+ utility functions across math, strings, lists, I/O, and type conversion
- **Advanced Optimization Engine** - 8 optimization passes with up to 71% instruction reduction
- **Comprehensive Error Handling** - No crashes or panics, detailed error messages
- **Memory Safety** - Automatic cleanup, stack overflow protection, resource management
- **Module System** - Robust import/export with circular dependency detection
- **Advanced Functions** - Closures, higher-order functions, cross-module compatibility
- **Debugging Tools** - Step-by-step tracing, performance metrics, breakpoint support
- **Cross-Platform** - Pure Rust implementation, runs on all major platforms
- **Battle-Tested** - Extensive test suite covering core VM and standard library functionality

## Future Roadmap

- **Enhanced Standard Library** - Extended math functions (trigonometry), regex support, date/time utilities
- **Advanced Optimizations** - Peephole optimizations, constant propagation, instruction combining, loop optimizations
- **IDE Integration** - Language server protocol support with syntax highlighting
- **Package Manager** - Centralized module distribution and dependency management
- **Mutable Closures** - Support for mutable captured variables and reference semantics
- **JIT Compilation** - Just-in-time compilation for performance-critical code
- **Native Extensions** - FFI support for calling external libraries

## License

Free, as in free beer.

---
