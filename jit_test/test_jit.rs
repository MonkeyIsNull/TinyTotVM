extern crate libc;

use std::io;
use std::ptr;
use std::mem;
use std::slice;

use libc::{
    mmap, mprotect, MAP_ANON, MAP_JIT, MAP_PRIVATE, PROT_READ, PROT_WRITE, PROT_EXEC,
};

fn main() -> io::Result<()> {
    const PAGE_SIZE: usize = 4096;

    unsafe {
        // Step 1: Allocate RW memory with MAP_JIT
        let addr = mmap(
            ptr::null_mut(),
            PAGE_SIZE,
            PROT_READ | PROT_WRITE,
            MAP_ANON | MAP_PRIVATE | MAP_JIT,
            -1,
            0,
        );

        if addr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }

        // Step 2: Write simple machine code: mov x0, #42; ret
        // AArch64 encoding:
        // mov x0, #42  => 0xd2800540
        // ret          => 0xd65f03c0
        let code: [u32; 2] = [
            0xd2800540, // mov x0, #42
            0xd65f03c0, // ret
        ];

        let code_bytes = slice::from_raw_parts_mut(addr as *mut u8, PAGE_SIZE);
        for (i, word) in code.iter().enumerate() {
            let bytes = word.to_le_bytes();
            code_bytes[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
        }

        // Step 3: Change memory protection to RX
        if mprotect(addr, PAGE_SIZE, PROT_READ | PROT_EXEC) != 0 {
            return Err(io::Error::last_os_error());
        }

        // Step 4: Cast to a function pointer and call
        let func: extern "C" fn() -> u64 = mem::transmute(addr);
        let result = func();

        println!("JIT result: {}", result); // Should print: 42
    }

    Ok(())
}

