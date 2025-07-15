// bytecode.rs
use crate::vm::OpCode;
use crate::vm::{VMError, VMResult};
use std::fs::File;
use std::io::{BufReader, Read};
use std::fs;
use std::collections::HashMap;

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
                // Read target address (2 bytes)
                let addr = u16::from_le_bytes(buffer[ip..ip+2].try_into().unwrap()) as usize;
                ip += 2;
                // Read parameter count (2 bytes)
                let count = u16::from_le_bytes(buffer[ip..ip+2].try_into().unwrap()) as usize;
                ip += 2;
                // Read each parameter name
                let mut params = Vec::with_capacity(count);
                for _ in 0..count {
                    let len = u16::from_le_bytes(buffer[ip..ip+2].try_into().unwrap()) as usize;
                    ip += 2;
                    let name_bytes = &buffer[ip..ip+len];
                    let name = String::from_utf8(name_bytes.to_vec()).unwrap();
                    ip += len;
                    params.push(name);
                }
                OpCode::Call{addr, params}
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

            // Concurrency opcodes
            0x0080 => OpCode::Spawn,
            0x0081 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::Register(s)
            }
            0x0082 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::Unregister(s)
            }
            0x0083 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::Whereis(s)
            }
            0x0084 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::SendNamed(s)
            }
            0x0085 => {
                let pid = u64::from_le_bytes(buffer[ip..ip + 8].try_into().unwrap());
                ip += 8;
                OpCode::Monitor(pid)
            }
            0x0086 => {
                let pid = u64::from_le_bytes(buffer[ip..ip + 8].try_into().unwrap());
                ip += 8;
                OpCode::Link(pid)
            }
            0x0087 => {
                let pid = u64::from_le_bytes(buffer[ip..ip + 8].try_into().unwrap());
                ip += 8;
                OpCode::Unlink(pid)
            }
            0x0088 => OpCode::StartSupervisor,
            0x0089 => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::SuperviseChild(s)
            }
            0x008A => {
                let len = u16::from_le_bytes(buffer[ip..ip + 2].try_into().unwrap()) as usize;
                ip += 2;
                let s = String::from_utf8(buffer[ip..ip + len].to_vec()).unwrap();
                ip += len;
                OpCode::RestartChild(s)
            }
            0x008B => OpCode::Yield,
            0x008C => OpCode::Receive,
            0x008D => {
                let pid = u64::from_le_bytes(buffer[ip..ip + 8].try_into().unwrap());
                ip += 8;
                OpCode::Send(pid)
            }

            0x00FF => OpCode::Halt,

            _ => panic!("Unknown bytecode: 0x{:04X}", opcode),
        };

        instructions.push(op);
    }

    Ok(instructions)
}

