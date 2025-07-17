use crate::vm::Value;
use std::collections::HashMap;

pub mod lowering;
pub mod vm;

pub type RegId = u32;
pub type VarId = String;
pub type FnId = usize;

#[derive(Debug, Clone)]
pub enum RegValue {
    Const(Value),
    Reg(RegId),
}

#[derive(Debug, Clone)]
pub enum RegInstr {
    // Data movement
    Mov(RegId, RegValue),
    
    // Arithmetic operations
    Add(RegId, RegId, RegId),
    AddF(RegId, RegId, RegId),
    Sub(RegId, RegId, RegId),
    SubF(RegId, RegId, RegId),
    Mul(RegId, RegId, RegId),
    MulF(RegId, RegId, RegId),
    Div(RegId, RegId, RegId),
    DivF(RegId, RegId, RegId),
    
    // String operations
    Concat(RegId, RegId, RegId),
    
    // Comparison operations
    Eq(RegId, RegId, RegId),
    Ne(RegId, RegId, RegId),
    Lt(RegId, RegId, RegId),
    Le(RegId, RegId, RegId),
    Gt(RegId, RegId, RegId),
    Ge(RegId, RegId, RegId),
    EqF(RegId, RegId, RegId),
    NeF(RegId, RegId, RegId),
    LtF(RegId, RegId, RegId),
    LeF(RegId, RegId, RegId),
    GtF(RegId, RegId, RegId),
    GeF(RegId, RegId, RegId),
    
    // Logical operations
    And(RegId, RegId, RegId),
    Or(RegId, RegId, RegId),
    Not(RegId, RegId),
    
    // Memory operations
    Load(RegId, VarId),
    Store(VarId, RegId),
    
    // Control flow
    Call(Option<RegId>, FnId, Vec<RegId>), // Optional return register
    Ret(Option<RegId>),
    Jmp(usize),
    Jz(RegId, usize),
    
    // Built-in operations
    Print(RegId),
    Len(RegId, RegId),
    Index(RegId, RegId, RegId),
    MakeList(RegId, Vec<RegId>),
    MakeObject(RegId),
    
    // Object operations
    SetField(RegId, String, RegId), // object, field_name, value
    GetField(RegId, RegId, String), // dst, object, field_name
    HasField(RegId, RegId, String), // dst, object, field_name
    DeleteField(RegId, String),     // object, field_name
    Keys(RegId, RegId),             // dst, object
    
    // Function operations
    MakeFunction(RegId, usize, Vec<String>), // dst, addr, params
    CallFunction(Option<RegId>, RegId, Vec<RegId>), // dst, func_reg, args
    MakeLambda(RegId, usize, Vec<String>),   // dst, addr, params
    Capture(String, RegId),                  // var_name, value
    
    // Exception handling
    Try(usize),        // catch_addr
    Catch(RegId),      // exception register
    Throw(RegId),      // exception value
    EndTry,
    
    // Module system  
    Import(RegId, String), // dst, module_path
    Export(String, RegId), // name, value
    
    // Concurrency operations
    Spawn(RegId, RegId),               // dst_pid, function
    Receive(RegId),                    // dst_message
    ReceiveMatch(RegId, Vec<crate::vm::MessagePattern>), // dst, patterns
    Yield,
    Send(RegId, RegId),                // target_pid, message
    Monitor(RegId, RegId),             // dst_ref, target_pid
    Demonitor(RegId),                  // monitor_ref
    Link(RegId),                       // target_pid
    Unlink(RegId),                     // target_pid
    TrapExit(RegId),                   // enable_flag
    
    // Process registry operations
    Register(String, RegId),           // name, pid
    Unregister(String),                // name
    Whereis(RegId, String),            // dst, name
    SendNamed(String, RegId),          // name, message
    
    // Supervision operations
    StartSupervisor(RegId),            // dst_pid
    SuperviseChild(String, RegId),     // strategy, child_spec
    RestartChild(String),              // child_name
    
    // I/O operations
    ReadFile(RegId, RegId),            // dst, filename
    WriteFile(RegId, RegId),           // filename, content
    ReadLine(RegId),                   // dst
    ReadChar(RegId),                   // dst
    ReadInput(RegId),                  // dst
    AppendFile(RegId, RegId),          // filename, content
    FileExists(RegId, RegId),          // dst, filename
    FileSize(RegId, RegId),            // dst, filename
    DeleteFile(RegId),                 // filename
    ListDir(RegId, RegId),             // dst, path
    ReadBytes(RegId, RegId),           // dst, filename
    WriteBytes(RegId, RegId),          // filename, data
    
