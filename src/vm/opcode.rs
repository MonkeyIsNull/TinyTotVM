use crate::vm::value::Value;

pub type ProcId = u64;

#[derive(Debug, Clone)]
pub enum MessagePattern {
    Any,                           // matches any message
    Value(Value),                  // matches specific value
    Signal(String),                // matches specific signal
    Exit(Option<ProcId>),          // matches exit from specific PID or any
    Down(Option<ProcId>, Option<String>), // matches down message
    Link(Option<ProcId>),          // matches link message
    Type(String),                  // matches message type (e.g., "int", "string")
    Guard(String),                 // guard condition (variable name to check)
}

#[derive(Debug, Clone)]
pub enum OpCode {
    PushInt(i64),
    PushFloat(f64),
    PushStr(String),
    PushBool(bool),
    Add,
    AddF,
    Sub,
    SubF,
    Mul,
    MulF,
    Div,
    DivF,
    Concat,
    Print,
    Halt,
    Jmp(usize),
    Jz(usize),
    Call { addr: usize, params: Vec<String> },
    Ret,
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
    EqF,
    NeF,
    GtF,
    LtF,
    GeF,
    LeF,
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
    // Enhanced I/O operations
    ReadLine,       // Read line from stdin
    ReadChar,       // Read single character from stdin  
    ReadInput,      // Read until EOF from stdin
    AppendFile,     // Append to file
    FileExists,     // Check if file exists
    FileSize,       // Get file size
    DeleteFile,     // Delete file
    ListDir,        // List directory contents
    ReadBytes,      // Read file as byte array
    WriteBytes,     // Write byte array to file
    // Environment and system
    GetEnv,         // Read environment variable
    SetEnv,         // Set environment variable
    GetArgs,        // Get command line arguments
    Exec,           // Execute external command
    ExecCapture,    // Execute and capture output
    Exit,           // Exit with status code
    // Time operations
    GetTime,        // Get current timestamp
    Sleep,          // Sleep for specified duration
    FormatTime,     // Format timestamp
    // Network operations
    HttpGet,        // HTTP GET request
    HttpPost,       // HTTP POST request
    TcpConnect,     // Connect to TCP server
    TcpListen,      // Listen on TCP port
    TcpSend,        // Send data over TCP
    TcpRecv,        // Receive data from TCP
    UdpBind,        // Bind UDP socket
    UdpSend,        // Send UDP packet
    UdpRecv,        // Receive UDP packet
    DnsResolve,     // Resolve hostname to IP
    // Advanced I/O operations
    AsyncRead,      // Asynchronous file read
    AsyncWrite,     // Asynchronous file write
    Await,          // Wait for async operation
    StreamCreate,   // Create data stream
    StreamRead,     // Read from stream
    StreamWrite,    // Write to stream
    StreamClose,    // Close stream
    JsonParse,      // Parse JSON string
    JsonStringify,  // Convert to JSON string
    CsvParse,       // Parse CSV data
    CsvWrite,       // Write CSV data
    Compress,       // Compress data
    Decompress,     // Decompress data
    Encrypt,        // Encrypt data
    Decrypt,        // Decrypt data
    Hash,           // Generate hash
    DbConnect,      // Connect to database
    DbQuery,        // Execute database query
    DbExec,         // Execute database command
    // Object operations
    MakeObject,
    SetField(String),   // field name
    GetField(String),   // field name
    HasField(String),   // field name
    DeleteField(String), // field name
    Keys,              // get all keys as a list
    // Function operations
    MakeFunction { addr: usize, params: Vec<String> }, // create function pointer
    CallFunction,      // call function from stack
    // Closure and lambda operations
    MakeLambda { addr: usize, params: Vec<String> },   // create lambda/closure
    Capture(String),   // capture variable for closure
    // Exception handling
    Try { catch_addr: usize },  // start try block, jump to catch_addr on exception
    Catch,             // start catch block (exception is on stack)
    Throw,             // throw exception from stack
    EndTry,            // end try block
    // Module system
    Import(String),    // import module by path
    Export(String),    // export variable/function by name
    // Concurrency operations
    Spawn,             // spawn new process from function on stack
    Receive,           // receive message from mailbox
    ReceiveMatch(Vec<MessagePattern>), // selective receive with pattern matching
    Yield,             // yield control to scheduler
    Send(ProcId),      // send message to process
    Monitor(ProcId),   // monitor a process
    Demonitor(String), // stop monitoring (using monitor reference)
    Link(ProcId),      // link to a process
    Unlink(ProcId),    // unlink from a process
    TrapExit,          // set trap_exit flag from stack
    // Process registry operations
    Register(String),  // register current process with a name
    Unregister(String), // unregister a name
    Whereis(String),   // find PID by name (returns 0 if not found)
    SendNamed(String), // send message to named process
    // Supervision operations
    StartSupervisor, // start a supervisor process
    SuperviseChild(String), // supervise a child process with restart strategy
    RestartChild(String), // restart a specific child process
}