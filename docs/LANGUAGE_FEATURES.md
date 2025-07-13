# Language Features Guide

TinyTotVM provides comprehensive language features supporting functional programming, object-oriented programming, and modern runtime capabilities.

## Functions and Parameters

### Basic Functions
```assembly
; Define a function
LABEL add_numbers
LOAD a
LOAD b
ADD
RET

; Call with parameters
PUSH_INT 10
PUSH_INT 5
CALL add_numbers a b
PRINT              ; Outputs: 15
```

### Function Pointers
```assembly
; Create function pointer
MAKE_FUNCTION add_numbers x y
STORE adder

; Dynamic function call
PUSH_INT 3
PUSH_INT 7
LOAD adder
CALL_FUNCTION      ; Outputs: 10
```

## Closures and Variable Capture

### Basic Closures
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
CALL_FUNCTION      ; Outputs: 15 (5 + captured 10)
```

### Advanced Closure Patterns
```assembly
; Closure factory
LABEL make_multiplier
LOAD factor
STORE captured_factor
CAPTURE captured_factor
MAKE_LAMBDA multiply_lambda x
RET

LABEL multiply_lambda
LOAD x
LOAD captured_factor
MUL
RET

; Create specific multipliers
PUSH_INT 3
CALL make_multiplier factor
STORE triple

PUSH_INT 5
LOAD triple
CALL_FUNCTION      ; Outputs: 15
```

## Object-Oriented Programming

### Object Creation and Manipulation
```assembly
; Create person object
MAKE_OBJECT
PUSH_STR "Alice"
SET_FIELD name
PUSH_INT 30
SET_FIELD age

; Create nested address object
MAKE_OBJECT
PUSH_STR "123 Main St"
SET_FIELD street
PUSH_STR "Boston"
SET_FIELD city
SET_FIELD address

; Access nested data
DUP
GET_FIELD address
GET_FIELD city
PRINT              ; Outputs: "Boston"
```

### Object Introspection
```assembly
MAKE_OBJECT
PUSH_STR "value1"
SET_FIELD key1
PUSH_INT 42
SET_FIELD key2

; Get all keys
DUP
KEYS
PRINT              ; Outputs: ["key1", "key2"]

; Check field existence
PUSH_STR "key1"
HAS_FIELD
PRINT              ; Outputs: 1 (true)

; Get object size
LEN
PRINT              ; Outputs: 2
```

## Module System

### Module Export
```assembly
; math_module.ttvm
LABEL add_func
LOAD x
LOAD y
ADD
RET

LABEL multiply_func
LOAD x
LOAD y
MUL
RET

; Export functions
MAKE_FUNCTION add_func x y
STORE add
EXPORT add

MAKE_FUNCTION multiply_func x y
STORE multiply
EXPORT multiply
```

### Module Import and Usage
```assembly
; main.ttvm
IMPORT examples/math_module.ttvm

; Use imported functions
PUSH_INT 5
PUSH_INT 3
LOAD add
CALL_FUNCTION
PRINT              ; Outputs: 8

PUSH_INT 4
PUSH_INT 6
LOAD multiply
CALL_FUNCTION
PRINT              ; Outputs: 24
```

### Cross-Module Closures
```assembly
; closure_module.ttvm
LABEL make_adder
LOAD increment
STORE captured_increment
CAPTURE captured_increment
MAKE_LAMBDA adder_lambda x
RET

LABEL adder_lambda
LOAD x
LOAD captured_increment
ADD
RET

MAKE_FUNCTION make_adder increment
STORE create_adder
EXPORT create_adder

; main.ttvm
IMPORT closure_module.ttvm

PUSH_INT 10
LOAD create_adder
CALL_FUNCTION
STORE my_adder

PUSH_INT 5
LOAD my_adder
CALL_FUNCTION      ; Outputs: 15
```

## Exception Handling

### Basic Exception Handling
```assembly
TRY
    PUSH_INT 10
    PUSH_INT 0
    DIV                ; Division by zero
CATCH error
    PUSH_STR "Division by zero: "
    LOAD error
    CONCAT
    PRINT
END_TRY
```

### Nested Exception Handling
```assembly
TRY
    PUSH_STR "Outer try block"
    PRINT
    
    TRY
        PUSH_STR "Inner try block"
        PRINT
        PUSH_STR "Inner exception"
        THROW
    CATCH inner_error
        PUSH_STR "Caught in inner: "
        LOAD inner_error
        CONCAT
        PRINT
        PUSH_STR "Re-throwing from inner"
        THROW
    END_TRY
    
