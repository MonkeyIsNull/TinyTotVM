// compiler.rs
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[repr(u16)]
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
    
    // Concurrency opcodes
    Spawn = 0x80,
    Register = 0x81,
    Unregister = 0x82,
    Whereis = 0x83,
    SendNamed = 0x84,
    Monitor = 0x85,
    Link = 0x86,
    Unlink = 0x87,
    StartSupervisor = 0x88,
    SuperviseChild = 0x89,
    RestartChild = 0x8A,
    Yield = 0x8B,
    Receive = 0x8C,
    Send = 0x8D,
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
                output.write_all(&(ByteCode::PushInt as u16).to_le_bytes())?;
                let n: i64 = arg.unwrap().parse().expect("Invalid integer");
                output.write_all(&n.to_le_bytes())?;
            }
            "PUSH_STR" => {
                output.write_all(&(ByteCode::PushStr as u16).to_le_bytes())?;
                let s = arg.unwrap().trim_matches('"');
                let bytes = s.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "TRUE" => output.write_all(&(ByteCode::True as u16).to_le_bytes())?,
            "FALSE" => output.write_all(&(ByteCode::False as u16).to_le_bytes())?,
            "NULL" => output.write_all(&(ByteCode::Null as u16).to_le_bytes())?,
            "ADD" => output.write_all(&(ByteCode::Add as u16).to_le_bytes())?,
            "SUB" => output.write_all(&(ByteCode::Sub as u16).to_le_bytes())?,
            "CONCAT" => output.write_all(&(ByteCode::Concat as u16).to_le_bytes())?,
            "EQ" => output.write_all(&(ByteCode::Eq as u16).to_le_bytes())?,
            "NE" => output.write_all(&(ByteCode::Ne as u16).to_le_bytes())?,
            "GT" => output.write_all(&(ByteCode::Gt as u16).to_le_bytes())?,
            "LT" => output.write_all(&(ByteCode::Lt as u16).to_le_bytes())?,
            "GE" => output.write_all(&(ByteCode::Ge as u16).to_le_bytes())?,
            "LE" => output.write_all(&(ByteCode::Le as u16).to_le_bytes())?,
            "NOT" => output.write_all(&(ByteCode::Not as u16).to_le_bytes())?,
            "AND" => output.write_all(&(ByteCode::And as u16).to_le_bytes())?,
            "OR" => output.write_all(&(ByteCode::Or as u16).to_le_bytes())?,
            "DUP" => output.write_all(&(ByteCode::Dup as u16).to_le_bytes())?,
            "PRINT" => output.write_all(&(ByteCode::Print as u16).to_le_bytes())?,
            "HALT" => output.write_all(&(ByteCode::Halt as u16).to_le_bytes())?,

            "JMP" | "JZ" => {
                let addr = arg
                    .and_then(|label| labels.get(label))
                    .copied()
                    .expect("Unknown jump label");
                let code = match op {
                    "JMP" => ByteCode::Jmp,
                    "JZ" => ByteCode::Jz,
                    _ => unreachable!(),
                };
                output.write_all(&(code as u16).to_le_bytes())?;
                output.write_all(&(addr as u16).to_le_bytes())?;
            }
            "CALL" => {
                    output.write_all(&(ByteCode::Call as u16).to_le_bytes())?;
                    let tokens: Vec<&str> = arg.unwrap().split_whitespace().collect();
                    let label_name = tokens[0];
                    let addr = labels.get(label_name)
                            .copied()
                            .expect("Unknown call label");
                    // Write the function address (2 bytes)
                    output.write_all(&(addr as u16).to_le_bytes())?;
                    // Write parameter count (2 bytes)
                    let param_count = (tokens.len() - 1) as u16;
                    output.write_all(&param_count.to_le_bytes())?;
                    // Write each parameter name (length + bytes)
                    for param in &tokens[1..] {
                        let name_bytes = param.as_bytes();
                        output.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                        output.write_all(name_bytes)?;
                    }
                }
            "RET" => output.write_all(&(ByteCode::Ret as u16).to_le_bytes())?,

            "STORE" => {
                output.write_all(&(ByteCode::Store as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "LOAD" => {
                output.write_all(&(ByteCode::Load as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "DELETE" => {
                output.write_all(&(ByteCode::Delete as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }

            "MAKELIST" => {
                output.write_all(&(ByteCode::MakeList as u16).to_le_bytes())?;
                let n: u8 = arg.unwrap().parse().expect("Invalid list size");
                output.write_all(&[n])?;
            }
            "LEN" => output.write_all(&(ByteCode::Len as u16).to_le_bytes())?,
            "INDEX" => output.write_all(&(ByteCode::Index as u16).to_le_bytes())?,
            "READ_FILE" => output.write_all(&(ByteCode::ReadFile as u16).to_le_bytes())?,
            "WRITE_FILE" => output.write_all(&(ByteCode::WriteFile as u16).to_le_bytes())?,
            "DUMPSCOPE" => output.write_all(&(ByteCode::DumpScope as u16).to_le_bytes())?,
            
            // Concurrency opcodes
            "SPAWN" => output.write_all(&(ByteCode::Spawn as u16).to_le_bytes())?,
            "REGISTER" => {
                output.write_all(&(ByteCode::Register as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "UNREGISTER" => {
                output.write_all(&(ByteCode::Unregister as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "WHEREIS" => {
                output.write_all(&(ByteCode::Whereis as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "SEND_NAMED" => {
                output.write_all(&(ByteCode::SendNamed as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "MONITOR" => {
                output.write_all(&(ByteCode::Monitor as u16).to_le_bytes())?;
                let pid: u64 = arg.unwrap().parse().expect("Invalid PID");
                output.write_all(&pid.to_le_bytes())?;
            }
            "LINK" => {
                output.write_all(&(ByteCode::Link as u16).to_le_bytes())?;
                let pid: u64 = arg.unwrap().parse().expect("Invalid PID");
                output.write_all(&pid.to_le_bytes())?;
            }
            "UNLINK" => {
                output.write_all(&(ByteCode::Unlink as u16).to_le_bytes())?;
                let pid: u64 = arg.unwrap().parse().expect("Invalid PID");
                output.write_all(&pid.to_le_bytes())?;
            }
            "START_SUPERVISOR" => output.write_all(&(ByteCode::StartSupervisor as u16).to_le_bytes())?,
            "SUPERVISE_CHILD" => {
                output.write_all(&(ByteCode::SuperviseChild as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "RESTART_CHILD" => {
                output.write_all(&(ByteCode::RestartChild as u16).to_le_bytes())?;
                let name = arg.unwrap().trim_matches('"');
                let bytes = name.as_bytes();
                output.write_all(&(bytes.len() as u16).to_le_bytes())?;
                output.write_all(bytes)?;
            }
            "YIELD" => output.write_all(&(ByteCode::Yield as u16).to_le_bytes())?,
            "RECEIVE" => output.write_all(&(ByteCode::Receive as u16).to_le_bytes())?,
            "SEND" => {
                output.write_all(&(ByteCode::Send as u16).to_le_bytes())?;
                let pid: u64 = arg.unwrap().parse().expect("Invalid PID");
                output.write_all(&pid.to_le_bytes())?;
            }

            _ => panic!("Unknown opcode: {}", op),
        }
    }
    Ok(())
}
