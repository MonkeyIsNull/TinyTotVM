# TinyTotVM Standard Library

The TinyTotVM Standard Library provides a comprehensive collection of utility functions and modules that make programming in TinyTotVM more productive and enjoyable.

## Architecture

The standard library is organized into focused modules:

- **`std/math.ttvm`** - Mathematical functions and constants
- **`std/string.ttvm`** - String manipulation utilities  
- **`std/list.ttvm`** - List/array utilities and higher-order functions
- **`std/convert.ttvm`** - Type conversion and type checking utilities
- **`std/io.ttvm`** - I/O operations and data format handling
- **`std/network.ttvm`** - Network operations and protocols
- **`std/prelude.ttvm`** - Commonly used functions (imports other modules)

## Usage

### Quick Start with Prelude

For most programs, import the prelude to get commonly used functions:

```assembly
IMPORT std/prelude.ttvm

; Now you have access to math, string, list, and conversion functions
LOAD PI
PRINT

PUSH_STR "Hello"
PUSH_STR " World"
LOAD strcat
CALL_FUNCTION
PRINT
```

### Individual Module Import

For specific functionality, import individual modules:

```assembly
IMPORT std/math.ttvm

PUSH_FLOAT 16.0
LOAD sqrt
CALL_FUNCTION
PRINT  ; Outputs: 4
```

## Module Reference

### Math Module (`std/math.ttvm`)

**Constants:**
- `PI` - Mathematical constant π (3.141592653589793)
- `E` - Mathematical constant e (2.718281828459045) 
- `SQRT2` - Square root of 2 (1.4142135623730951)

**Functions:**
- `abs(x)` - Absolute value
- `sqrt(x)` - Square root using Newton's method
- `pow(x, n)` - Power function (x^n for integer n)
- `max(a, b)` - Maximum of two numbers
- `min(a, b)` - Minimum of two numbers
- `factorial(n)` - Factorial function

**Example:**
```assembly
IMPORT std/math.ttvm

PUSH_FLOAT -42.5
LOAD abs
CALL_FUNCTION
PRINT  ; Outputs: 42.5

PUSH_FLOAT 25.0
LOAD sqrt  
CALL_FUNCTION
PRINT  ; Outputs: 5
```

### String Module (`std/string.ttvm`)

**Functions:**
- `strlen(str)` - Get string length (simplified implementation)
- `strcat(str1, str2)` - Concatenate two strings
- `repeat(str, count)` - Repeat string count times
- `upper(str)` - Convert to uppercase (demo implementation)
- `lower(str)` - Convert to lowercase (demo implementation)  
- `trim(str)` - Remove whitespace (demo implementation)
- `starts_with(str, prefix)` - Check if string starts with prefix
- `ends_with(str, suffix)` - Check if string ends with suffix
- `contains(str, substring)` - Check if string contains substring

**Example:**
```assembly
IMPORT std/string.ttvm

PUSH_STR "Hello"
PUSH_STR " World"
LOAD strcat
CALL_FUNCTION
PRINT  ; Outputs: Hello World

PUSH_STR "Hi"
PUSH_INT 3
LOAD repeat
CALL_FUNCTION  
PRINT  ; Outputs: HiHiHi
```

### List Module (`std/list.ttvm`)

**Basic Functions:**
- `length(list)` - Get list length
- `first(list)` - Get first element
- `last(list)` - Get last element  
- `is_empty(list)` - Check if list is empty
- `append(list, element)` - Add element to end
- `prepend(element, list)` - Add element to beginning
- `reverse(list)` - Reverse list order

**Higher-Order Functions:**
- `map(list, func)` - Apply function to each element
- `filter(list, predicate)` - Filter elements by predicate
- `reduce(list, func, initial)` - Reduce list to single value

**Aggregate Functions:**
- `sum(list)` - Sum all numbers in list
- `list_max(list)` - Find maximum element
- `list_min(list)` - Find minimum element

**Example:**
```assembly
IMPORT std/list.ttvm

PUSH_INT 1
PUSH_INT 2  
PUSH_INT 3
MAKE_LIST 3
STORE my_list

LOAD my_list
LOAD length
CALL_FUNCTION
PRINT  ; Outputs: 3

LOAD my_list
LOAD first
CALL_FUNCTION
PRINT  ; Outputs: 1
```

### Convert Module (`std/convert.ttvm`)

**Type Conversion:**
- `to_string(value)` - Convert any value to string representation
- `to_int(str)` - Convert string to integer  
- `to_float(value)` - Convert value to float
- `to_bool(value)` - Convert value to boolean

**Type Checking:**
- `is_int(value)` - Check if value is integer
- `is_float(value)` - Check if value is float
- `is_string(value)` - Check if value is string
- `is_bool(value)` - Check if value is boolean
- `is_null(value)` - Check if value is null
- `is_list(value)` - Check if value is list
- `is_object(value)` - Check if value is object
- `is_function(value)` - Check if value is function
- `type_of(value)` - Get type name as string

**Safe Conversion:**
- `safe_to_int(value, default)` - Convert with fallback
- `safe_to_float(value, default)` - Convert with fallback

