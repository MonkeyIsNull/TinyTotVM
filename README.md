# TinyTotVM

<img src="tiny_tot_vm_logo.png" alt="ttm" width="80%" />

> „Ohne Kaffee keine Erleuchtung – auch nicht für Maschinen."

TinyTotVM is a tiny virtual machine written in Rust with both stack-based and register-based execution modes.

This repo is, in essence, a toy-box for my experiments in writing a VM. It's not expected to be used for production usage in anything. That said, if you want to play around with it, have at it. At some point I'll update my long-term goals for it and post them someplace here, at least so I remember them.

## Features

TinyTotVM provides a **complete functional programming runtime** with advanced capabilities:

- **Hybrid Architecture**: Both stack-based and register-based execution modes with IR (Intermediate Representation)
- **Core Runtime**: Dynamic typing, automatic memory management
- **Data Types**: 64-bit integers, IEEE 754 floats, strings, booleans, dynamic lists, objects
- **Functions**: First-class functions, closures with variable capture, higher-order programming
- **Modules**: Import/export system with circular dependency detection
- **Exception Handling**: Structured try/catch/throw with stack unwinding
- **Standard Library**: Comprehensive utility modules for math, strings, lists, I/O
- **Debugging**: Step-by-step execution tracing, performance metrics, profiling support
- **Performance**: Pre-allocated stacks, instruction counting, advanced optimization engine
- **Pluggable GC**: Multiple garbage collection engines with runtime selection
- **Testing**: 93 comprehensive tests with color-coded formatted output
- **BEAM-Style Concurrency**: Actor model with process monitoring, linking, supervision trees, and named processes (SMP enabled by default)

## Quick Start

### Building
```bash
git clone https://github.com/MonkeyIsNull/TinyTotVM
cd TinyTotVM
cargo build --release
```

### Basic Usage
```bash
# Run a program
ttvm examples/showcase.ttvm

# With debugging
ttvm --debug examples/showcase.ttvm

# With optimizations
ttvm --optimize examples/showcase.ttvm

# With profiling and tracing
ttvm --profile --trace examples/simple_profiling_test.ttvm

# With garbage collection options
ttvm --gc mark-sweep --gc-stats examples/showcase.ttvm

# With single-threaded mode (disable SMP)
ttvm --no-smp examples/showcase.ttvm

# With register-based IR execution
ttvm --use-ir examples/showcase.ttvm
```

### Profiling and Tracing

TinyTotVM includes powerful **profiling and tracing** capabilities for performance analysis and debugging:

```bash
# Enable instruction-level tracing
ttvm --trace examples/program.ttvm

# Enable function performance profiling
ttvm --profile examples/program.ttvm

# Use both together
ttvm --trace --profile examples/program.ttvm

# Plain text output for automation
ttvm --profile --no-table examples/program.ttvm
```

**Tracing Output** (with color coding):
```
[trace] PushInt(10) @ 0x0002
[trace] Call { addr: 11, params: ["a", "b"] } @ 0x0004
[trace] CALL fn@0x000B with 2 params
[trace]   Load("a") @ 0x000B
[trace]   Add @ 0x000D
[trace] RETURN from fn@0x000B → Int(15)
```

**Profiling Output** (with performance-based color coding):
```
=== Profiling Results ===
┌───────────┬───────┬───────────┬──────────────┬────────────────────┐
│ Function  ┆ Calls ┆ Time (ms) ┆ Instructions ┆ Avg Time/Call (μs) │
╞═══════════╪═══════╪═══════════╪══════════════╪════════════════════╡
│ fn@0x000B ┆ 1     ┆ 0.026     ┆ 4            ┆ 26.0               │
└───────────┴───────┴───────────┴──────────────┴────────────────────┘
```
*Performance metrics are color-coded: green (fast), yellow (moderate), red (slow)*

### Testing
```bash
# Run all comprehensive tests
ttvm test-all

# Run built-in unit tests
ttvm --run-tests

# Test individual features
ttvm examples/function_test.ttvm
ttvm examples/closure_test.ttvm
ttvm examples/module_test.ttvm
```

## Documentation

For detailed information, see:

- **[Instruction Set Reference](docs/INSTRUCTION_SET.md)** - Complete instruction documentation
- **[Language Features Guide](docs/LANGUAGE_FEATURES.md)** - Functions, closures, modules, exceptions
- **[Standard Library Documentation](docs/STANDARD_LIBRARY.md)** - Math, string, I/O, and utility functions
- **[Optimization Guide](docs/OPTIMIZATION.md)** - Advanced optimization engine and performance
- **[Examples Guide](docs/EXAMPLES.md)** - Complete walkthrough of example programs
- **[Architecture Documentation](docs/ARCHITECTURE.md)** - VM design and implementation details
- **[BEAM-Style Concurrency Guide](docs/CONCURRENCY.md)** - Complete SMP scheduler, process isolation, and fault tolerance

## Example Programs

The `examples/` directory contains 93 comprehensive test programs demonstrating all features:

- **`showcase.ttvm`** - Complete feature demonstration
- **`simple_profiling_test.ttvm`** - Profiling and tracing demonstration
- **`function_pointer_test.ttvm`** - Dynamic function calls
- **`closure_test.ttvm`** - Variable capture and lexical scoping
- **`module_test.ttvm`** - Module import/export system
- **`exception_test.ttvm`** - Exception handling examples
- **`stdlib_comprehensive_test.ttvm`** - Complete standard library showcase
- **`complete_optimization_showcase.ttvm`** - All 8 optimization passes
- **`concurrency_test.ttvm`** - BEAM-style concurrency with YIELD instruction
- **`coffee_shop_demo.ttvm`** - Multi-actor message passing demo (Customer/Cashier/Barista)


