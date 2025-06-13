// bytecode.rs
use crate::OpCode;
use std::fs::File;
use std::io::{BufReader, Read};

pub fn load_bytecode(path: &str) -> std::io::Result<Vec<OpCode>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    let mut instructions = Vec::new();
    let mut ip = 0;

    while ip < buffer.len() {
        let opcode = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap());
        ip += 2;

        let op = match opcode {
            0x0001 => {
                let val = i64::from_le_bytes(buffer[ip..ip + 8].try_into().unwrap());
                ip += 8;
                OpCode::PushInt(val)
            }
            0x0002 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::PushStr(s)
            }
            0x0003 => OpCode::True,
            0x0004 => OpCode::False,
            0x0005 => OpCode::Null,
            0x0006 => OpCode::Not,
            0x0007 => OpCode::And,
            0x0008 => OpCode::Or,
            0x0009 => OpCode::Dup,

            0x0010 => OpCode::Add,
            0x0011 => OpCode::Sub,
            0x0012 => OpCode::Concat,

            0x0020 => OpCode::Eq,
            0x0021 => OpCode::Gt,
            0x0022 => OpCode::Lt,
            0x0023 => OpCode::Ne,
            0x0024 => OpCode::Ge,
            0x0025 => OpCode::Le,

            0x0030 => {
                let addr = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                OpCode::Jmp(addr)
            }
            0x0031 => {
                let addr = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                OpCode::Jz(addr)
            }
            0x0032 => {
                let addr = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                OpCode::Call(addr)
            }
            0x0033 => OpCode::Ret,

            0x0040 => OpCode::Print,

            0x0050 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::Store(s)
            }
            0x0051 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::Load(s)
            }
            0x0052 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::Delete(s)
            }

            0x0060 => {
                let n = buffer[ip] as usize;
                ip += 1;
                OpCode::MakeList(n)
            }
            0x0061 => OpCode::Len,
            0x0062 => OpCode::Index,

            0x0070 => OpCode::DumpScope,
            0x0072 => OpCode::ReadFile,
            0x0073 => OpCode::WriteFile,

            0x00FF => OpCode::Halt,

            _ => panic!("Unknown bytecode: 0x{:04X}", opcode),
        };

        instructions.push(op);
    }

    Ok(instructions)
}
