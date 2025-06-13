use std::collections::HashMap;
use std::fs;
mod bytecode;
mod compiler;
mod lisp_compiler;

#[derive(Debug, Clone)]
enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    Null,
    List(Vec<Value>),
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
    Delete(String),
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    True,
    False,
    Not,
    And,
    Or,
    Null,
    MakeList(usize), // operand: how many items to pop
    Len,
    Index,
    DumpScope,
    ReadFile,
    WriteFile,
}

struct VM {
    stack: Vec<Value>,
    instructions: Vec<OpCode>,
    ip: usize,                              // instruction pointer
    call_stack: Vec<usize>,                 // return addresses for CALL/RET
    variables: Vec<HashMap<String, Value>>, // call frame stack
}

impl VM {
    fn new(instructions: Vec<OpCode>) -> Self {
        VM {
            stack: Vec::new(),
            instructions,
            ip: 0,
            call_stack: Vec::new(),
            variables: vec![HashMap::new()], // global frame
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
                    let val = self.stack.pop().expect("stack underflow on JZ");
                    let is_zero = match val {
                        Value::Int(0) => true,
                        Value::Bool(false) => true,
                        Value::Null => true,
                        _ => false,
                    };
                    if is_zero {
                        self.ip = *target;
                        continue;
                    }
                }
                OpCode::Call(target) => {
                    self.call_stack.push(self.ip + 1);
                    self.variables.push(HashMap::new()); // push new scope
                    self.ip = *target;
                    continue;
                }
                OpCode::Ret => {
                    self.variables.pop().expect("call frame stack underflow");
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
                    self.variables
                        .last_mut()
                        .expect("no variable scope")
                        .insert(name.clone(), val);
                }
                OpCode::Load(name) => {
                    let val = self
                        .variables
                        .last()
                        .expect("no variable scope")
                        .get(name)
                        .unwrap_or_else(|| panic!("Undefined variable: {}", name))
                        .clone();
                    self.stack.push(val);
                }
                OpCode::Delete(name) => {
                    let removed = self
                        .variables
                        .last_mut()
                        .expect("no variable scope")
                        .remove(name);
                    if removed.is_none() {
                        eprintln!("Warning: tried to DELETE unknown variable '{}'", name);
                    }
                }
                OpCode::Eq => {
                    let b = self.stack.pop().expect("stack underflow on EQ");
                    let a = self.stack.pop().expect("stack underflow on EQ");
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x == y,
                        (Value::Str(x), Value::Str(y)) => x == y,
                        _ => panic!("EQ requires values of the same type"),
                    };
                    self.stack.push(Value::Int(if result { 1 } else { 0 }));
                }
                OpCode::Gt => {
                    let b = self.stack.pop().expect("stack underflow on GT");
                    let a = self.stack.pop().expect("stack underflow on GT");
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x > y { 1 } else { 0 }));
                        }
                        _ => panic!("GT requires two integers"),
                    }
                }
                OpCode::Lt => {
                    let b = self.stack.pop().expect("stack underflow on LT");
                    let a = self.stack.pop().expect("stack underflow on LT");
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x < y { 1 } else { 0 }));
                        }
                        _ => panic!("LT requires two integers"),
                    }
                }
                OpCode::Ne => {
                    let b = self.stack.pop().expect("stack underflow on NE");
                    let a = self.stack.pop().expect("stack underflow on NE");
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x != y,
                        (Value::Str(x), Value::Str(y)) => x != y,
                        _ => panic!("NE requires values of the same type"),
                    };
                    self.stack.push(Value::Int(if result { 1 } else { 0 }));
                }
                OpCode::Ge => {
                    let b = self.stack.pop().expect("stack underflow on GE");
                    let a = self.stack.pop().expect("stack underflow on GE");
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x >= y { 1 } else { 0 }));
                        }
                        _ => panic!("GE requires two integers"),
                    }
                }
                OpCode::Le => {
                    let b = self.stack.pop().expect("stack underflow on LE");
                    let a = self.stack.pop().expect("stack underflow on LE");
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x <= y { 1 } else { 0 }));
                        }
                        _ => panic!("LE requires two integers"),
                    }
                }
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::False => self.stack.push(Value::Bool(false)),
                OpCode::Not => {
                    let val = self.stack.pop().expect("stack underflow on NOT");
                    let result = match val {
                        Value::Bool(b) => !b,
                        Value::Int(i) => i == 0, // optional: treat 0 as false
                        _ => panic!("NOT expects a Bool or Int"),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::And => {
                    let b = self.stack.pop().expect("stack underflow on AND");
                    let a = self.stack.pop().expect("stack underflow on AND");
                    let result = match (a, b) {
                        (Value::Bool(x), Value::Bool(y)) => x && y,
                        _ => panic!("AND expects two Booleans"),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::Or => {
                    let b = self.stack.pop().expect("stack underflow on OR");
                    let a = self.stack.pop().expect("stack underflow on OR");
                    let result = match (a, b) {
                        (Value::Bool(x), Value::Bool(y)) => x || y,
                        _ => panic!("OR expects two Booleans"),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::Null => {
                    self.stack.push(Value::Null);
                }
                OpCode::MakeList(n) => {
                    if self.stack.len() < *n {
                        panic!("Not enough values on stack for MAKE_LIST");
                    }
                    let mut list = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        list.push(self.stack.pop().unwrap());
                    }
                    list.reverse(); // keep original order
                    self.stack.push(Value::List(list));
                }
                OpCode::Len => {
                    let val = self.stack.pop().expect("stack underflow on LEN");
                    match val {
                        Value::List(l) => self.stack.push(Value::Int(l.len() as i64)),
                        _ => panic!("LEN expects a list"),
                    }
                }
                OpCode::Index => {
                    let index = match self.stack.pop().expect("stack underflow on INDEX") {
                        Value::Int(i) => i as usize,
                        _ => panic!("INDEX expects an integer index"),
                    };
                    let list = match self.stack.pop().expect("stack underflow on INDEX") {
                        Value::List(l) => l,
                        _ => panic!("INDEX expects a list"),
                    };
                    if index >= list.len() {
                        panic!("Index out of bounds");
                    }
                    self.stack.push(list[index].clone());
                }
                OpCode::ReadFile => {
                    let val = self.stack.pop().expect("stack underflow");
                    if let Value::Str(filename) = val {
                        match std::fs::read_to_string(&filename) {
                            Ok(content) => self.stack.push(Value::Str(content)),
                            Err(e) => panic!("Failed to read file {}: {}", filename, e),
                        }
                    } else {
                        panic!("READ_FILE expects a string filename");
                    }
                }
                OpCode::WriteFile => {
                    let filename = self.stack.pop().expect("stack underflow");
                    let content = self.stack.pop().expect("stack underflow");
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(body)) => {
                            if let Err(e) = std::fs::write(&fname, body) {
                                panic!("Failed to write to file {}: {}", fname, e);
                            }
                        }
                        _ => panic!("WRITE_FILE expects two strings (filename, content)"),
                    }
                }
                OpCode::DumpScope => {
                    println!("Current scope: {:?}", self.variables.last());
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
            "TRUE" => OpCode::True,
            "FALSE" => OpCode::False,
            "NOT" => OpCode::Not,
            "AND" => OpCode::And,
            "OR" => OpCode::Or,
            "NULL" => OpCode::Null,
            "MAKE_LIST" => {
                let n = parts[1].parse::<usize>().expect("Invalid list size");
                OpCode::MakeList(n)
            }
            "LEN" => OpCode::Len,
            "INDEX" => OpCode::Index,
            "DUMP_SCOPE" => OpCode::DumpScope,
            _ => panic!("Unknown instruction: {line} on line {line_num}"),
        };
        program.push(opcode);
    }

    program
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 && args[1] == "compile" {
        if args.len() != 4 {
            eprintln!("Usage: tinytotvm compile <input.ttvm> <output.ttb>");
            std::process::exit(1);
        }
        let input = &args[2];
        let output = &args[3];
        compiler::compile(input, output).expect("Compilation failed");
        println!("Compiled to {}", output);
        return;
    }

    if args[1].ends_with(".ttb") {
        let program = bytecode::load_bytecode(&args[1]).expect("Failed to load bytecode");
        let mut vm = VM::new(program);
        vm.run();
        return;
    }

    if args[1] == "compile-lisp" {
        if args.len() != 4 {
            eprintln!("Usage: tinytotvm compile-lisp <input.lisp> <output.ttvm>");
            std::process::exit(1);
        }
        let input = &args[2];
        let output = &args[3];
        lisp_compiler::compile_lisp(input, output);
        println!("Compiled Lisp to {}", output);
        return;
    }

    let program = parse_program(&args[1]);
    let mut vm = VM::new(program);
    vm.run();
}
