#!/bin/bash
cargo run --bin tiny_tot_vm -- compile-lisp examples/showcase.lisp examples/showcase.ttvm
cargo run --bin tiny_tot_vm -- compile examples/showcase.ttvm examples/showcase.ttb
cargo run --bin tiny_tot_vm -- examples/showcase.ttb
