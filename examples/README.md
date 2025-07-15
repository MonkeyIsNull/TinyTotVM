# TinyTotVM BEAM-Style Concurrency Examples

This directory contains comprehensive examples demonstrating real BEAM-style concurrency patterns in TinyTotVM, including process spawning, inter-process communication, and name registry functionality.

## OpCode Reference

### Process Management

#### SPAWN
```
PUSH_STR "function_name"
SPAWN                    ; Creates new process, returns PID on stack
```

**Description**: Creates a new process based on the function name on the stack. Returns the new process ID (PID) on the stack.

**Supported Function Names**:
- `"hello_world"` - Creates a simple greeting process
- `"counter"` - Creates a counting process
- `"worker"` - Creates a worker process template
- `"task_manager"` - Creates a task coordination process
- `"collector"` - Creates a result collection process
- `"receiver"` - Creates a message receiving process
- `"sender"` - Creates a message sending process
- Any other string creates a default process

**Example**:
```
PUSH_STR "hello_world"
SPAWN                    ; New process PID now on stack
PRINT                    ; Print the PID
```

#### YIELD
```
YIELD                    ; Voluntarily give up CPU time
```

**Description**: Voluntarily yields control to the scheduler, allowing other processes to run. Essential for cooperative multitasking.

### Message Passing

#### SEND
```
PUSH_STR "message"       ; Or any other value
SEND <process_id>        ; Send to specific PID
```

**Description**: Sends the value on top of the stack to the specified process ID. The receiving process can retrieve it with RECEIVE.

**Example**:
```
PUSH_INT 42
SEND 2                   ; Send integer 42 to process 2

PUSH_STR "Hello"
SEND 3                   ; Send string "Hello" to process 3
```

#### RECEIVE
```
RECEIVE                  ; Wait for incoming message
```

**Description**: Blocks until a message is available in the process mailbox, then puts the message on the stack.

**Example**:
```
RECEIVE                  ; Wait for message
PUSH_STR "Got: "
PRINT
PRINT                    ; Print the received message
```

### Name Registry

#### REGISTER
```
REGISTER "process_name"  ; Register current process with name
```

**Description**: Registers the current process with a human-readable name. Other processes can then find this process using WHEREIS and send messages using SENDNAMED.

**Example**:
```
REGISTER "worker1"       ; Register current process as "worker1"
```

#### WHEREIS
```
WHEREIS "process_name"   ; Find PID by name
```

**Description**: Looks up a process by its registered name. Returns the PID on the stack, or 0 if the name is not found.

**Example**:
```
WHEREIS "worker1"        ; Find PID of "worker1"
PRINT                    ; Print the PID (or 0 if not found)
```

#### SENDNAMED
```
PUSH_STR "message"       ; Or any other value
SENDNAMED "process_name" ; Send to named process
```

**Description**: Sends the value on top of the stack to a process identified by its registered name. More convenient than using PIDs directly.

**Example**:
```
PUSH_STR "Hello worker!"
SENDNAMED "worker1"      ; Send message to process named "worker1"
```

## Process ID Assignment

TinyTotVM assigns Process IDs (PIDs) sequentially starting from 1:

- **PID 1**: Main process (the one that starts first)
- **PID 2**: First spawned process
- **PID 3**: Second spawned process
- **PID 4**: Third spawned process
- And so on...

## BEAM-Style Patterns

### 1. Actor Model
Each process is an independent actor with its own mailbox and state. Processes communicate only through message passing.

### 2. Supervisor Pattern
Use a main process to spawn and monitor worker processes, handling failures and coordinating system-wide operations.

### 3. Name Registry
Register important processes with meaningful names for easier communication and system organization.

### 4. Message Protocols
Establish conventions for message formats, such as:
- `"TASK:TYPE:DATA"` for work distribution
- `"STATUS_REQUEST"` for health checks
- `"SHUTDOWN"` for graceful termination

## Example Files

### 01_process_spawning.ttvm
**Purpose**: Demonstrates basic process spawning patterns
**Key Features**:
- Spawning different process types
- Understanding PID assignment
- Process lifecycle management

**Run with**:
```bash
ttvm examples/01_process_spawning.ttvm
```

### 02_message_passing.ttvm
**Purpose**: Shows inter-process communication patterns
**Key Features**:
- Sending different data types
- Bidirectional communication
- Message ordering and synchronization

**Run with**:
```bash
ttvm examples/02_message_passing.ttvm
```

### 03_name_registry.ttvm
**Purpose**: Demonstrates name-based process communication
**Key Features**:
- Process registration and lookup
- Named message passing
- Name lifecycle management

**Run with**:
```bash
ttvm examples/03_name_registry.ttvm
```

### 04_comprehensive_workflow.ttvm
**Purpose**: Complete distributed system simulation
**Key Features**:
- Multi-process coordination
- Task distribution patterns
- System monitoring and health checks
- Graceful shutdown procedures

**Run with**:
```bash
ttvm examples/04_comprehensive_workflow.ttvm
```

### coffee_shop_demo.ttvm
**Purpose**: Realistic actor model demonstration with multi-process coordination
**Key Features**:
- Three-actor workflow (Customer, Cashier, Barista)
- Structured message passing with objects
- Real-world business process modeling
- Sequential workflow with proper synchronization
- Comprehensive demonstration of BEAM-style patterns

**Run with**:
```bash
# Recommended: Run as integrated test (avoids hanging)
ttvm test-coffee-shop

# File execution may hang with current SMP scheduler
# ttvm examples/coffee_shop_demo.ttvm
```

## Best Practices

### 1. Process Design
- Keep processes focused on single responsibilities
- Use meaningful names for process registration
- Implement proper error handling and recovery

### 2. Message Passing
- Use YIELD strategically to allow other processes to run
- Implement message protocols for complex communications
- Handle message ordering dependencies carefully

### 3. System Architecture
- Use supervisor processes to manage worker pools
- Implement health check and monitoring systems
- Plan for graceful shutdown procedures

### 4. Debugging
- Use descriptive PRINT statements to trace execution
- Monitor process states and message flows
- Test with different process counts and message loads

## Performance Considerations

### Scheduler Behavior
- TinyTotVM uses cooperative multitasking
- Processes must call YIELD to allow others to run
- Long-running computations should include YIELD calls

### Message Handling
- Messages are queued in process mailboxes
- RECEIVE blocks until a message is available
- Large message volumes may require careful scheduling

### Memory Management
- Each process has its own stack and variables
- Process cleanup happens automatically on termination
- Name registry entries are cleaned up when processes exit

## Testing Your Examples

To run the examples with the TinyTotVM scheduler:

```bash
# SMP scheduler (enabled by default)
ttvm examples/01_process_spawning.ttvm

# SMP scheduler with debugging output
ttvm --debug examples/02_message_passing.ttvm

# SMP scheduler with process tracing
ttvm --trace-procs examples/03_name_registry.ttvm

# Single-threaded mode (if needed)
ttvm --no-smp examples/04_comprehensive_workflow.ttvm
```

## Advanced Topics

### Error Handling
While not covered in these basic examples, production systems should implement:
- Process monitoring and restart logic
- Error message protocols
- Supervisor tree patterns

### Scalability
For larger systems, consider:
- Process pooling strategies
- Load balancing across workers
- Dynamic process creation and cleanup

### Integration
TinyTotVM also supports:
- File I/O operations
- Network communication
- Database connectivity
- External process execution

These features can be combined with the concurrency primitives shown in these examples to build complete distributed applications.