## Command Line Options

```bash
ttvm [OPTIONS] <program.ttvm>

OPTIONS:
  --debug               Enable step-by-step execution tracing
  --optimize           Enable 8-pass optimization engine
  --gc <type>          Garbage collector: mark-sweep, no-gc
  --gc-debug           Show GC allocation/collection debug info
  --gc-stats           Display GC performance statistics
  --trace              Enable instruction-level tracing
  --profile            Enable function performance profiling
  --run-tests          Run built-in unit tests
  --no-table           Use plain text output instead of formatted tables
  --no-smp             Disable SMP scheduler (use single-threaded mode)
  --use-ir             Enable register-based IR execution mode

COMMANDS:
  ttvm test-all                           # Run all example tests
  ttvm test-concurrency                   # Test concurrency features
  ttvm test-register-whereis              # Test REGISTER and WHEREIS opcodes
  ttvm test-yield-comprehensive           # Test YIELD opcode thoroughly
  ttvm test-spawn-comprehensive           # Test SPAWN opcode
  ttvm test-send-receive-comprehensive    # Test SEND/RECEIVE message passing
  ttvm test-concurrency-bytecode         # Test bytecode compilation of concurrency
  ttvm test-smp-concurrency              # Test SMP scheduler with concurrency
  ttvm test-coffee-shop                  # Test coffee shop actor model demo
  ttvm optimize <input> <output>          # Optimize and save program
  ttvm compile <input.ttvm> <output.ttb>  # Compile to bytecode
  ttvm compile-lisp <input.lisp> <output.ttvm>  # Transpile Lisp
```

## What's This Good For?

- **Educational Use** - Complete implementation of modern VM concepts
- **Research Projects** - Extensible architecture for language research
- **Embedded Scripting** - Lightweight runtime with full standard library
- **Prototyping** - Rapid development of domain-specific languages
- **Experimentation** - Runtime with comprehensive math, string, I/O operations

## Execution Modes

TinyTotVM supports two execution modes that can be selected at runtime:

### Stack-Based Execution (Default)
The traditional stack-based virtual machine that executes bytecode directly using a stack for operand storage. This mode provides:
- Simple instruction semantics
- Direct bytecode interpretation
- Full compatibility with all TinyTotVM features including concurrency
- Mature and stable implementation

### Register-Based IR Execution (`--use-ir`)
An advanced register-based execution mode using Intermediate Representation (IR). This mode provides:
- **Stack-to-Register Translation**: Automatically converts stack-based bytecode to register-based IR
- **Register Allocation**: Efficient register management with spill handling
- **Optimized Execution**: Register-based operations for improved performance potential
- **Research Platform**: Experimental mode for studying register-based VM architectures

```bash
# Use traditional stack-based execution (default)
ttvm examples/program.ttvm

# Use register-based IR execution
ttvm --use-ir examples/program.ttvm
```

**Note**: The IR mode is experimental and currently supports basic arithmetic, control flow, and simple programs. Complex features like concurrency operations (SPAWN, SEND, RECEIVE) are not yet supported in IR mode.

## Architecture

TinyTotVM uses a clean, modular architecture organized into logical modules:

### Core Modules
- **`src/main.rs`** - Main entry point and CLI interface
- **`src/lib.rs`** - Library interface and public API
- **`src/bytecode.rs`** - Bytecode parsing and instruction decoding

### Virtual Machine Core (`src/vm/`)
- **`machine.rs`** - Main VM execution engine and runtime
- **`value.rs`** - Dynamic value types and operations
- **`opcode.rs`** - Instruction definitions and message patterns
- **`stack.rs`** - Stack management and operations
- **`memory.rs`** - Memory management and variable scoping
- **`errors.rs`** - VM error types and handling

### Concurrency System (`src/concurrency/`)
- **`pool.rs`** - SMP scheduler pool and work-stealing
- **`process.rs`** - Process isolation and actor model
- **`scheduler.rs`** - Individual scheduler threads
- **`registry.rs`** - Process registry and name resolution
- **`supervisor.rs`** - Supervision trees and fault tolerance
- **`messages.rs`** - Inter-process message types

### Garbage Collection (`src/gc/`)
- **`mark_sweep.rs`** - Mark-and-sweep garbage collector
- **`no_gc.rs`** - No-op garbage collector for testing
- **`stats.rs`** - GC performance statistics

### Compilation and Optimization
- **`src/compiler.rs`** - Assembly to bytecode compilation
- **`src/lisp_compiler.rs`** - Lisp to TinyTotVM transpilation
- **`src/optimizer.rs`** - Advanced 8-pass optimization engine

### Profiling and Debugging (`src/profiling/`)
- **`profiler.rs`** - Performance profiling and metrics
- **`stats.rs`** - Profiling statistics and reporting

### Testing Framework (`src/testing/`)
- **`harness.rs`** - Test execution framework
- **`runner.rs`** - Test runner and result reporting

### Intermediate Representation (`src/ir/`)
- **`mod.rs`** - Core IR data structures and register allocation
- **`lowering.rs`** - Stack-to-register translation pass
- **`vm.rs`** - Register-based execution engine

### Command Line Interface (`src/cli/`)
- **`args.rs`** - Command line argument parsing
- **`commands.rs`** - Command execution and dispatch

### Supporting Files
- **`std/`** - Standard library modules (math, string, I/O, network)
- **`docs/`** - Comprehensive documentation
- **`examples/`** - 93 test programs covering all features

## License

Free, as in free beer.

---
