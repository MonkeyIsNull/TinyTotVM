// bytecode.rs
use std::fs::File;
use std::io::{Read, BufReader};
use crate::OpCode;

pub fn load_bytecode(path: &str) -> std::io::Result<Vec<OpCode>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    let mut instructions = Vec::new();
    let mut ip = 0;

    while ip < buffer.len() {
        let opcode = buffer[ip];
        ip += 1;

        let op = match opcode {
            0x01 => {
                let val = i64::from_le_bytes(buffer[ip..ip+8].try_into().unwrap());
                ip += 8;
                OpCode::PushInt(val)
            }
            0x02 => {
                let len = buffer[ip] as usize;
                ip += 1;
                let s = String::from_utf8(buffer[ip..ip+len].to_vec()).unwrap();
                ip += len;
                OpCode::PushStr(s)
            }
            0x03 => OpCode::True,
            0x04 => OpCode::False,
            0x05 => OpCode::Null,
            0x06 => OpCode::Not,
            0x07 => OpCode::And,
            0x08 => OpCode::Or,
            0x09 => OpCode::Dup,
            0x10 => OpCode::Add,
            0x11 => OpCode::Sub,
            0x12 => OpCode::Concat,
            0x20 => OpCode::Eq,
            0x21 => OpCode::Gt,
            0x22 => OpCode::Lt,
            0x23 => OpCode::Ne,
            0x24 => OpCode::Ge,
            0x25 => OpCode::Le,
            0x30 => {
                let addr = u16::from_le_bytes(buffer[ip..ip+2].try_into().unwrap()) as usize;
                ip += 2;
                OpCode::Jmp(addr)
            }
            0x31 => {
                let addr = u16::from_le_bytes(buffer[ip..ip+2].try_into().unwrap()) as usize;
                ip += 2;
                OpCode::Jz(addr)
            }
            0x32 => {
                let addr = u16::from_le_bytes(buffer[ip..ip+2].try_into().unwrap()) as usize;
                ip += 2;
                OpCode::Call(addr)
            }
            0x33 => OpCode::Ret,
            0x40 => OpCode::Print,
            0x50 => {
                let len = buffer[ip] as usize;
                ip += 1;
                let s = String::from_utf8(buffer[ip..ip+len].to_vec()).unwrap();
                ip += len;
                OpCode::Store(s)
            }
            0x51 => {
                let len = buffer[ip] as usize;
                ip += 1;
                let s = String::from_utf8(buffer[ip..ip+len].to_vec()).unwrap();
                ip += len;
                OpCode::Load(s)
            }
            0x52 => {
                let len = buffer[ip] as usize;
                ip += 1;
                let s = String::from_utf8(buffer[ip..ip+len].to_vec()).unwrap();
                ip += len;
                OpCode::Delete(s)
            }
            0x60 => {
                let n = buffer[ip] as usize;
                ip += 1;
                OpCode::MakeList(n)
            }
            0x61 => OpCode::Len,
            0x62 => OpCode::Index,
            0x70 => OpCode::DumpScope,
            0xFF => OpCode::Halt,
            _ => panic!("Unknown bytecode: 0x{:02X}", opcode),
        };

        instructions.push(op);
    }

    Ok(instructions)
}