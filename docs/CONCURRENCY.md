# BEAM-Style Concurrency in TinyTotVM

TinyTotVM implements BEAM-style (Erlang/Elixir-inspired) actor model concurrency with lightweight processes, message passing, and fault tolerance.

## Quick Start

**Enable SMP Scheduler** (Required for concurrency):
```bash
ttvm --smp your_program.ttvm
```

**Basic Example**:
```assembly
; hello_concurrent.ttvm
PUSH_STR "Process starting"
PRINT
YIELD                    ; Allow other processes to run
PUSH_STR "After yield"
PRINT
HALT
```

```bash
ttvm --smp hello_concurrent.ttvm
```

## Core Concurrency Opcodes

### SPAWN - Create New Process
Creates a new lightweight process.

```assembly
; Spawn a hello_world process
PUSH_STR "hello_world"
SPAWN
PRINT               ; Prints the new process ID

; Spawn a counter process  
PUSH_STR "counter"
SPAWN
PRINT               ; Prints the new process ID

; Spawn custom process (creates default behavior)
PUSH_STR "my_worker"
SPAWN
PRINT
```

### YIELD - Cooperative Scheduling
Allows other processes to run.

```assembly
PUSH_STR "Before yield"
PRINT
YIELD               ; Other processes can run now
PUSH_STR "After yield"
PRINT
```

### REGISTER & WHEREIS - Process Names
Register processes with names for easy communication.

```assembly
; Register current process
REGISTER "main_process"
PUSH_STR "Process registered as main_process"
PRINT

; Find process by name
WHEREIS "main_process"
PRINT               ; Prints the process ID (or 0 if not found)
```

### SEND & RECEIVE - Message Passing
Send messages between processes.

```assembly
; Send message to process ID 1
PUSH_STR "Hello, Process 1!"
SEND 1
PUSH_STR "Message sent"
PRINT

; Send message to named process
PUSH_STR "Hello, worker!"
SENDNAMED "worker_process"

; Receive message (blocks until message arrives)
RECEIVE
PRINT               ; Prints the received message
```

## Complete Examples

### Example 1: Basic Process Communication

**File: process_comm.ttvm**
```assembly
; Main process registers itself and spawns a worker
REGISTER "main"
PUSH_STR "Main process registered"
PRINT

; Spawn a worker process
PUSH_STR "hello_world"
SPAWN
PRINT               ; Print worker PID

; Send greeting to worker (if we knew its PID)
PUSH_STR "Hello from main!"
SEND 2              ; Assume worker got PID 2
PUSH_STR "Sent greeting to worker"
PRINT

YIELD               ; Let worker run
PUSH_STR "Main process continuing"
PRINT
HALT
```

### Example 2: Producer-Consumer Pattern

**File: producer_consumer.ttvm**
```assembly
; Producer process
REGISTER "producer"
PUSH_STR "Producer starting"
PRINT

; Produce some data
PUSH_STR "data_item_1"
SENDNAMED "consumer"
PUSH_STR "Sent data_item_1"
PRINT

PUSH_STR "data_item_2" 
SENDNAMED "consumer"
PUSH_STR "Sent data_item_2"
PRINT

YIELD               ; Let consumer process

; Consumer would be in another .ttvm file or spawned process
HALT
```

### Example 3: Worker Pool

**File: worker_pool.ttvm**
```assembly
; Spawn multiple worker processes
PUSH_STR "Creating worker pool"
PRINT

; Spawn worker 1
PUSH_STR "counter"
SPAWN
PRINT

; Spawn worker 2  
PUSH_STR "hello_world"
SPAWN  
PRINT

; Spawn worker 3
PUSH_STR "counter"
SPAWN
PRINT

PUSH_STR "Worker pool created"
PRINT

; Coordinate workers
YIELD
PUSH_STR "Main coordinator continuing"
PRINT

HALT
```

## Testing Your Concurrent Programs

```bash
# Test basic concurrency
ttvm test-concurrency

# Test specific features
ttvm test-spawn-comprehensive
ttvm test-send-receive-comprehensive
ttvm test-register-whereis

# Test SMP scheduler
ttvm test-smp-concurrency
```

## Process Types

When using SPAWN, these predefined process types are available:

- **"hello_world"**: Prints greeting and exits
- **"counter"**: Counts from 1 to 3 and exits  
- **Any other string**: Creates a basic process

## Message Types

Messages can contain:
- Strings (most common)
- Integers
- Other basic data types

```assembly
; Send different message types
PUSH_STR "text message"
SEND 1

PUSH_INT 42
SEND 1  

PUSH_BOOL true
SEND 1
```

