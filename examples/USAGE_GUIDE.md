# TinyTotVM BEAM-Style Concurrency Usage Guide

## Current Implementation Status

Based on analysis of the TinyTotVM codebase, the following BEAM-style concurrency features are implemented:

### Fully Implemented
- **SPAWN**: Creates new processes from function names
- **SEND**: Sends messages to processes by PID
- **RECEIVE**: Receives messages from process mailbox
- **REGISTER**: Registers current process with a name
- **WHEREIS**: Finds process PID by registered name
- **SENDNAMED**: Sends messages to named processes
- **YIELD**: Cooperative multitasking yield

### Implementation Details

#### Process Spawning
The SPAWN opcode currently supports these predefined process types:
- `"hello_world"`: Simple greeting process
- `"counter"`: Counting process (prints 1, 2, 3)
- Any other string: Creates default process

#### Scheduler Requirements
Concurrency features require the SMP scheduler:
```bash
ttvm --smp your_program.ttvm
```

The regular VM only supports basic operations and will error on concurrency opcodes.

#### Process ID Assignment
- PID 1: Main process (first process)
- PID 2+: Spawned processes in order

## Working Examples

### Basic Process Spawning
```ttvm
; Main process
PUSH_STR "Starting main process"
PRINT

; Spawn hello_world process
PUSH_STR "hello_world"
SPAWN                    ; Returns PID 2
PRINT                    ; Print the PID

YIELD                    ; Let spawned process run

HALT
```

### Message Passing
```ttvm
; Main process
PUSH_STR "Main process starting"
PRINT

; Spawn a process to receive messages
PUSH_STR "hello_world"
SPAWN                    ; Returns PID 2
PRINT

YIELD                    ; Let process start

; Send a message
PUSH_STR "Hello from main!"
SEND 2                   ; Send to PID 2

YIELD                    ; Let receiver process

HALT
```

### Name Registry
```ttvm
; Register main process
REGISTER "main"
PUSH_STR "Registered as 'main'"
PRINT

; Look up process by name
WHEREIS "main"           ; Returns PID 1
PRINT

; Look up non-existent process
WHEREIS "missing"        ; Returns 0
PRINT

HALT
```

## Testing Your Programs

### Running Tests
TinyTotVM includes built-in tests for concurrency features:

```bash
# Test process spawning
./target/release/ttvm test-spawn-comprehensive

# Test message passing
./target/release/ttvm test-send-receive-comprehensive

# Test name registry
./target/release/ttvm test-register-whereis

# Test SMP scheduler
./target/release/ttvm test-smp-concurrency
```

### Example Execution
```bash
# Run with SMP scheduler (required for concurrency)
./target/release/ttvm --smp examples/working_example.ttvm

# Run with debug output
./target/release/ttvm --smp --debug examples/working_example.ttvm

# Run with process tracing
./target/release/ttvm --smp --trace-procs examples/working_example.ttvm
```

## OpCode Reference

### SPAWN
```
PUSH_STR "process_name"
SPAWN                    ; Creates process, returns PID
```

**Supported Process Names:**
- `"hello_world"` - Prints greeting and halts
- `"counter"` - Prints numbers 1, 2, 3 and halts  
- `"other"` - Creates default process

### SEND
```
PUSH_STR "message"       ; Or any value
SEND <pid>               ; Send to process ID
```

### RECEIVE
```
RECEIVE                  ; Wait for message, push to stack
```

### REGISTER
```
REGISTER "name"          ; Register current process
```

### WHEREIS
```
WHEREIS "name"           ; Find PID by name (0 if not found)
```

### SENDNAMED
```
PUSH_STR "message"
SENDNAMED "name"         ; Send to named process
```

### YIELD
```
YIELD                    ; Give up CPU time to other processes
```

## Best Practices

### 1. Always Use SMP Scheduler
Concurrency features only work with `--smp` flag.

### 2. Use YIELD Strategically
Call YIELD after:
- Spawning processes
- Sending messages
- Before expecting responses

### 3. Handle Process Synchronization
The system uses cooperative multitasking, so processes must yield control.

### 4. Test with Built-in Tests
Use the built-in test commands to verify concurrency is working.

## Known Limitations

### 1. Limited Process Types
Only `"hello_world"` and `"counter"` are predefined. Other names create default processes.

### 2. No Error Handling
The examples assume processes exist and messages are delivered successfully.

### 3. No Process Termination Signals
The current implementation doesn't include proper process lifecycle management.

### 4. Cooperative Scheduling
Processes must explicitly yield; no preemptive scheduling.

## Troubleshooting

### Program Hangs
- Ensure you're using `--smp` flag
- Add YIELD calls after spawning processes
- Check that spawned processes eventually halt

### "Unsupported operation" Errors
- Use `--smp` flag for concurrency features
- Verify you're using supported OpCodes

### Messages Not Received
- Add YIELD calls to allow message processing
- Verify process PIDs are correct
- Check that receiving processes are waiting

## Future Enhancements

The current implementation provides a solid foundation for BEAM-style concurrency. Potential improvements include:

1. **More Process Types**: Additional predefined process templates
2. **Error Handling**: Proper error propagation and handling
3. **Process Monitoring**: Supervisor trees and process monitoring
4. **Preemptive Scheduling**: Time-sliced scheduling options
5. **Message Patterns**: Pattern matching for message reception

## Contributing

When creating new examples or extending the system:

1. Test with the SMP scheduler
2. Use the built-in test commands
3. Follow cooperative multitasking patterns
4. Document any new process types or features

The TinyTotVM concurrency model closely follows BEAM principles while being adapted for a stack-based virtual machine architecture.