CATCH outer_error
    PUSH_STR "Caught in outer: "
    LOAD outer_error
    CONCAT
    PRINT
END_TRY
```

### Exception with Stack Unwinding
```assembly
LABEL risky_function
    TRY
        PUSH_STR "About to fail"
        PRINT
        PUSH_STR "Function failed"
        THROW
    CATCH func_error
        PUSH_STR "Handled in function: "
        LOAD func_error
        CONCAT
        PRINT
        PUSH_STR "Re-throwing"
        THROW
    END_TRY
RET

; Main execution
TRY
    CALL risky_function
CATCH main_error
    PUSH_STR "Caught in main: "
    LOAD main_error
    CONCAT
    PRINT
END_TRY
```

## Advanced Data Structures

### Dynamic Lists
```assembly
; Create heterogeneous list
PUSH_INT 1
PUSH_STR "hello"
PUSH_FLOAT 3.14
TRUE
MAKE_LIST 4

; Access elements
DUP
PUSH_INT 0
INDEX
PRINT              ; Outputs: 1

DUP
PUSH_INT 1
INDEX
PRINT              ; Outputs: "hello"
```

### Complex Data Combinations
```assembly
; List of objects
MAKE_OBJECT
PUSH_STR "Alice"
SET_FIELD name
PUSH_INT 25
SET_FIELD age

MAKE_OBJECT
PUSH_STR "Bob"
SET_FIELD name
PUSH_INT 30
SET_FIELD age

MAKE_LIST 2
STORE people

; Access nested data
LOAD people
PUSH_INT 0
INDEX
GET_FIELD name
PRINT              ; Outputs: "Alice"
```

## Higher-Order Programming

### Functions as Parameters
```assembly
LABEL apply_twice
LOAD func
LOAD value
LOAD func
CALL_FUNCTION
LOAD func
CALL_FUNCTION
RET

LABEL double
LOAD x
PUSH_INT 2
MUL
RET

; Use higher-order function
MAKE_FUNCTION double x
PUSH_INT 3
CALL apply_twice func value
PRINT              ; Outputs: 12 (3 * 2 * 2)
```

### Map-like Operations
```assembly
LABEL square
LOAD x
LOAD x
MUL
RET

LABEL map_function
LOAD list
LOAD func
; Implementation would iterate and apply function
; Simplified for example
RET

; Create list and function
PUSH_INT 1
PUSH_INT 2
PUSH_INT 3
MAKE_LIST 3

MAKE_FUNCTION square x
CALL map_function list func
```

## Type Coercion and Safety

### Automatic Type Coercion
```assembly
; Integer to float coercion
PUSH_INT 5
PUSH_FLOAT 2.5
ADD_F              ; 5 is automatically converted to 5.0
PRINT              ; Outputs: 7.5
```

### Type Checking
```assembly
; Safe type operations with error handling
TRY
    PUSH_STR "not a number"
    PUSH_INT 5
    ADD            ; Type mismatch will throw exception
CATCH type_error
    PUSH_STR "Type error: "
    LOAD type_error
    CONCAT
    PRINT
END_TRY
```

## Memory Management

### Automatic Scoping
```assembly
LABEL function_with_locals
    ; Create local scope
    PUSH_STR "local_value"
    STORE local_var
    
    LOAD local_var
    PRINT
    
    ; Local variables automatically cleaned up on return
RET

CALL function_with_locals
; local_var is no longer accessible here
```

### Variable Lifetime
```assembly
; Global scope
PUSH_INT 42
STORE global_var

LABEL test_scoping
    ; Function scope
    PUSH_STR "function_local"
    STORE local_var
    
    ; Can access global
    LOAD global_var
    PRINT
    
    ; Can access local
    LOAD local_var
    PRINT
RET

CALL test_scoping
; Can still access global
LOAD global_var
PRINT
```

## Performance Considerations

### Efficient Function Calls
- Pre-allocated call stack (64 frames)
- Efficient parameter passing
- Tail call optimization available

### Memory Efficiency
- Pre-allocated main stack (1024 items)
- Automatic memory cleanup
- Optional garbage collection

### Optimization Features
- Constant folding
- Dead code elimination
- Jump threading
- Tail call optimization