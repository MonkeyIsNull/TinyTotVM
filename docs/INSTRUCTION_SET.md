# Instruction Set Reference

TinyTotVM provides **55+ core instructions** plus **comprehensive standard library** covering all aspects of modern programming language execution.

## Arithmetic & Logic

### Integer Arithmetic
```
ADD, SUB, MUL, DIV       ; Basic integer arithmetic
MOD                      ; Modulo operation
```

### Float Arithmetic
```
ADD_F, SUB_F, MUL_F, DIV_F  ; Float arithmetic with IEEE 754 compliance
```

### Comparisons
```
EQ, NE, GT, LT, GE, LE      ; Integer comparisons
EQ_F, NE_F, GT_F, LT_F, GE_F, LE_F  ; Float comparisons
```

### Boolean Operations
```
AND, OR, NOT             ; Boolean logic operations
```

## Stack Operations

```
PUSH_INT 42              ; Push integer literal
PUSH_FLOAT 3.14          ; Push float literal
PUSH_STR "hello"         ; Push string literal
TRUE, FALSE, NULL        ; Push boolean/null constants
DUP                      ; Duplicate top stack value
```

## Variables & Scoping

```
STORE varname            ; Store value in current scope
LOAD varname             ; Load variable value
DELETE varname           ; Remove variable from scope
DUMP_SCOPE              ; Debug: print current scope
```

## Objects & Collections

### Object Operations
```
MAKE_OBJECT             ; Create empty object
SET_FIELD name          ; Set object field
GET_FIELD name          ; Get object field
HAS_FIELD name          ; Check if field exists
DELETE_FIELD name       ; Remove object field
KEYS                   ; Get all field names as list
```

### List Operations
```
MAKE_LIST 3            ; Create list from top 3 stack items
LEN                    ; Get length of list/object
INDEX                  ; Access list element by index
```

## Functions & Control Flow

### Function Calls
```
CALL label param1 param2    ; Call function with parameters
RET                         ; Return from function
MAKE_FUNCTION label x y     ; Create function pointer
CALL_FUNCTION              ; Call function from stack
```

### Closures & Lambdas
```
MAKE_LAMBDA label x y       ; Create anonymous function (closure)
CAPTURE varname             ; Capture variable for closure
```

### Control Flow
```
JMP label              ; Unconditional jump
JZ label               ; Jump if zero/false/null
LABEL name             ; Define a label
```

## Exception Handling

```
TRY                    ; Start exception handling block
CATCH varname          ; Catch exceptions in variable
THROW                  ; Throw exception from stack
END_TRY               ; End exception handling block
```

## Module System

```
IMPORT path            ; Import module by file path
EXPORT name            ; Export variable/function by name
```

## I/O & System Operations

### Basic I/O
```
PRINT                  ; Print top stack value
READ_LINE              ; Read line from stdin
READ_CHAR              ; Read single character from stdin
READ_INPUT             ; Read all input until EOF from stdin
```

### File Operations
```
READ_FILE              ; Read file contents to string
WRITE_FILE             ; Write string to file
APPEND_FILE            ; Append content to file
FILE_EXISTS            ; Check if file exists
FILE_SIZE              ; Get file size in bytes
DELETE_FILE            ; Delete a file
LIST_DIR               ; List directory contents
READ_BYTES             ; Read file as byte array
WRITE_BYTES            ; Write byte array to file
```

### Environment & Process
```
GET_ENV                ; Get environment variable
SET_ENV                ; Set environment variable
GET_ARGS               ; Get command line arguments
EXEC                   ; Execute external command
EXEC_CAPTURE           ; Execute command and capture output
EXIT                   ; Exit with status code
```

### Time Operations
```
GET_TIME               ; Get current Unix timestamp
SLEEP                  ; Sleep for specified milliseconds
FORMAT_TIME            ; Format timestamp to string
```

### Network Operations
```
HTTP_GET               ; HTTP GET request
HTTP_POST              ; HTTP POST request
TCP_CONNECT            ; Connect to TCP server
TCP_LISTEN             ; Listen on TCP port
TCP_SEND               ; Send data over TCP connection
TCP_RECV               ; Receive data from TCP connection
UDP_BIND               ; Bind UDP socket to port
UDP_SEND               ; Send UDP packet
UDP_RECV               ; Receive UDP packet
DNS_RESOLVE            ; Resolve hostname to IP address
```

### Async Operations
```
ASYNC_READ             ; Asynchronous file read
ASYNC_WRITE            ; Asynchronous file write
AWAIT                  ; Wait for async operation completion
```

### Stream Operations
```
STREAM_CREATE          ; Create data stream
STREAM_READ            ; Read from data stream
STREAM_WRITE           ; Write to data stream
STREAM_CLOSE           ; Close data stream
```

### Data Format Operations
```
JSON_PARSE             ; Parse JSON string to object
JSON_STRINGIFY         ; Convert value to JSON string
CSV_PARSE              ; Parse CSV data to list
CSV_WRITE              ; Convert list to CSV format
```

### Compression & Crypto
```
COMPRESS               ; Compress data
DECOMPRESS             ; Decompress data
ENCRYPT                ; Encrypt data with key
DECRYPT                ; Decrypt data with key
HASH                   ; Generate hash of data
```

### Database Operations
```
DB_CONNECT             ; Connect to database
DB_QUERY               ; Execute database query
DB_EXEC                ; Execute database command
```

### Control
```
HALT                   ; Stop execution
```

## Type System

TinyTotVM supports 14 built-in value types:

- **Int(i64)** - 64-bit signed integers
- **Float(f64)** - IEEE 754 double-precision floats
- **Str(String)** - UTF-8 strings
- **Bool(bool)** - Boolean values
- **Null** - Null/undefined value
- **List(Vec<Value>)** - Dynamic arrays
- **Object(HashMap<String, Value>)** - Dynamic objects
- **Bytes(Vec<u8>)** - Byte arrays
- **Connection(String)** - Network connections
- **Stream(String)** - Data streams
- **Future(String)** - Async operations
- **Function** - Function pointers
- **Closure** - Closures with captured environment
- **Exception** - Exception objects with stack traces

## Error Handling

All operations use comprehensive error handling instead of crashes:

- `StackUnderflow` - Not enough values on stack
- `TypeMismatch` - Incompatible types for operation
- `UndefinedVariable` - Variable not found in scope
- `IndexOutOfBounds` - List/string index out of range
- `FileError` - File operation failure
- `ParseError` - Syntax or parsing error
- `CallStackUnderflow` - Return without call
- `NoVariableScope` - No variable scope available

## Addressing Modes

Instructions support both numeric and symbolic addressing:

```assembly
; Numeric addressing
JMP 42
CALL 100 param1 param2

; Symbolic addressing  
JMP end_loop
CALL my_function x y

LABEL end_loop
LABEL my_function
```