    // Environment operations
    GetEnv(RegId, RegId),              // dst, var_name
    SetEnv(RegId, RegId),              // var_name, value
    GetArgs(RegId),                    // dst
    Exec(RegId, RegId),                // dst, command
    ExecCapture(RegId, RegId),         // dst, command
    Exit(RegId),                       // status_code
    
    // Time operations
    GetTime(RegId),                    // dst
    Sleep(RegId),                      // duration
    FormatTime(RegId, RegId, RegId),   // dst, timestamp, format
    
    // Network operations
    HttpGet(RegId, RegId),             // dst, url
    HttpPost(RegId, RegId, RegId),     // dst, url, data
    TcpConnect(RegId, RegId, RegId),   // dst, host, port
    TcpListen(RegId, RegId),           // dst, port
    TcpSend(RegId, RegId),             // socket, data
    TcpRecv(RegId, RegId),             // dst, socket
    UdpBind(RegId, RegId),             // dst, port
    UdpSend(RegId, RegId, RegId),      // socket, addr, data
    UdpRecv(RegId, RegId),             // dst, socket
    DnsResolve(RegId, RegId),          // dst, hostname
    
    // Advanced I/O operations
    AsyncRead(RegId, RegId),           // dst, filename
    AsyncWrite(RegId, RegId),          // filename, content
    Await(RegId, RegId),               // dst, async_op
    StreamCreate(RegId),               // dst
    StreamRead(RegId, RegId),          // dst, stream
    StreamWrite(RegId, RegId),         // stream, data
    StreamClose(RegId),                // stream
    JsonParse(RegId, RegId),           // dst, json_str
    JsonStringify(RegId, RegId),       // dst, value
    CsvParse(RegId, RegId),            // dst, csv_str
    CsvWrite(RegId, RegId),            // dst, data
    Compress(RegId, RegId),            // dst, data
    Decompress(RegId, RegId),          // dst, compressed_data
    Encrypt(RegId, RegId, RegId),      // dst, data, key
    Decrypt(RegId, RegId, RegId),      // dst, encrypted_data, key
    Hash(RegId, RegId),                // dst, data
    DbConnect(RegId, RegId),           // dst, connection_string
    DbQuery(RegId, RegId, RegId),      // dst, connection, query
    DbExec(RegId, RegId),              // connection, command
    
    // Misc operations
    DumpScope(RegId),                  // dst (scope info)
    Delete(String),                    // var_name
    
    // Boolean constants
    True(RegId),                       // dst = true
    False(RegId),                      // dst = false
    Null(RegId),                       // dst = null
    
    // Special operations
    Halt,
    Nop,
}

#[derive(Debug, Clone)]
pub struct RegBlock {
    pub instructions: Vec<RegInstr>,
    pub entry: usize,
    pub register_count: u32,
    pub variables: HashMap<String, RegId>, // Variable name to register mapping
}

impl RegBlock {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            entry: 0,
            register_count: 0,
            variables: HashMap::new(),
        }
    }
    
    pub fn alloc_register(&mut self) -> RegId {
        let reg = self.register_count;
        self.register_count += 1;
        reg
    }
    
    pub fn add_instruction(&mut self, instr: RegInstr) {
        self.instructions.push(instr);
    }
    
    pub fn get_or_alloc_var(&mut self, var_name: &str) -> RegId {
        if let Some(&reg) = self.variables.get(var_name) {
            reg
        } else {
            let reg = self.alloc_register();
            self.variables.insert(var_name.to_string(), reg);
            reg
        }
    }
}

#[derive(Debug)]
pub struct StackState {
    // Simulated stack for lowering
    stack: Vec<RegId>,
}

impl StackState {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
        }
    }
    
    pub fn push(&mut self, reg: RegId) {
        self.stack.push(reg);
    }
    
    pub fn pop(&mut self) -> Option<RegId> {
        self.stack.pop()
    }
    
    pub fn peek(&self) -> Option<RegId> {
        self.stack.last().copied()
    }
    
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}