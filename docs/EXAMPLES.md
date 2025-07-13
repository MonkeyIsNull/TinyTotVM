# Examples Guide

TinyTotVM includes 67 comprehensive example programs demonstrating all language features. This guide walks through the key examples and what they demonstrate.

## Basic Examples

### showcase.ttvm
Complete feature demonstration showing:
- Basic arithmetic and string operations
- Variable storage and retrieval
- Conditional logic and comparisons
- Object creation and manipulation
- Function calls and returns

```bash
ttvm examples/showcase.ttvm
```

### simple_profiling_test.ttvm
Demonstrates profiling and tracing capabilities:
- Function calls with parameters
- Performance measurement
- Instruction counting

```bash
ttvm --profile --trace examples/simple_profiling_test.ttvm
```

## Data Type Examples

### float_test.ttvm
IEEE 754 floating-point operations:
- Float arithmetic (ADD_F, SUB_F, MUL_F, DIV_F)
- Float comparisons with epsilon handling
- Type coercion between int and float

### object_test.ttvm
Dynamic object manipulation:
- Object creation with MAKE_OBJECT
- Field operations (SET_FIELD, GET_FIELD, HAS_FIELD)
- Object introspection with KEYS
- Nested object structures

### list_test.ttvm
Dynamic list operations:
- List creation with MAKE_LIST
- Element access with INDEX
- Length operations
- Heterogeneous data storage

## Function Examples

### function_test.ttvm
Basic function definitions and calls:
- Simple function with no parameters
- Function call and return mechanism
- Stack-based parameter passing

### function_args_test.ttvm
Function parameter handling:
- Multiple parameter functions
- Parameter passing and retrieval
- Local variable scoping

### function_pointer_test.ttvm
First-class functions:
- Creating function pointers with MAKE_FUNCTION
- Dynamic function calls with CALL_FUNCTION
- Functions as values

### higher_order_test.ttvm
Higher-order programming:
- Functions taking other functions as parameters
- Function composition
- Functional programming patterns

## Closure Examples

### simple_closure_test.ttvm
Basic closure functionality:
- Variable capture with CAPTURE
- Anonymous functions with MAKE_LAMBDA
- Lexical scoping demonstration

### closure_test.ttvm
Advanced closure patterns:
- Multiple variable capture
- Closure factories
- Environment isolation

### nested_closure_test.ttvm
Complex closure scenarios:
- Nested closure creation
- Multi-level variable capture
- Closure composition

### lambda_test.ttvm
Lambda expression examples:
- Anonymous function creation
- Captured environment handling
- Lambda as first-class values

## Module System Examples

### module_test.ttvm
Basic module functionality:
- Module import with IMPORT
- Function export with EXPORT
- Cross-module function calls

### comprehensive_module_test.ttvm
Advanced module patterns:
- Multiple module imports
- Complex function sharing
- Module namespace isolation

### closure_module_test.ttvm
Cross-module closures:
- Exporting closure factories
- Importing and using closures
- Captured variable preservation

### complex_closure_test.ttvm
Advanced cross-module closures:
- Nested closures across modules
- Complex environment sharing
- Module boundary testing

## Exception Handling Examples

### exception_test.ttvm
Basic exception handling:
- TRY/CATCH/END_TRY blocks
- Exception throwing with THROW
- Error message handling

### function_exception_test.ttvm
Function-level exception handling:
- Exceptions in function calls
- Stack unwinding demonstration
- Exception propagation

### nested_exception_test.ttvm
Nested exception scenarios:
- Multiple exception levels
- Inner and outer exception handling
- Exception re-throwing

### vm_error_exception_test.ttvm
VM error handling:
- Built-in VM errors as exceptions
- Type mismatch handling
- Stack underflow recovery

## Standard Library Examples

### stdlib_test.ttvm
Math library demonstration:
- Mathematical constants (PI, E)
- Math functions (sqrt, abs, max, min)
- Factorial implementation

### stdlib_string_test.ttvm
String utility functions:
- String concatenation and repetition
- Case conversion (upper, lower)
- String trimming and length

### stdlib_prelude_test.ttvm
Prelude module usage:
- Common function imports
- Simplified standard library access

