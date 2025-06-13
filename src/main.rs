use std::env;
use std::fs;
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum Value {
    Int(i64),
    Str(String),
}

#[derive(Debug)]
enum OpCode {
    PushInt(i64),
    PushStr(String),
    Add,
    Concat,
    Print,
    Halt,
    Jmp(usize),
    Jz(usize),
    Call(usize),
    Ret,
    Sub,
    Dup,
    Store(String),
    Load(String),
}

struct VM {
    stack: Vec<Value>,
    instructions: Vec<OpCode>,
    ip: usize, // instruction pointer
    call_stack: Vec<usize>, // return addresses for CALL/RET
    variables: HashMap<String, Value>,
}

impl VM {
    fn new(instructions: Vec<OpCode>) -> Self {
        VM {
            stack: Vec::new(),
            instructions,
            ip: 0,
            call_stack: Vec::new(),
            variables: HashMap::new(),
        }
    }
    

    fn run(&mut self) {
        while self.ip < self.instructions.len() {
            match &self.instructions[self.ip] {
                OpCode::PushInt(n) => self.stack.push(Value::Int(*n)),
                OpCode::PushStr(s) => self.stack.push(Value::Str(s.clone())),
                OpCode::Add => {
                    let b = self.stack.pop().expect("stack underflow");
                    let a = self.stack.pop().expect("stack underflow");
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x + y)),
                        _ => panic!("ADD expects two integers"),
                    }
                }
                OpCode::Concat => {
                    let b = self.stack.pop().expect("stack underflow");
                    let a = self.stack.pop().expect("stack underflow");
                    match (a, b) {
                        (Value::Str(x), Value::Str(y)) => self.stack.push(Value::Str(x + &y)),
                        _ => panic!("CONCAT expects two strings"),
                    }
                }
                OpCode::Print => {
                    let val = self.stack.pop().expect("stack underflow");
                    println!("{:?}", val);
                }
                OpCode::Jmp(target) => {
                    self.ip = *target;
                    continue;
                }
                OpCode::Jz(target) => {
                    let val = self.stack.pop().expect("stack underflow");
                    match val {
                        Value::Int(0) => {
                            self.ip = *target;
                            continue;
                        }
                        Value::Int(_) => {} // no jump
                        _ => panic!("JZ requires an integer"),
                    }
                }
                OpCode::Call(target) => {
                    self.call_stack.push(self.ip + 1);
                    self.ip = *target;
                    continue;
                }
                OpCode::Ret => {
                    self.ip = self.call_stack.pop().expect("call stack underflow");
                    continue;
                }
                OpCode::Sub => {
                    let b = self.stack.pop().expect("stack underflow on SUB");
                    let a = self.stack.pop().expect("stack underflow on SUB");
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x - y)),
                        _ => panic!("SUB expects two integers"),
                    }
                }
                OpCode::Dup => {
                    let val = self.stack.last().expect("stack underflow on DUP").clone();
                    self.stack.push(val);
                }      
                OpCode::Store(name) => {
                    let val = self.stack.pop().expect("stack underflow on STORE");
                    self.variables.insert(name.clone(), val);
                }
                OpCode::Load(name) => {
                    let val = self.variables.get(name)
                        .unwrap_or_else(|| panic!("Undefined variable: {}", name))
                        .clone();
                    self.stack.push(val);
                }                          
                OpCode::Halt => break,
            }
            self.ip += 1;
        }        
    }
}

fn parse_program(path: &str) -> Vec<OpCode> {
    let content = fs::read_to_string(path).expect("Failed to read file");

    let mut label_map: HashMap<String, usize> = HashMap::new();
    let mut instructions_raw: Vec<(usize, &str)> = Vec::new();

    // First pass: build label -> index map
    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
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
                let n = parts[1].parse::<i64>().expect("Invalid integer");
                OpCode::PushInt(n)
            }
            "PUSH_STR" => {
                let s = parts[1].trim_matches('"').to_string();
                OpCode::PushStr(s)
            }
            "ADD" => OpCode::Add,
            "SUB" => OpCode::Sub,
            "DUP" => OpCode::Dup,
            "CONCAT" => OpCode::Concat,
            "PRINT" => OpCode::Print,
            "HALT" => OpCode::Halt,
            "CALL" => {
                let label = parts[1].trim();
                let target = label_map.get(label).expect("Unknown label in CALL");
                OpCode::Call(*target)
            }
            "JMP" => {
                let label = parts[1].trim();
                let target = label_map.get(label).expect("Unknown label in JMP");
                OpCode::Jmp(*target)
            }
            "JZ" => {
                let label = parts[1].trim();
                let target = label_map.get(label).expect("Unknown label in JZ");
                OpCode::Jz(*target)
            }
            "RET" => OpCode::Ret,
            "STORE" => {
                let var = parts[1].trim().to_string();
                OpCode::Store(var)
            }
            "LOAD" => {
                let var = parts[1].trim().to_string();
                OpCode::Load(var)
            }
            _ => panic!("Unknown instruction: {line} on line {line_num}"),
        };
        program.push(opcode);
    }

    program
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: tinytotvm <file.ttvm>");
        std::process::exit(1);
    }

    let program = parse_program(&args[1]);
    let mut vm = VM::new(program);
    vm.run();
}