## Best Practices

### 1. Always Use SMP Scheduler
```bash
# Correct - enables concurrency
ttvm --smp program.ttvm

# Wrong - concurrency opcodes will fail
ttvm program.ttvm
```

### 2. Use YIELD for Cooperation
```assembly
; Good - allows other processes to run
PUSH_STR "Working..."
PRINT
YIELD
PUSH_STR "More work..."
PRINT
```

### 3. Register Important Processes
```assembly
; Good - easy to find later
REGISTER "database"
REGISTER "logger"
REGISTER "main_controller"
```

### 4. Handle Message Flow
```assembly
; Send then yield to allow processing
PUSH_STR "urgent_task"
SENDNAMED "worker"
YIELD               ; Let worker process the task
```

## Compilation and Bytecode

Concurrency opcodes compile to bytecode:

```bash
# Compile concurrent program
ttvm compile worker.ttvm worker.ttb

# Run compiled version
ttvm --smp worker.ttb
```

**Bytecode mappings:**
- SPAWN → 0x80
- REGISTER → 0x81  
- WHEREIS → 0x82
- YIELD → 0x8B
- RECEIVE → 0x8C
- SEND → 0x8D

## Performance Tips

### SMP Benefits
- Automatic CPU core detection
- Work-stealing scheduler
- True parallelism for CPU-intensive tasks

### Scheduling
- Processes are preemptively scheduled
- Each process gets 1000 instruction "reductions" before yielding
- Use YIELD to be cooperative

## Limitations

### Current Implementation
- Process spawning limited to predefined types
- No distributed processes (single machine only)
- No hot code loading
- Advanced supervision requires Rust API

### Working Features
- All basic concurrency opcodes work in .ttvm files
- SMP scheduler with work-stealing  
- Process isolation and message passing
- Bytecode compilation support
- Preemptive scheduling

## Real-World Examples

### Web Server Simulation
```assembly
; server.ttvm
REGISTER "web_server"
PUSH_STR "Web server starting"
PRINT

; Spawn request handlers
PUSH_STR "hello_world"    ; Handler 1
SPAWN
PRINT

PUSH_STR "counter"        ; Handler 2  
SPAWN
PRINT

PUSH_STR "Request handlers spawned"
PRINT

; Main server loop
YIELD
PUSH_STR "Server processing requests"
PRINT

HALT
```

### Data Pipeline
```assembly
; pipeline.ttvm
REGISTER "pipeline_controller"

; Stage 1: Data ingestion
PUSH_STR "counter"        ; Simulates data source
SPAWN
PRINT

; Stage 2: Processing  
PUSH_STR "hello_world"    ; Simulates processor
SPAWN
PRINT

; Stage 3: Output
PUSH_STR "counter"        ; Simulates output writer
SPAWN  
PRINT

PUSH_STR "Pipeline stages started"
PRINT

; Coordinate pipeline
YIELD
PUSH_STR "Pipeline coordinator active"
PRINT

HALT
```

### Coffee Shop Actor Model Demo
A comprehensive example showing multi-actor coordination:

```assembly
; coffee_shop_demo.ttvm - Full actor model workflow
; Demonstrates Customer -> Cashier -> Barista communication

REGISTER "customer"
PUSH_STR "Customer registered"
PRINT

; Spawn cashier and barista
PUSH_STR "cashier_worker"
SPAWN
PRINT

PUSH_STR "barista_worker" 
SPAWN
PRINT

; Order workflow with structured messages
NEW_OBJECT
PUSH_STR "order_type"
PUSH_STR "order"
OBJECT_SET

PUSH_STR "drink"
PUSH_STR "Latte"
OBJECT_SET

SEND 2              ; Send to cashier
RECEIVE             ; Wait for confirmation
PRINT

; Continue workflow...
HALT
```

**Run the demo:**
```bash
# Run as integrated test (recommended - completes without hanging)
ttvm test-coffee-shop

# File execution may hang due to SMP scheduler behavior
# ttvm --smp examples/coffee_shop_demo.ttvm
```

**Note**: The test version creates three separate processes (Customer, Cashier, Barista) that coordinate through message passing, demonstrating real BEAM-style actor patterns. The file version is provided for reference but may hang due to SMP scheduler limitations with SPAWN.

## Getting Help

For implementation details, see the source code. For examples:

```bash
# See working examples
ls examples/*.ttvm | grep -E "(concurrency|spawn|send)"

# Run comprehensive tests
ttvm test-all
```

The BEAM-style concurrency in TinyTotVM provides lightweight processes, message passing, and fault isolation - the core building blocks for robust concurrent applications.