### stdlib_comprehensive_test.ttvm
Complete standard library showcase:
- Math, string, list, and I/O operations
- Type conversion utilities
- Combined library usage patterns

## Optimization Examples

### optimization_test.ttvm
Basic optimization demonstration:
- Simple constant folding
- Dead code elimination
- Performance improvements

### constant_folding_test.ttvm
Constant folding optimization:
- Compile-time arithmetic evaluation
- Boolean expression optimization
- Constant propagation

### dead_code_test.ttvm
Dead code elimination:
- Unreachable code removal
- Conditional branch optimization
- Jump optimization

### tail_call_test.ttvm
Tail call optimization:
- Recursive function optimization
- Tail call detection
- Performance improvement demonstration

### comprehensive_optimization_test.ttvm
All optimization passes:
- Combined optimization techniques
- Performance measurement
- Instruction count reduction

## I/O Examples

### io_simple_test.ttvm
Basic I/O operations:
- File creation and reading
- FILE_EXISTS checking
- Simple file manipulation

### io_comprehensive_test.ttvm
Advanced I/O features:
- Environment variable access
- Command-line argument handling
- File and directory operations
- Process execution

### network_simple_test.ttvm
Basic networking:
- DNS resolution
- HTTP GET requests
- Simple network operations

### network_tcp_test.ttvm
TCP socket operations:
- TCP connection establishment
- Data sending and receiving
- Connection management

## Special Test Cases

### circular_a.ttvm / circular_b.ttvm
Circular dependency detection:
- Demonstrates import cycle detection
- Shows proper error handling
- Tests module system robustness

### delete.ttvm
Variable deletion:
- DELETE instruction usage
- Variable scope management
- Error handling for undefined variables

## Running Examples

### Individual Examples
```bash
# Basic functionality
ttvm examples/showcase.ttvm
ttvm examples/function_pointer_test.ttvm
ttvm examples/closure_test.ttvm

# Advanced features
ttvm examples/module_test.ttvm
ttvm examples/exception_test.ttvm
ttvm examples/stdlib_comprehensive_test.ttvm
```

### With Debugging and Optimization
```bash
# Debug mode
ttvm --debug examples/showcase.ttvm

# Optimization
ttvm --optimize examples/constant_folding_test.ttvm

# Profiling and tracing
ttvm --profile --trace examples/simple_profiling_test.ttvm

# Combined flags
ttvm --debug --optimize --gc-stats examples/showcase.ttvm
```

### Comprehensive Testing
```bash
# Run all examples
ttvm test-all

# Test specific categories
for test in examples/function*.ttvm; do ttvm "$test"; done
for test in examples/closure*.ttvm; do ttvm "$test"; done
for test in examples/stdlib*.ttvm; do ttvm "$test"; done
```

## Expected Output

Most examples produce specific output demonstrating their functionality. For example:

### showcase.ttvm output:
```
30
1
1
Not equal
Now equal!
```

### simple_profiling_test.ttvm with --profile:
```
=== Simple Profiling Test ===
15
Hello World

=== Profiling Results ===
┌───────────┬───────┬───────────┬──────────────┬────────────────────┐
│ Function  ┆ Calls ┆ Time (ms) ┆ Instructions ┆ Avg Time/Call (μs) │
╞═══════════╪═══════╪═══════════╪══════════════╪════════════════════╡
│ fn@0x000B ┆ 1     ┆ 0.026     ┆ 4            ┆ 26.0               │
│ fn@0x000F ┆ 1     ┆ 0.022     ┆ 4            ┆ 22.0               │
└───────────┴───────┴───────────┴──────────────┴────────────────────┘
```

## Creating New Examples

When creating new examples, follow these patterns:

1. **Clear documentation** - Comment the purpose and features
2. **Progressive complexity** - Start simple, build up
3. **Expected output** - Include comments showing expected results
4. **Error handling** - Demonstrate proper exception handling
5. **Feature focus** - Each example should focus on specific features

### Example Template
```assembly
; Example Name: Feature Description
; Demonstrates: List of features being shown

PUSH_STR "=== Example Name ==="
PRINT

; Feature demonstration code here
; ...

HALT
```