pub fn parse_program(path: &str) -> VMResult<Vec<OpCode>> {
    let content = fs::read_to_string(path).map_err(|e| VMError::FileError { 
        filename: path.to_string(), 
        error: e.to_string() 
    })?;

    let mut label_map: HashMap<String, usize> = HashMap::new();
    let mut instructions_raw: Vec<(usize, &str)> = Vec::new();

    // First pass: build label -> index map
    for (line_num, line) in content.lines().enumerate() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("LABEL ") {
            let label_name = line[6..].trim();
            label_map.insert(label_name.to_string(), instructions_raw.len());
        } else {
            instructions_raw.push((line_num + 1, line)); // save for second pass
        }
    }

    // Second pass: convert raw instructions to OpCode using label map
    let mut program: Vec<OpCode> = Vec::new();
    for (line_num, line) in instructions_raw {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let opcode = match parts[0] {
            "PUSH_INT" => {
                let n = parts[1].parse::<i64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid integer: {}", parts[1]) 
                })?;
                OpCode::PushInt(n)
            }
            "PUSH_FLOAT" => {
                let f = parts[1].parse::<f64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid float: {}", parts[1]) 
                })?;
                OpCode::PushFloat(f)
            }
            "PUSH_STR" => {
                let s = parts[1].trim_matches('"').to_string();
                OpCode::PushStr(s)
            }
            "PUSH_BOOL" => {
                let b = parts[1].parse::<bool>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid boolean: {}", parts[1]) 
                })?;
                OpCode::PushBool(b)
            }
            "ADD" => OpCode::Add,
            "ADD_F" => OpCode::AddF,
            "SUB" => OpCode::Sub,
            "SUB_F" => OpCode::SubF,
            "MUL" => OpCode::Mul,
            "MUL_F" => OpCode::MulF,
            "DIV" => OpCode::Div,
            "DIV_F" => OpCode::DivF,
            "DUP" => OpCode::Dup,
            "CONCAT" => OpCode::Concat,
            "PRINT" => OpCode::Print,
            "HALT" => OpCode::Halt,
            "CALL" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: "CALL requires at least a target".to_string() });
                }
                let call_parts: Vec<&str> = parts[1].split_whitespace().collect();
                let label = call_parts[0];
                let params: Vec<String> = call_parts[1..].iter().map(|s| s.to_string()).collect();
                
                // Try parsing as number first, then as label
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Call { addr: target, params }
            }
            "JMP" => {
                let label = parts[1].trim();
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Jmp(target)
            }
            "JZ" => {
                let label = parts[1].trim();
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Jz(target)
            }
            "RET" => OpCode::Ret,
            "STORE" => {
                let var = parts[1].trim().to_string();
                OpCode::Store(var)
            }
            "DELETE" => {
                let var = parts[1].trim().to_string();
                OpCode::Delete(var)
            }
            "LOAD" => {
                let var = parts[1].trim().to_string();
                OpCode::Load(var)
            }
            "EQ" => OpCode::Eq,
            "GT" => OpCode::Gt,
            "LT" => OpCode::Lt,
            "NE" => OpCode::Ne,
            "GE" => OpCode::Ge,
            "LE" => OpCode::Le,
            "EQ_F" => OpCode::EqF,
            "GT_F" => OpCode::GtF,
            "LT_F" => OpCode::LtF,
            "NE_F" => OpCode::NeF,
            "GE_F" => OpCode::GeF,
            "LE_F" => OpCode::LeF,
            "TRUE" => OpCode::True,
            "FALSE" => OpCode::False,
            "NOT" => OpCode::Not,
            "AND" => OpCode::And,
            "OR" => OpCode::Or,
            "NULL" => OpCode::Null,
            "MAKE_LIST" => {
                let n = parts[1].parse::<usize>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid list size: {}", parts[1]) 
                })?;
                OpCode::MakeList(n)
            }
            "LEN" => OpCode::Len,
            "INDEX" => OpCode::Index,
            "DUMP_SCOPE" => OpCode::DumpScope,
            "MAKE_OBJECT" => OpCode::MakeObject,
            "SET_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::SetField(field)
            }
            "GET_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::GetField(field)
            }
            "HAS_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::HasField(field)
            }
            "DELETE_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::DeleteField(field)
            }
            "KEYS" => OpCode::Keys,
            "MAKE_FUNCTION" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: "MAKE_FUNCTION requires at least a target".to_string() });
                }
                let func_parts: Vec<&str> = parts[1].split_whitespace().collect();
                let label = func_parts[0];
                let params: Vec<String> = func_parts[1..].iter().map(|s| s.to_string()).collect();
                
                // Try parsing as number first, then as label
                let addr = if let Ok(address) = label.parse::<usize>() {
                    address
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::MakeFunction { addr, params }
            }
            "CALL_FUNCTION" => OpCode::CallFunction,
            "MAKE_LAMBDA" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: line.to_string() });
                }
                
                let remaining_parts: Vec<&str> = parts[1].split_whitespace().collect();
                if remaining_parts.is_empty() {
                    return Err(VMError::ParseError { line: line_num, instruction: line.to_string() });
                }
                
                let label = remaining_parts[0];
                let params = remaining_parts[1..].iter().map(|s| s.to_string()).collect();
                
                let addr = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::MakeLambda { addr, params }
            }
            "CAPTURE" => {
                let var = parts[1].trim().to_string();
                OpCode::Capture(var)
            }
            "TRY" => {
                let catch_label = parts[1].trim();
                let catch_addr = *label_map.get(catch_label).ok_or_else(|| VMError::UnknownLabel(catch_label.to_string()))?;
                OpCode::Try { catch_addr }
            }
            "CATCH" => OpCode::Catch,
            "THROW" => OpCode::Throw,
            "END_TRY" => OpCode::EndTry,
            "READ_FILE" => OpCode::ReadFile,
            "WRITE_FILE" => OpCode::WriteFile,
            // Enhanced I/O operations
            "READ_LINE" => OpCode::ReadLine,
            "READ_CHAR" => OpCode::ReadChar,
            "READ_INPUT" => OpCode::ReadInput,
            "APPEND_FILE" => OpCode::AppendFile,
            "FILE_EXISTS" => OpCode::FileExists,
            "FILE_SIZE" => OpCode::FileSize,
            "DELETE_FILE" => OpCode::DeleteFile,
            "LIST_DIR" => OpCode::ListDir,
            "READ_BYTES" => OpCode::ReadBytes,
            "WRITE_BYTES" => OpCode::WriteBytes,
            // Environment and system
            "GET_ENV" => OpCode::GetEnv,
            "SET_ENV" => OpCode::SetEnv,
            "GET_ARGS" => OpCode::GetArgs,
            "EXEC" => OpCode::Exec,
            "EXEC_CAPTURE" => OpCode::ExecCapture,
            "EXIT" => OpCode::Exit,
            // Time operations
            "GET_TIME" => OpCode::GetTime,
            "SLEEP" => OpCode::Sleep,
            "FORMAT_TIME" => OpCode::FormatTime,
            // Network operations
            "HTTP_GET" => OpCode::HttpGet,
            "HTTP_POST" => OpCode::HttpPost,
            "TCP_CONNECT" => OpCode::TcpConnect,
            "TCP_LISTEN" => OpCode::TcpListen,
            "TCP_SEND" => OpCode::TcpSend,
            "TCP_RECV" => OpCode::TcpRecv,
            "UDP_BIND" => OpCode::UdpBind,
            "UDP_SEND" => OpCode::UdpSend,
            "UDP_RECV" => OpCode::UdpRecv,
            "DNS_RESOLVE" => OpCode::DnsResolve,
            // Advanced I/O operations
            "ASYNC_READ" => OpCode::AsyncRead,
            "ASYNC_WRITE" => OpCode::AsyncWrite,
            "AWAIT" => OpCode::Await,
            "STREAM_CREATE" => OpCode::StreamCreate,
            "STREAM_READ" => OpCode::StreamRead,
            "STREAM_WRITE" => OpCode::StreamWrite,
            "STREAM_CLOSE" => OpCode::StreamClose,
            "JSON_PARSE" => OpCode::JsonParse,
            "JSON_STRINGIFY" => OpCode::JsonStringify,
            "CSV_PARSE" => OpCode::CsvParse,
            "CSV_WRITE" => OpCode::CsvWrite,
            "COMPRESS" => OpCode::Compress,
            "DECOMPRESS" => OpCode::Decompress,
            "ENCRYPT" => OpCode::Encrypt,
            "DECRYPT" => OpCode::Decrypt,
            "HASH" => OpCode::Hash,
            "DB_CONNECT" => OpCode::DbConnect,
            "DB_QUERY" => OpCode::DbQuery,
            "DB_EXEC" => OpCode::DbExec,
            "IMPORT" => {
                let path = parts[1].trim();
                // Remove quotes if present
                let path = if path.starts_with('"') && path.ends_with('"') {
                    path[1..path.len()-1].to_string()
                } else {
                    path.to_string()
                };
                OpCode::Import(path)
            }
            "EXPORT" => {
                let name = parts[1].trim().to_string();
                OpCode::Export(name)
            }
            "YIELD" => OpCode::Yield,
            "RECEIVE" => OpCode::Receive,
            "SEND" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Send(pid)
            }
            "REGISTER" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::Register(name)
            }
            "WHEREIS" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::Whereis(name)
            }
            "SPAWN" => OpCode::Spawn,
            "SENDNAMED" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::SendNamed(name)
            }
            "UNREGISTER" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::Unregister(name)
            }
            "TRAP_EXIT" => OpCode::TrapExit,
            "START_SUPERVISOR" => OpCode::StartSupervisor,
            "SUPERVISE_CHILD" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::SuperviseChild(name)
            }
            "RESTART_CHILD" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::RestartChild(name)
            }
            "LINK" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Link(pid)
            }
            "UNLINK" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Unlink(pid)
            }
            "MONITOR" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Monitor(pid)
            }
            "DEMONITOR" => {
                let monitor_ref = parts[1].trim_matches('"').to_string();
                OpCode::Demonitor(monitor_ref)
            }
            _ => return Err(VMError::ParseError { line: line_num, instruction: line.to_string() }),
        };
        program.push(opcode);
    }

    Ok(program)
}
