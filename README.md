# TinyTotVM

<img src="tiny_tot_vm_logo.png" alt="ttm" width="80%" />

> „Ohne Kaffee keine Erleuchtung – auch nicht für Maschinen."

**TinyTotVM** is a comprehensive, stack-based virtual machine written in Rust with advanced language features including float arithmetic, objects, function pointers, exception handling, and comprehensive debugging support.

## Features

### **Core Language Features**
- **Stack-based Architecture** - Efficient execution with pre-allocated memory
- **Dynamic Typing** - Int, Float, String, Bool, Null, List, Object, Function values
- **Type Coercion** - Automatic conversion between compatible types (int ↔ float)
- **Memory Management** - Automatic cleanup via Rust's ownership system

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

## Getting Started

### **Installation**
```bash
git clone https://github.com/yourusername/TinyTotVM
cd TinyTotVM
cargo build --release
```

### **Running Programs**
```bash
# Execute assembly directly
cargo run examples/program.ttvm

# Debug mode with step-by-step tracing
cargo run -- --debug examples/program.ttvm

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

- **Pre-allocated Stacks** - 1024-item stack, 64-item call stack
- **Instruction Counting** - Performance metrics and profiling
- **Memory Tracking** - Stack usage monitoring and statistics
- **Bytecode Compilation** - Binary format for faster program loading
- **Error Recovery** - Graceful error handling without crashes

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

## Architecture

TinyTotVM uses a clean, modular architecture:

- **`main.rs`** - Core VM implementation and execution engine
- **`compiler.rs`** - Assembly to bytecode compilation
- **`bytecode.rs`** - Binary format loading and processing
- **`lisp_compiler.rs`** - Lisp to assembly transpilation

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
    Function { addr: usize, params: Vec<String> },
    Exception(Box<Value>),
}
```

### **Error Handling**
Comprehensive error types with detailed messages:
- `StackUnderflow`, `TypeMismatch`, `UndefinedVariable`
- `IndexOutOfBounds`, `FileError`, `ParseError`
- `CallStackUnderflow`, `NoVariableScope`

## Educational Value

TinyTotVM serves as an excellent learning resource for:
- **Virtual Machine Design** - Stack-based execution model
- **Language Implementation** - Parsing, compilation, runtime systems
- **Type Systems** - Dynamic typing with coercion rules
- **Memory Management** - Scoping, stack management, garbage collection
- **Functional Programming** - First-class functions, higher-order programming
- **Exception Handling** - Structured error handling mechanisms

## Future Roadmap

- **Closures & Lambdas** - Anonymous functions with captured variables
- **Standard Library** - Math, string manipulation, data structure utilities
- **Optimization Passes** - Dead code elimination, constant folding
- **IDE Integration** - Language server protocol support
- **Package Manager** - Centralized module distribution and dependency management

## License

Free, as in free beer.

---

*TinyTotVM - Where tiny programs achieve big things!*