# Standard Library Documentation

TinyTotVM includes a comprehensive standard library with over 100 utility functions organized into focused modules.

## Quick Start

```assembly
; Import the prelude for commonly used functions
IMPORT std/prelude.ttvm

; Use math functions
PUSH_FLOAT 25.0
LOAD sqrt
CALL_FUNCTION      ; Outputs: 5.0

; Use string functions
PUSH_STR "Hello"
PUSH_STR " World"
LOAD strcat
CALL_FUNCTION      ; Outputs: "Hello World"
```

## Math Library (std/math.ttvm)

### Constants
- **PI** - Mathematical constant π (3.141592653589793)
- **E** - Mathematical constant e (2.718281828459045)

### Functions
- **abs(x)** - Absolute value
- **sqrt(x)** - Square root
- **pow(x, y)** - x raised to power y
- **max(x, y)** - Maximum of two values
- **min(x, y)** - Minimum of two values
- **factorial(n)** - Factorial calculation

### Example Usage
```assembly
IMPORT std/math.ttvm

; Constants
LOAD PI
PRINT              ; Outputs: 3.141592653589793

; Functions
PUSH_FLOAT -42.5
LOAD abs
CALL_FUNCTION
PRINT              ; Outputs: 42.5

PUSH_FLOAT 16.0
LOAD sqrt
CALL_FUNCTION
PRINT              ; Outputs: 4.0

PUSH_INT 5
LOAD factorial
CALL_FUNCTION
PRINT              ; Outputs: 120
```

## String Library (std/strings.ttvm)

### Functions
- **strlen(str)** - String length
- **strcat(str1, str2)** - String concatenation
- **repeat(str, n)** - Repeat string n times
- **upper(str)** - Convert to uppercase
- **lower(str)** - Convert to lowercase
- **trim(str)** - Remove leading/trailing whitespace

### Example Usage
```assembly
IMPORT std/strings.ttvm

PUSH_STR "Hello World"
LOAD strlen
CALL_FUNCTION
PRINT              ; Outputs: 11

PUSH_STR "Hi"
PUSH_INT 3
LOAD repeat
CALL_FUNCTION
PRINT              ; Outputs: "HiHiHi"

PUSH_STR "hello"
LOAD upper
CALL_FUNCTION
PRINT              ; Outputs: "HELLO"
```

## List Library (std/lists.ttvm)

### Functions
- **length(list)** - Get list length
- **first(list)** - Get first element
- **last(list)** - Get last element
- **sum(list)** - Sum numeric elements
- **append(list, item)** - Add item to list
- **reverse(list)** - Reverse list order

### Example Usage
```assembly
IMPORT std/lists.ttvm

PUSH_INT 1
PUSH_INT 2
PUSH_INT 3
MAKE_LIST 3
STORE mylist

LOAD mylist
LOAD length
CALL_FUNCTION
PRINT              ; Outputs: 3

LOAD mylist
LOAD sum
CALL_FUNCTION
PRINT              ; Outputs: 6
```

## I/O Library (std/io.ttvm)

### Functions
- **log_info(message)** - Log informational message
- **log_error(message)** - Log error message
- **read_file_safe(filename)** - Safe file reading with error handling
- **write_file_safe(filename, content)** - Safe file writing
- **file_size_safe(filename)** - Get file size safely

### Example Usage
```assembly
IMPORT std/io.ttvm

PUSH_STR "Application started"
LOAD log_info
CALL_FUNCTION      ; Outputs: "INFO: Application started"

PUSH_STR "test.txt"
LOAD read_file_safe
CALL_FUNCTION      ; Returns file content or error message
```

## Type Conversion Library (std/convert.ttvm)

### Functions
- **to_string(value)** - Convert value to string
- **to_int(value)** - Convert to integer
- **to_float(value)** - Convert to float
- **to_bool(value)** - Convert to boolean

### Example Usage
```assembly
IMPORT std/convert.ttvm

PUSH_INT 42
LOAD to_string
CALL_FUNCTION
PRINT              ; Outputs: "42"

PUSH_STR "123"
LOAD to_int
CALL_FUNCTION
PRINT              ; Outputs: 123
```

## Prelude Module (std/prelude.ttvm)

The prelude imports commonly used functions from all standard library modules for convenience.

### Included Functions
From **math**: sqrt, abs, max, min, PI  
From **strings**: strcat, strlen, upper, lower  
From **lists**: length, sum, first, last  
From **io**: log_info, log_error  
From **convert**: to_string, to_int, to_float

### Example Usage
```assembly
; Single import gives access to all common functions
IMPORT std/prelude.ttvm

; Use any prelude function directly
PUSH_FLOAT 25.0
LOAD sqrt
CALL_FUNCTION

PUSH_STR "Hello"
PUSH_STR " World"
LOAD strcat
CALL_FUNCTION

PUSH_INT 42
LOAD to_string
CALL_FUNCTION
```

## Testing Standard Library

### Individual Module Tests
```bash
ttvm examples/stdlib_test.ttvm           # Math library
ttvm examples/stdlib_string_test.ttvm    # String utilities
ttvm examples/stdlib_prelude_test.ttvm   # Prelude module
```

### Comprehensive Test
```bash
ttvm examples/stdlib_comprehensive_test.ttvm
```

This test demonstrates all standard library modules working together in a complete application.

## Creating Custom Library Modules

### Module Structure
```assembly
; my_module.ttvm

; Define functions
LABEL my_function
LOAD param
; ... function implementation
RET

; Create function pointer and export
MAKE_FUNCTION my_function param
STORE my_function
EXPORT my_function
```

### Using Custom Modules
```assembly
; main.ttvm
IMPORT my_module.ttvm

PUSH_STR "test"
LOAD my_function
CALL_FUNCTION
```

## Standard Library Architecture

The standard library is designed with these principles:

- **Modular Organization** - Focused, single-purpose modules
- **Consistent Interfaces** - Predictable function signatures
- **Error Handling** - Safe operations with proper error handling
- **Performance** - Efficient implementations using VM primitives
- **Composability** - Functions work well together
- **Extensibility** - Easy to add new modules and functions

## Advanced Standard Library Features

### Enhanced I/O (std/io_enhanced.ttvm)
- Environment variable operations
- Time and date functions
- Process execution utilities
- Advanced file operations

### Network Library (std/network.ttvm)
- HTTP client functions
- TCP/UDP socket operations
- DNS resolution utilities
- URL parsing and manipulation

### Advanced Features (std/advanced.ttvm)
- Async operation utilities
- Data streaming functions
- Compression and encryption
- Database operations
- JSON/CSV processing

These advanced modules provide comprehensive functionality for complex applications while maintaining the simplicity and safety of the core VM.