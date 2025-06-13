// compiler.rs
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum ByteCode {
    PushInt = 0x01,
    PushStr = 0x02,
    True = 0x03,
    False = 0x04,
    Null = 0x05,
    Not = 0x06,
    And = 0x07,
    Or = 0x08,
    Dup = 0x09,

    Add = 0x10,
    Sub = 0x11,
    Concat = 0x12,

    Eq = 0x20,
    Ne = 0x23,
    Gt = 0x21,
    Lt = 0x22,
    Ge = 0x24,
    Le = 0x25,

    Jmp = 0x30,
    Jz = 0x31,
    Call = 0x32,
    Ret = 0x33,

    Print = 0x40,
    Halt = 0xFF,

    Store = 0x50,
    Load = 0x51,
    Delete = 0x52,

    MakeList = 0x60,
    Len = 0x61,
    Index = 0x62,

    DumpScope = 0x70,

    ReadFile = 0x72,
    WriteFile = 0x73,
}

pub fn compile<P: AsRef<Path>>(input_path: P, output_path: P) -> std::io::Result<()> {
    let file = File::open(&input_path)?;
    let reader = BufReader::new(file);

    let mut output = File::create(&output_path)?;
    let mut lines: Vec<String> = Vec::new();
    let mut labels: HashMap<String, usize> = HashMap::new();

    // First pass: collect lines and label addresses
    for line in reader.lines() {
        let line = line?.trim().to_string();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if let Some(label) = line.strip_prefix("LABEL ") {
            labels.insert(label.trim().to_string(), lines.len());
        } else {
            lines.push(line);
        }
    }

    // Second pass: encode instructions
    for line in lines.iter() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let op = parts[0];
        let arg = parts.get(1).map(|s| s.trim());

        match op {
            "PUSH_INT" => {
                output.write_all(&[ByteCode::PushInt as u8])?;
                let n: i64 = arg.unwrap().parse().expect("Invalid integer");
                output.write_all(&n.to_le_bytes())?;
            }
            "PUSH_STR" => {
                output.write_all(&[ByteCode::PushStr as u8])?;
                let s = arg.unwrap().trim_matches('"');
                let bytes = s.as_bytes();
                output.write_all(&[bytes.len() as u8])?;
                output.write_all(bytes)?;
            }
            "TRUE" => output.write_all(&[ByteCode::True as u8])?,
            "FALSE" => output.write_all(&[ByteCode::False as u8])?,
            "NULL" => output.write_all(&[ByteCode::Null as u8])?,
            "ADD" => output.write_all(&[ByteCode::Add as u8])?,
            "SUB" => output.write_all(&[ByteCode::Sub as u8])?,
            "CONCAT" => output.write_all(&[ByteCode::Concat as u8])?,
            "EQ" => output.write_all(&[ByteCode::Eq as u8])?,
            "NE" => output.write_all(&[ByteCode::Ne as u8])?,
            "GT" => output.write_all(&[ByteCode::Gt as u8])?,
            "LT" => output.write_all(&[ByteCode::Lt as u8])?,
            "GE" => output.write_all(&[ByteCode::Ge as u8])?,
            "LE" => output.write_all(&[ByteCode::Le as u8])?,
            "NOT" => output.write_all(&[ByteCode::Not as u8])?,
            "AND" => output.write_all(&[ByteCode::And as u8])?,
            "OR" => output.write_all(&[ByteCode::Or as u8])?,
            "DUP" => output.write_all(&[ByteCode::Dup as u8])?,
            "PRINT" => output.write_all(&[ByteCode::Print as u8])?,
            "HALT" => output.write_all(&[ByteCode::Halt as u8])?,

            "JMP" | "JZ" | "CALL" => {
                let addr = arg
                    .and_then(|label| labels.get(label))
                    .copied()
                    .expect("Unknown jump label");
                let code = match op {
                    "JMP" => ByteCode::Jmp,
                    "JZ" => ByteCode::Jz,
                    "CALL" => ByteCode::Call,
                    _ => unreachable!(),
                } as u8;
                output.write_all(&[code])?;
                output.write_all(&(addr as u16).to_le_bytes())?;
            }

            "RET" => output.write_all(&[ByteCode::Ret as u8])?,

            "STORE" => {
                output.write_all(&[ByteCode::Store as u8])?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&[bytes.len() as u8])?;
                output.write_all(bytes)?;
            }
            "LOAD" => {
                output.write_all(&[ByteCode::Load as u8])?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&[bytes.len() as u8])?;
                output.write_all(bytes)?;
            }
            "DELETE" => {
                output.write_all(&[ByteCode::Delete as u8])?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&[bytes.len() as u8])?;
                output.write_all(bytes)?;
            }
            "MAKELIST" => {
                output.write_all(&[ByteCode::MakeList as u8])?;
                let n: u8 = arg.unwrap().parse().expect("Invalid list size");
                output.write_all(&[n])?;
            }
            "LEN" => output.write_all(&[ByteCode::Len as u8])?,
            "INDEX" => output.write_all(&[ByteCode::Index as u8])?,
            "READ_FILE" => output.write_all(&[ByteCode::ReadFile as u8])?,
            "WRITE_FILE" => output.write_all(&[ByteCode::WriteFile as u8])?,
            "DUMPSCOPE" => output.write_all(&[ByteCode::DumpScope as u8])?,

            _ => panic!("Unknown opcode: {}", op),
        }
    }

    Ok(())
}
