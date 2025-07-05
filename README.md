# TinyTotVM
![TinyTotVM Logo](tiny_tot_vm_logo.png)

> „Ohne Kaffee keine Erleuchtung – auch nicht für Maschinen.“

**TinyTotVM** is a tiny, stack-based virtual machine written in Rust.
This repo is, in essence, a toy-box for my experiments in writing a VM. It's not expected to be used for production usage in anything. That said, if you want to play around with it, have at it. At some point I'll update my long-term goals for and post them someplace here, at least so I remember them.


## Demo
Take a look at the instructions in examples/showcase.ttvm.
That should have the latest supported opcodes.

Go ahead and make sure it compiles to bytecode.
```
cargo run -- compile examples/showcase.ttvm sample_ttbs/showcase.ttb
```

Then go ahead and run it:
```
cargo run -- sample_ttbs/showcase.ttb
```

You should see something like:
```
Str("=== Core Ops ===")
Int(5)
Str("Hello, World!")
Str("=== Booleans & Null ===")
Bool(false)
Bool(false)
Null
Str("=== Comparisons ===")
Int(1)
Int(1)
Int(1)
Str("=== Variables ===")
Int(42)
Current scope: Some({})
Str("=== Control Flow ===")
Str("In ELSE block")
Str("Inside CALL")
Int(0)
Str("=== Lists ===")
Int(3)
Int(20)
```

## Lisp InterOp

Take a look at the example lisp code in examples/showcase.lisp

Compile the lisp code to ttvm code
```
cargo run --bin tiny_tot_vm -- compile-lisp examples/showcase.lisp examples/showcase_lisp.ttvm
```

Then run the ttvm code
```
cargo run -- examples/showcase_lisp.ttvm
```
And you should get something along the lines of:
```
Int(30)
Int(1)
Int(1)
Str("Not equal")
Str("Now equal!")
```

To compile it to bytecode just:
```
 cargo run -- compile examples/showcase_lisp.ttvm sample_ttbs/showcase_lisp.ttb
 ```

 and then run the vm on the bytecode:
 ```
 cargo run -- sample_ttbs/showcase_lisp.ttb
 ```

 Again, you should get something like:
 ```
 Int(30)
Int(1)
Int(1)
Str("Not equal")
Str("Now equal!")
```

LICENSE: Free, as in free beer.