**Example:**
```assembly
IMPORT std/convert.ttvm

PUSH_INT 42
LOAD to_string
CALL_FUNCTION
PRINT  ; Outputs: converted_to_string

PUSH_FLOAT 3.14
LOAD is_float
CALL_FUNCTION
PRINT  ; Outputs: false (simplified implementation)
```

### I/O Module (`std/io.ttvm`)

**File Operations:**
- `read_file(filename)` - Read file contents
- `write_file(filename, content)` - Write content to file
- `append_file(filename, content)` - Append content to file
- `file_exists(filename)` - Check if file exists
- `file_size(filename)` - Get file size in bytes
- `list_dir(dirname)` - List directory contents

**Printing:**
- `println(value)` - Print with newline
- `print_func_alias(value)` - Print without newline
- `format(template, value)` - Format string with value

**Data Formats:**
- `to_json(obj)` - Serialize object to JSON-like string
- `from_json(json_str)` - Parse JSON-like string to object
- `to_csv_row(list)` - Convert list to CSV row
- `from_csv(csv_str)` - Parse CSV string to list

**Logging:**
- `log_info(message)` - Log info message
- `log_error(message)` - Log error message  
- `log_debug(message)` - Log debug message

**Interactive I/O:**
- `read_line()` - Read line from stdin
- `read_char()` - Read single character from stdin
- `read_input()` - Read all input until EOF from stdin

**Environment & System:**
- `get_env(var_name)` - Get environment variable
- `set_env(var_name, value)` - Set environment variable
- `get_args()` - Get command line arguments
- `get_time()` - Get current Unix timestamp
- `sleep(millis)` - Sleep for specified milliseconds

### Network Module (`std/network.ttvm`)

**DNS Operations:**
- `dns_resolve(hostname)` - Resolve hostname to IP address

**HTTP Operations:**
- `http_get(url)` - Perform HTTP GET request
- `http_post(url, data)` - Perform HTTP POST request

**TCP Operations:**
- `tcp_connect(host, port)` - Connect to TCP server
- `tcp_listen(port)` - Listen on TCP port
- `tcp_send(connection, data)` - Send data over TCP connection
- `tcp_recv(connection, buffer_size)` - Receive data from TCP connection

**UDP Operations:**
- `udp_bind(port)` - Bind UDP socket to port
- `udp_send(socket, host, port, data)` - Send UDP packet
- `udp_recv(socket, buffer_size)` - Receive UDP packet

**URL Utilities:**
- `parse_url(url)` - Parse URL into components (simplified)
- `build_url(components)` - Build URL from components (simplified)

**Example:**
```assembly
IMPORT std/network.ttvm

; DNS resolution
PUSH_STR "google.com"
LOAD dns_resolve
CALL_FUNCTION
PRINT  ; Outputs: IP address

; HTTP GET request
PUSH_STR "https://api.github.com/users/octocat"
LOAD http_get
CALL_FUNCTION
PRINT  ; Outputs: HTTP response

; TCP connection
PUSH_STR "google.com"
PUSH_INT 80
LOAD tcp_connect
CALL_FUNCTION
STORE connection

LOAD connection
PUSH_STR "GET / HTTP/1.0\r\n\r\n"
LOAD tcp_send
CALL_FUNCTION
PRINT  ; Outputs: bytes sent
```

**Example:**
```assembly
IMPORT std/io.ttvm

PUSH_STR "Hello, logging!"
LOAD log_info
CALL_FUNCTION  ; Outputs: INFO: Hello, logging!

MAKE_OBJECT
PUSH_STR "test"
SET_FIELD name
LOAD to_json
CALL_FUNCTION
PRINT  ; Outputs: {serialized_object}
```

## Design Philosophy

The TinyTotVM Standard Library follows these principles:

1. **Modular Design** - Each module focuses on a specific domain
2. **Functional Style** - Pure functions without side effects where possible
3. **Consistency** - Similar naming and calling conventions across modules
4. **Extensibility** - Easy to add new functions and modules
5. **Cross-Module Integration** - Functions work well together

## Implementation Notes

The current standard library implementations are simplified for demonstration purposes. In a production environment, these would include:

- Full string manipulation with proper character handling
- Complete mathematical functions including trigonometry
- Robust type conversion with proper error handling
- Real JSON/CSV parsing and serialization
- File I/O with proper error handling
- Complete list operations with efficient algorithms

## Examples

See the `examples/` directory for comprehensive tests:

- `examples/stdlib_test.ttvm` - Math library demonstration
- `examples/stdlib_string_test.ttvm` - String utilities demonstration  
- `examples/stdlib_prelude_test.ttvm` - Prelude module demonstration
- `examples/stdlib_comprehensive_test.ttvm` - Complete standard library showcase

## Contributing

The standard library is designed to be easily extensible. To add new functionality:

1. Create new functions in appropriate modules
2. Export functions using the `EXPORT` instruction
3. Add tests demonstrating the new functionality
4. Update this documentation

The modular architecture ensures that additions don't break existing functionality and maintain the clean separation of concerns.