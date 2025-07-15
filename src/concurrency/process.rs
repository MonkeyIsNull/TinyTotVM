use std::collections::{HashMap, HashSet};
use std::time::Instant;
use std::sync::Arc;
use crossbeam::channel::{Receiver, Sender};
use colored::*;

use crate::vm::{VMError, VMResult, Value, OpCode, ProcId, MessagePattern, ExceptionHandler};
use crate::gc::{GcEngine, MarkSweepGc};
use crate::profiling::Profiler;
use crate::concurrency::{Message, SupervisorSpec, ChildSpec, ChildState, RestartPolicy};
use crate::ProcState;
use crate::bytecode::parse_program;

// Trait for sending messages between processes
pub trait MessageSender: Send + Sync + std::fmt::Debug {
    fn send_message(&self, target_pid: ProcId, message: Message) -> Result<(), String>;
}

// Trait for spawning new processes
pub trait ProcessSpawner: Send + Sync + std::fmt::Debug {
    fn spawn_process(&self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>);
}

// Trait for name registry operations
pub trait NameRegistry: Send + Sync + std::fmt::Debug {
    fn register_name(&self, name: String, pid: ProcId) -> Result<(), String>;
    fn unregister_name(&self, name: &str) -> Result<(), String>;
    fn whereis(&self, name: &str) -> Option<ProcId>;
    fn send_to_named(&self, name: &str, message: Message) -> Result<(), String>;
}

#[derive(Debug)]
pub struct TinyProc {
    pub id: ProcId,
    pub state: ProcState,
    pub mailbox: Receiver<Message>,
    pub mailbox_sender: Sender<Message>,
    pub reduction_count: usize,
    pub max_reductions: usize,
    pub message_sender: Option<Arc<dyn MessageSender>>,
    pub process_spawner: Option<Arc<dyn ProcessSpawner>>,
    pub name_registry: Option<Arc<dyn NameRegistry>>,
    pub waiting_for_message: bool,
    pub monitors: HashMap<String, ProcId>, // monitor_ref -> monitored_pid
    pub monitored_by: HashMap<ProcId, String>, // monitoring_pid -> monitor_ref
    pub linked_processes: HashSet<ProcId>, // bidirectional links
    pub exit_reason: Option<String>, // reason for exit
    pub trap_exit: bool, // whether process traps exit signals
    // Supervision data
    pub supervisor_spec: Option<SupervisorSpec>,
    pub supervised_children: HashMap<String, ChildState>, // child_id -> child_state
    pub supervisor_pid: Option<ProcId>, // parent supervisor
    pub restart_intensity_count: usize, // current restart count in period
    pub restart_period_start: Instant, // when current period started
    // VM state (isolated per process)
    pub stack: Vec<Value>,
    pub instructions: Vec<OpCode>,
    pub ip: usize,
    pub call_stack: Vec<usize>,
    pub variables: Vec<HashMap<String, Value>>,
    pub try_stack: Vec<ExceptionHandler>,
    pub exports: HashMap<String, Value>,
    pub loaded_modules: HashMap<String, HashMap<String, Value>>,
    pub loading_stack: Vec<String>,
    pub lambda_captures: HashMap<String, Value>,
    pub max_stack_size: usize,
    pub instruction_count: usize,
    pub debug_mode: bool,
    pub breakpoints: Vec<usize>,
    pub gc_engine: Box<dyn GcEngine>,
    pub _gc_stats_enabled: bool,
    pub profiler: Option<Profiler>,
    pub trace_enabled: bool,
}

impl TinyProc {
    pub fn new(id: ProcId, instructions: Vec<OpCode>) -> (Self, Sender<Message>) {
        let (sender, receiver) = crossbeam::channel::unbounded();
        let gc_engine = Box::new(MarkSweepGc::new(false));
        
        let proc = TinyProc {
            id,
            state: ProcState::Ready,
            mailbox: receiver,
            mailbox_sender: sender.clone(),
            reduction_count: 0,
            max_reductions: 1000, // Default reduction limit
            message_sender: None, // Will be set by scheduler
            process_spawner: None, // Will be set by scheduler
            name_registry: None, // Will be set by scheduler
            waiting_for_message: false,
            monitors: HashMap::new(),
            monitored_by: HashMap::new(),
            linked_processes: HashSet::new(),
            exit_reason: None,
            trap_exit: false,
            // Initialize supervision data
            supervisor_spec: None,
            supervised_children: HashMap::new(),
            supervisor_pid: None,
            restart_intensity_count: 0,
            restart_period_start: Instant::now(),
            
            // Initialize VM state
            stack: Vec::new(),
            instructions,
            ip: 0,
            call_stack: Vec::new(),
            variables: vec![HashMap::new()], // Initial global scope
            try_stack: Vec::new(),
            exports: HashMap::new(),
            loaded_modules: HashMap::new(),
            loading_stack: Vec::new(),
            lambda_captures: HashMap::new(),
            max_stack_size: 0,
            instruction_count: 0,
            debug_mode: false,
            breakpoints: Vec::new(),
            gc_engine,
            _gc_stats_enabled: false,
            profiler: None,
            trace_enabled: false,
        };
        
        (proc, sender)
    }
    
    pub fn new_supervisor(id: ProcId, spec: SupervisorSpec) -> (Self, Sender<Message>) {
        let (sender, receiver) = crossbeam::channel::unbounded();
        let gc_engine = Box::new(MarkSweepGc::new(false));
        
        // Create supervisor loop instructions
        let supervisor_instructions = vec![
            OpCode::Receive,
            OpCode::Yield,
        ];
        
        let proc = TinyProc {
            id,
            state: ProcState::Ready,
            mailbox: receiver,
            mailbox_sender: sender.clone(),
            reduction_count: 0,
            max_reductions: 1000,
            message_sender: None,
            process_spawner: None,
            name_registry: None,
            waiting_for_message: false,
            monitors: HashMap::new(),
            monitored_by: HashMap::new(),
            linked_processes: HashSet::new(),
            exit_reason: None,
            trap_exit: true, // Supervisors trap exits by default
            // Initialize supervision data
            supervisor_spec: Some(spec),
            supervised_children: HashMap::new(),
            supervisor_pid: None,
            restart_intensity_count: 0,
            restart_period_start: Instant::now(),
            
            // Initialize VM state
            stack: Vec::new(),
            instructions: supervisor_instructions,
            ip: 0,
            call_stack: Vec::new(),
            variables: vec![HashMap::new()],
            try_stack: Vec::new(),
            exports: HashMap::new(),
            loaded_modules: HashMap::new(),
            loading_stack: Vec::new(),
            lambda_captures: HashMap::new(),
            max_stack_size: 0,
            instruction_count: 0,
            debug_mode: false,
            breakpoints: Vec::new(),
            gc_engine,
            _gc_stats_enabled: false,
            profiler: None,
            trace_enabled: false,
        };
        
        (proc, sender)
    }
    
    pub fn send_message(&self, message: Message) -> Result<(), crossbeam::channel::SendError<Message>> {
        self.mailbox_sender.send(message)
    }
    
    pub fn receive_message(&self) -> Result<Message, crossbeam::channel::TryRecvError> {
        self.mailbox.try_recv()
    }
    
    pub fn receive_message_blocking(&self) -> Result<Message, crossbeam::channel::RecvError> {
        self.mailbox.recv()
    }
    
    pub fn has_messages(&self) -> bool {
        !self.mailbox.is_empty()
    }
    
    pub fn monitor_process(&mut self, target_pid: ProcId) -> String {
        let monitor_ref = format!("mon_{}_{}", self.id, target_pid);
        self.monitors.insert(monitor_ref.clone(), target_pid);
        monitor_ref
    }
    
    pub fn demonitor_process(&mut self, monitor_ref: &str) -> Option<ProcId> {
        self.monitors.remove(monitor_ref)
    }
    
    pub fn link_process(&mut self, target_pid: ProcId) {
        self.linked_processes.insert(target_pid);
    }
    
    pub fn unlink_process(&mut self, target_pid: ProcId) {
        self.linked_processes.remove(&target_pid);
    }
    
    pub fn add_monitor(&mut self, monitoring_pid: ProcId, monitor_ref: String) {
        self.monitored_by.insert(monitoring_pid, monitor_ref);
    }
    
    pub fn remove_monitor(&mut self, monitoring_pid: ProcId) -> Option<String> {
        self.monitored_by.remove(&monitoring_pid)
    }
    
    pub fn set_exit_reason(&mut self, reason: String) {
        self.exit_reason = Some(reason);
    }
    
    // Handle process exit by sending appropriate signals and cleanup
    pub fn handle_process_exit(&mut self, reason: String) {
        println!("Process {} exiting with reason: {}", self.id, reason);
        self.set_exit_reason(reason.clone());
        self.state = ProcState::Exited;
        
        // Send down messages to all monitors
        for (monitor_ref, monitored_pid) in self.monitors.iter() {
            if let Some(sender) = &self.message_sender {
                let down_msg = Message::Down(self.id, monitor_ref.clone(), reason.clone());
                println!("Sending down message to monitor {}: {:?}", monitored_pid, down_msg);
                let _ = sender.send_message(*monitored_pid, down_msg);
            }
        }
        
        // Send down messages to all processes monitoring this one
        for (monitoring_pid, monitor_ref) in self.monitored_by.iter() {
            if let Some(sender) = &self.message_sender {
                let down_msg = Message::Down(self.id, monitor_ref.clone(), reason.clone());
                println!("Sending down message to monitoring process {}: {:?}", monitoring_pid, down_msg);
                let _ = sender.send_message(*monitoring_pid, down_msg);
            }
        }
        
        // Send exit signals to all linked processes
        for linked_pid in self.linked_processes.iter() {
            if let Some(sender) = &self.message_sender {
                let exit_msg = Message::Exit(self.id);
                println!("Sending exit signal to linked process {}: {:?}", linked_pid, exit_msg);
                let _ = sender.send_message(*linked_pid, exit_msg);
            }
        }
        
        // Notify supervisor of child exit (if this process has a supervisor)
        if let Some(supervisor_pid) = self.supervisor_pid {
            if let Some(sender) = &self.message_sender {
                // Find the child name for this process
                // In a real implementation, this would be tracked properly
                let child_exit_msg = Message::Signal(format!("child_exit_{}_{}", self.id, reason));
                println!("Notifying supervisor {} of child {} exit: {}", supervisor_pid, self.id, reason);
                let _ = sender.send_message(supervisor_pid, child_exit_msg);
            }
        }
    }
    
    pub fn has_reductions_left(&self) -> bool {
        self.reduction_count < self.max_reductions
    }
    
    pub fn increment_reductions(&mut self) {
        self.reduction_count += 1;
    }
    
    pub fn reset_reductions(&mut self) {
        self.reduction_count = 0;
    }
    
    // Supervision helper methods
    fn can_restart_supervisor(&mut self) -> bool {
        if let Some(spec) = &self.supervisor_spec {
            let now = Instant::now();
            
            // If period has passed, reset intensity count
            if now.duration_since(self.restart_period_start) > spec.period {
                self.restart_intensity_count = 0;
                self.restart_period_start = now;
                return true;
            }
            
            // Check if we've exceeded the intensity limit
            self.restart_intensity_count < spec.intensity
        } else {
            false
        }
    }
    
    fn record_restart_supervisor(&mut self) {
        self.restart_intensity_count += 1;
    }
    
    fn should_restart_child(&self, child_spec: &ChildSpec, exit_reason: &str) -> bool {
        match child_spec.restart {
            RestartPolicy::Permanent => true,
            RestartPolicy::Temporary => false,
            RestartPolicy::Transient => exit_reason != "normal",
        }
    }
    
    fn start_child(&mut self, child_spec: &ChildSpec) -> Result<ProcId, String> {
        if let Some(spawner) = &self.process_spawner {
            let (child_pid, _) = spawner.spawn_process(child_spec.instructions.clone());
            
            let child_state = ChildState {
                pid: child_pid,
                spec: child_spec.clone(),
                restart_count: 0,
                last_restart: Instant::now(),
            };
            
            self.supervised_children.insert(child_spec.id.clone(), child_state);
            Ok(child_pid)
        } else {
            Err("No process spawner available".to_string())
        }
    }
    
    pub fn start_all_children(&mut self) -> Result<(), String> {
        if let Some(spec) = self.supervisor_spec.clone() {
            for child_spec in &spec.children {
                self.start_child(child_spec)?;
            }
        }
        Ok(())
    }
    
    // Pattern matching helper methods
    fn matches_pattern(&self, message: &Message, pattern: &MessagePattern) -> bool {
        match (message, pattern) {
            (_, MessagePattern::Any) => true,
            (Message::Value(val), MessagePattern::Value(pattern_val)) => val == pattern_val,
            (Message::Signal(sig), MessagePattern::Signal(pattern_sig)) => sig == pattern_sig,
            (Message::Exit(pid), MessagePattern::Exit(pattern_pid)) => {
                pattern_pid.is_none() || pattern_pid == &Some(*pid)
            }
            (Message::Down(pid, _ref, _reason), MessagePattern::Down(pattern_pid, _pattern_ref)) => {
                pattern_pid.is_none() || pattern_pid == &Some(*pid)
            }
            (Message::Link(pid), MessagePattern::Link(pattern_pid)) => {
                pattern_pid.is_none() || pattern_pid == &Some(*pid)
            }
            (Message::Value(val), MessagePattern::Type(type_name)) => {
                match (val, type_name.as_str()) {
                    (Value::Int(_), "int") => true,
                    (Value::Float(_), "float") => true,
                    (Value::Str(_), "string") => true,
                    (Value::Bool(_), "bool") => true,
                    (Value::List(_), "list") => true,
                    (Value::Object(_), "object") => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }
    
    fn selective_receive(&mut self, patterns: &[MessagePattern]) -> Result<Option<Message>, String> {
        // Create a temporary vector to store messages that don't match
        let mut temp_messages = Vec::new();
        
        // Try to find a matching message
        let mut found_message = None;
        
        // Check all available messages
        while let Ok(msg) = self.receive_message() {
            let mut matched = false;
            
            for pattern in patterns {
                if self.matches_pattern(&msg, pattern) {
                    found_message = Some(msg.clone());
                    matched = true;
                    break;
                }
            }
            
            if matched {
                break;
            } else {
                temp_messages.push(msg);
            }
        }
        
        // Put back non-matching messages in original order
        for msg in temp_messages.into_iter().rev() {
            let _ = self.mailbox_sender.try_send(msg);
        }
        
        Ok(found_message)
    }
    

    pub fn step(&mut self) -> VMResult<bool> {
        if self.ip >= self.instructions.len() {
            self.handle_process_exit("normal".to_string());
            return Ok(false); // Process is done
        }
        
        if !self.has_reductions_left() {
            self.state = ProcState::Waiting;
            return Ok(false); // Out of reductions, yield
        }
        
        // Check for exit signals at the beginning of each instruction cycle
        // Process all available messages to handle exit signals immediately
        let mut exit_signal_received = false;
        
        while let Ok(msg) = self.receive_message() {
            match msg {
                Message::Exit(pid) => {
                    // Handle exit signal from linked process
                    println!("Process {} received exit signal from process {}", self.id, pid);
                    if self.linked_processes.contains(&pid) {
                        if self.trap_exit {
                            // Process traps exits - convert to regular message
                            println!("Process {} traps exits, converting exit signal to message", self.id);
                            let _ = self.mailbox_sender.send(Message::Exit(pid));
                        } else {
                            println!("Process {} is linked to {}, exiting due to exit signal", self.id, pid);
                            // In BEAM, linked processes normally exit when receiving exit signals
                            self.handle_process_exit(format!("exit_from_{}", pid));
                            exit_signal_received = true;
                            break;
                        }
                    } else {
                        println!("Process {} not linked to {}, discarding exit signal", self.id, pid);
                        // Not linked, just discard the message
                    }
                }
                Message::TrapExit(trap) => {
                    // Handle trap_exit setting
                    println!("Process {} setting trap_exit to {}", self.id, trap);
                    self.trap_exit = trap;
                }
                Message::Link(pid) => {
                    // Handle link request automatically - bidirectional linking
                    println!("Process {} received link request from process {}", self.id, pid);
                    let was_already_linked = self.linked_processes.contains(&pid);
                    println!("Process {} was already linked to {}: {}", self.id, pid, was_already_linked);
                    self.link_process(pid);
                    
                    // Send back link confirmation to make it bidirectional
                    // Only send if we weren't already linked to prevent infinite loop
                    if !was_already_linked {
                        if let Some(sender) = &self.message_sender {
                            let link_back_msg = Message::Link(self.id);
                            println!("Process {} sending link back message to process {}", self.id, pid);
                            let _ = sender.send_message(pid, link_back_msg);
                        }
                    }
                }
                Message::Unlink(pid) => {
                    // Handle unlink request automatically
                    println!("Process {} received unlink request from process {}", self.id, pid);
                    self.unlink_process(pid);
                }
                Message::Monitor(pid, monitor_ref) => {
                    // Handle monitor request automatically
                    println!("Process {} received monitor request from process {} with ref {}", self.id, pid, monitor_ref);
                    self.add_monitor(pid, monitor_ref);
                }
                Message::Down(pid, monitor_ref, reason) => {
                    // Handle down message automatically for immediate delivery
                    println!("Process {} received down message: pid={}, ref={}, reason={}", self.id, pid, monitor_ref, reason);
                    // Put it back in the queue for the Receive instruction to pick up
                    let _ = self.mailbox_sender.send(Message::Down(pid, monitor_ref, reason));
                }
                _ => {
                    // Other messages go back to queue for Receive to handle
                    let _ = self.mailbox_sender.send(msg);
                }
            }
        }
        
        if exit_signal_received {
            return Ok(false);
        }
        
        let instruction = &self.instructions[self.ip].clone();
        self.increment_reductions();
        self.instruction_count += 1;
        
        if self.trace_enabled {
            let indent = "  ".repeat(self.call_stack.len());
            println!("{} {}{} @ {}", 
                     "[trace]".bright_blue(),
                     indent, 
                     format!("{:?}", instruction).white(),
                     format!("0x{:04X}", self.ip).cyan());
        }
        
        self.execute_instruction_safe(instruction)?;
        
        // Check if process state changed during instruction execution
        match self.state {
            ProcState::Exited => Ok(false),  // Process is done
            ProcState::Waiting => Ok(false), // Process yielded
            _ => Ok(true), // Continue execution
        }
    }
    
    pub fn run_until_yield(&mut self) -> VMResult<ProcState> {
        self.state = ProcState::Running;
        self.reset_reductions();
        
        loop {
            match self.step()? {
                true => continue,  // Keep running
                false => break,    // Yielded or exited
            }
        }
        
        Ok(self.state)
    }
    
    fn pop_stack(&mut self, operation: &str) -> VMResult<Value> {
        self.stack.pop().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }

    fn peek_stack(&self, operation: &str) -> VMResult<&Value> {
        self.stack.last().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }

    fn check_stack_size(&self, needed: usize, _operation: &str) -> VMResult<()> {
        if self.stack.len() < needed {
            return Err(VMError::StackUnderflow(_operation.to_string()));
        }
        Ok(())
    }
    
    fn execute_instruction_safe(&mut self, instruction: &OpCode) -> VMResult<()> {
        // For now, implement a simplified version that handles basic operations
        // This can be expanded later as needed
        match instruction {
            OpCode::PushInt(n) => self.stack.push(Value::Int(*n)),
            OpCode::PushFloat(f) => self.stack.push(Value::Float(*f)),
            OpCode::PushStr(s) => self.stack.push(Value::Str(s.clone())),
            OpCode::PushBool(b) => self.stack.push(Value::Bool(*b)),
            OpCode::Print => {
                let val = self.pop_stack("PRINT")?;
                println!("{}", val);
            }
            OpCode::Add => {
                let b = self.pop_stack("ADD")?;
                let a = self.pop_stack("ADD")?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x + y)),
                    (Value::Int(x), Value::Float(y)) => self.stack.push(Value::Float(*x as f64 + y)),
                    (Value::Float(x), Value::Int(y)) => self.stack.push(Value::Float(x + *y as f64)),
                    (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x + y)),
                    _ => return Err(VMError::TypeMismatch { 
                        expected: "two numbers (int or float)".to_string(), 
                        got: format!("{:?}, {:?}", a, b), 
                        operation: "ADD".to_string() 
                    }),
                }
            }
            OpCode::Mul => {
                let b = self.pop_stack("MUL")?;
                let a = self.pop_stack("MUL")?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x * y)),
                    (Value::Int(x), Value::Float(y)) => self.stack.push(Value::Float(*x as f64 * y)),
                    (Value::Float(x), Value::Int(y)) => self.stack.push(Value::Float(x * *y as f64)),
                    (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x * y)),
                    _ => return Err(VMError::TypeMismatch { 
                        expected: "two numbers (int or float)".to_string(), 
                        got: format!("{:?}, {:?}", a, b), 
                        operation: "MUL".to_string() 
                    }),
                }
            }
            OpCode::Div => {
                let b = self.pop_stack("DIV")?;
                let a = self.pop_stack("DIV")?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => {
                        if *y == 0 {
                            return Err(VMError::DivisionByZero);
                        }
                        self.stack.push(Value::Int(x / y));
                    },
                    (Value::Int(x), Value::Float(y)) => {
                        if *y == 0.0 {
                            return Err(VMError::DivisionByZero);
                        }
                        self.stack.push(Value::Float(*x as f64 / y));
                    },
                    (Value::Float(x), Value::Int(y)) => {
                        if *y == 0 {
                            return Err(VMError::DivisionByZero);
                        }
                        self.stack.push(Value::Float(x / *y as f64));
                    },
                    (Value::Float(x), Value::Float(y)) => {
                        if *y == 0.0 {
                            return Err(VMError::DivisionByZero);
                        }
                        self.stack.push(Value::Float(x / y));
                    },
                    _ => return Err(VMError::TypeMismatch { 
                        expected: "two numbers (int or float)".to_string(), 
                        got: format!("{:?}, {:?}", a, b), 
                        operation: "DIV".to_string() 
                    }),
                }
            }
            OpCode::Halt => {
                self.handle_process_exit("normal".to_string());
                // Don't advance IP for Halt - process is done
                return Ok(());
            }
            OpCode::Spawn => {
                // For now, spawn a simple process with basic instructions
                // In a real implementation, we'd parse the function from the stack
                let function_value = self.pop_stack("SPAWN")?;
                
                // Try to load the function as a ttvm file
                let new_process_instructions = match function_value {
                    Value::Str(ref s) => {
                        // Try to load from examples directory first
                        let mut file_path = format!("examples/{}.ttvm", s);
                        if !std::path::Path::new(&file_path).exists() {
                            // Try current directory
                            file_path = format!("{}.ttvm", s);
                        }
                        
                        if std::path::Path::new(&file_path).exists() {
                            // Load and parse the ttvm file
                            match parse_program(&file_path) {
                                Ok(instructions) => instructions,
                                Err(e) => {
                                    eprintln!("Failed to parse {}: {}", file_path, e);
                                    vec![
                                        OpCode::PushStr(format!("Failed to load {}", s)),
                                        OpCode::Print,
                                        OpCode::Halt,
                                    ]
                                }
                            }
                        } else {
                            // Fallback to hardcoded processes for backward compatibility
                            match s.as_str() {
                                "hello_world" => {
                                    vec![
                                        OpCode::PushStr("Hello from spawned process!".to_string()),
                                        OpCode::Print,
                                        OpCode::Halt,
                                    ]
                                }
                                "counter" => {
                                    vec![
                                        OpCode::PushInt(1),
                                        OpCode::Print,
                                        OpCode::PushInt(2),
                                        OpCode::Print,
                                        OpCode::PushInt(3),
                                        OpCode::Print,
                                        OpCode::Halt,
                                    ]
                                }
                                _ => {
                                    // Default: spawn a simple process
                                    vec![
                                        OpCode::PushStr(format!("Spawned process: {}", s)),
                                        OpCode::Print,
                                        OpCode::Halt,
                                    ]
                                }
                            }
                        }
                    }
                    _ => {
                        // Non-string values default to simple process
                        vec![
                            OpCode::PushStr("Spawned process".to_string()),
                            OpCode::Print,
                            OpCode::Halt,
                        ]
                    }
                };
                
                // Spawn the new process
                if let Some(spawner) = &self.process_spawner {
                    let (new_proc_id, _sender) = spawner.spawn_process(new_process_instructions);
                    self.stack.push(Value::Int(new_proc_id as i64));
                } else {
                    eprintln!("No process spawner available for process {}", self.id);
                    self.stack.push(Value::Int(0)); // Push dummy process ID
                }
            }
            OpCode::Receive => {
                // Try to receive a message from mailbox
                match self.receive_message() {
                    Ok(msg) => {
                        // Message received successfully
                        self.waiting_for_message = false;
                        match msg {
                            Message::Value(val) => self.stack.push(val),
                            Message::Signal(sig) => self.stack.push(Value::Str(sig)),
                            Message::Exit(pid) => {
                                // Handle exit signal from linked process
                                if self.linked_processes.contains(&pid) {
                                    // In BEAM, linked processes normally exit when receiving exit signals
                                    // For now, we'll implement basic exit propagation
                                    self.handle_process_exit(format!("exit_from_{}", pid));
                                    return Ok(());
                                } else {
                                    // Not linked, just treat as regular message
                                    self.stack.push(Value::Int(pid as i64));
                                }
                            }
                            Message::Monitor(pid, monitor_ref) => {
                                // Handle monitor request
                                self.add_monitor(pid, monitor_ref.clone());
                                self.stack.push(Value::Str(format!("monitor:{}", monitor_ref)));
                            }
                            Message::Down(pid, monitor_ref, reason) => {
                                // Handle down message
                                self.stack.push(Value::Str(format!("down:{}:{}:{}", pid, monitor_ref, reason)));
                            }
                            Message::Link(pid) => {
                                // Handle link request - bidirectional linking
                                println!("Process {} received link request from process {}", self.id, pid);
                                let was_already_linked = self.linked_processes.contains(&pid);
                                println!("Process {} was already linked to {}: {}", self.id, pid, was_already_linked);
                                self.link_process(pid);
                                
                                // Send back link confirmation to make it bidirectional
                                // Only send if we weren't already linked to prevent infinite loop
                                if !was_already_linked {
                                    if let Some(sender) = &self.message_sender {
                                        let link_back_msg = Message::Link(self.id);
                                        println!("Process {} sending link back message to process {}", self.id, pid);
                                        let _ = sender.send_message(pid, link_back_msg);
                                    }
                                }
                                
                                self.stack.push(Value::Str(format!("linked:{}", pid)));
                            }
                            Message::Unlink(pid) => {
                                // Handle unlink request
                                self.unlink_process(pid);
                                self.stack.push(Value::Str(format!("unlinked:{}", pid)));
                            }
                            Message::TrapExit(trap) => {
                                // Handle trap_exit setting
                                self.trap_exit = trap;
                                self.stack.push(Value::Bool(trap));
                            }
                        }
                    }
                    Err(_) => {
                        // No message available, mark as waiting and yield
                        self.waiting_for_message = true;
                        self.state = ProcState::Waiting;
                        // Don't advance IP - we want to retry this instruction when rescheduled
                        return Ok(());
                    }
                }
            }
            OpCode::ReceiveMatch(patterns) => {
                // Try selective receive with pattern matching
                match self.selective_receive(patterns) {
                    Ok(Some(msg)) => {
                        // Message received successfully
                        self.waiting_for_message = false;
                        match msg {
                            Message::Value(val) => self.stack.push(val),
                            Message::Signal(sig) => self.stack.push(Value::Str(sig)),
                            Message::Exit(pid) => {
                                if self.trap_exit {
                                    // Process traps exits - put exit message on stack
                                    self.stack.push(Value::Str(format!("exit:{}", pid)));
                                } else if self.linked_processes.contains(&pid) {
                                    // In BEAM, linked processes normally exit when receiving exit signals
                                    self.handle_process_exit(format!("exit_from_{}", pid));
                                    return Ok(());
                                } else {
                                    // Not linked, treat as regular message
                                    self.stack.push(Value::Int(pid as i64));
                                }
                            }
                            Message::Monitor(pid, monitor_ref) => {
                                // Handle monitor request
                                self.add_monitor(pid, monitor_ref.clone());
                                self.stack.push(Value::Str(format!("monitor:{}", monitor_ref)));
                            }
                            Message::Down(pid, monitor_ref, reason) => {
                                // Handle down message
                                self.stack.push(Value::Str(format!("down:{}:{}:{}", pid, monitor_ref, reason)));
                            }
                            Message::Link(pid) => {
                                // Handle link request - bidirectional linking
                                println!("Process {} received link request from process {}", self.id, pid);
                                let was_already_linked = self.linked_processes.contains(&pid);
                                self.link_process(pid);
                                
                                // Send back link confirmation to make it bidirectional
                                if !was_already_linked {
                                    if let Some(sender) = &self.message_sender {
                                        let link_back_msg = Message::Link(self.id);
                                        let _ = sender.send_message(pid, link_back_msg);
                                    }
                                }
                                
                                self.stack.push(Value::Str(format!("linked:{}", pid)));
                            }
                            Message::Unlink(pid) => {
                                // Handle unlink request
                                self.unlink_process(pid);
                                self.stack.push(Value::Str(format!("unlinked:{}", pid)));
                            }
                            Message::TrapExit(trap) => {
                                // Handle trap_exit setting
                                self.trap_exit = trap;
                                self.stack.push(Value::Bool(trap));
                            }
                        }
                    }
                    Ok(None) => {
                        // No matching message available, mark as waiting and yield
                        self.waiting_for_message = true;
                        self.state = ProcState::Waiting;
                        // Don't advance IP - we want to retry this instruction when rescheduled
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(VMError::RuntimeError(format!("Selective receive error: {}", e)));
                    }
                }
            }
            OpCode::Yield => {
                // Manually yield to scheduler
                self.ip += 1; // Advance IP before yielding
                self.state = ProcState::Waiting;
                return Ok(());
            }
            OpCode::Send(target_proc_id) => {
                // Get the message value from the stack
                let message_value = self.pop_stack("SEND")?;
                
                // Convert Value to Message
                let message = Message::Value(message_value);
                
                // Send message using the message sender
                if let Some(sender) = &self.message_sender {
                    match sender.send_message(*target_proc_id, message) {
                        Ok(_) => {
                            // Message sent successfully
                            println!("Process {} sent message to process {}", self.id, target_proc_id);
                        }
                        Err(e) => {
                            eprintln!("Failed to send message to process {}: {}", target_proc_id, e);
                            // In a real implementation, we might want to handle this error differently
                        }
                    }
                } else {
                    eprintln!("No message sender available for process {}", self.id);
                }
            }
            OpCode::Monitor(target_proc_id) => {
                // Monitor a process - add it to our monitors list
                let monitor_ref = self.monitor_process(*target_proc_id);
                
                // Send monitor request to the scheduler
                if let Some(sender) = &self.message_sender {
                    let monitor_msg = Message::Monitor(self.id, monitor_ref.clone());
                    println!("Process {} sending monitor request to process {}", self.id, target_proc_id);
                    match sender.send_message(*target_proc_id, monitor_msg) {
                        Ok(_) => {
                            // Push monitor reference to stack for use by the process
                            println!("Process {} successfully sent monitor request to process {}", self.id, target_proc_id);
                            self.stack.push(Value::Str(monitor_ref));
                        }
                        Err(e) => {
                            eprintln!("Failed to send monitor request to process {}: {}", target_proc_id, e);
                            // Remove from monitors if sending failed
                            self.demonitor_process(&monitor_ref);
                            self.stack.push(Value::Str("monitor_failed".to_string()));
                        }
                    }
                } else {
                    eprintln!("No message sender available for process {}", self.id);
                    self.stack.push(Value::Str("monitor_failed".to_string()));
                }
            }
            OpCode::Demonitor(monitor_ref) => {
                // Stop monitoring a process
                if let Some(target_proc_id) = self.demonitor_process(monitor_ref) {
                    // Send demonitor request to the target process
                    if let Some(sender) = &self.message_sender {
                        let demonitor_msg = Message::Monitor(self.id, format!("stop_{}", monitor_ref));
                        match sender.send_message(target_proc_id, demonitor_msg) {
                            Ok(_) => {
                                self.stack.push(Value::Str("demonitor_success".to_string()));
                            }
                            Err(e) => {
                                eprintln!("Failed to send demonitor request to process {}: {}", target_proc_id, e);
                                self.stack.push(Value::Str("demonitor_failed".to_string()));
                            }
                        }
                    } else {
                        eprintln!("No message sender available for process {}", self.id);
                        self.stack.push(Value::Str("demonitor_failed".to_string()));
                    }
                } else {
                    // Monitor reference not found
                    self.stack.push(Value::Str("monitor_not_found".to_string()));
                }
            }
            OpCode::Link(target_proc_id) => {
                // Link to a process - bidirectional link
                println!("Process {} linking to process {}", self.id, target_proc_id);
                self.link_process(*target_proc_id);
                
                // Send link request to the target process
                if let Some(sender) = &self.message_sender {
                    let link_msg = Message::Link(self.id);
                    println!("Process {} sending link message to process {}", self.id, target_proc_id);
                    match sender.send_message(*target_proc_id, link_msg) {
                        Ok(_) => {
                            println!("Process {} successfully sent link message to process {}", self.id, target_proc_id);
                            self.stack.push(Value::Str(format!("linked_{}", target_proc_id)));
                        }
                        Err(e) => {
                            eprintln!("Failed to send link request to process {}: {}", target_proc_id, e);
                            // Remove link if sending failed
                            self.unlink_process(*target_proc_id);
                            self.stack.push(Value::Str("link_failed".to_string()));
                        }
                    }
                } else {
                    eprintln!("No message sender available for process {}", self.id);
                    self.stack.push(Value::Str("link_failed".to_string()));
                }
            }
            OpCode::Unlink(target_proc_id) => {
                // Unlink from a process
                self.unlink_process(*target_proc_id);
                
                // Send unlink request to the target process
                if let Some(sender) = &self.message_sender {
                    let unlink_msg = Message::Unlink(self.id);
                    match sender.send_message(*target_proc_id, unlink_msg) {
                        Ok(_) => {
                            self.stack.push(Value::Str(format!("unlinked_{}", target_proc_id)));
                        }
                        Err(e) => {
                            eprintln!("Failed to send unlink request to process {}: {}", target_proc_id, e);
                            self.stack.push(Value::Str("unlink_failed".to_string()));
                        }
                    }
                } else {
                    eprintln!("No message sender available for process {}", self.id);
                    self.stack.push(Value::Str("unlink_failed".to_string()));
                }
            }
            OpCode::TrapExit => {
                // Set trap_exit flag from stack
                let trap_value = self.pop_stack("TRAP_EXIT")?;
                match trap_value {
                    Value::Bool(trap) => {
                        self.trap_exit = trap;
                        println!("Process {} set trap_exit to {}", self.id, trap);
                    }
                    _ => return Err(VMError::TypeError("TRAP_EXIT requires boolean value".to_string())),
                }
            }
            OpCode::Register(name) => {
                // Register current process with a name
                if let Some(registry) = &self.name_registry {
                    match registry.register_name(name.clone(), self.id) {
                        Ok(_) => self.stack.push(Value::Str(format!("registered_{}", name))),
                        Err(e) => self.stack.push(Value::Str(format!("register_failed_{}", e))),
                    }
                } else {
                    self.stack.push(Value::Str("register_failed_no_registry".to_string()));
                }
            }
            OpCode::Unregister(name) => {
                // Unregister a name
                if let Some(registry) = &self.name_registry {
                    match registry.unregister_name(name) {
                        Ok(_) => self.stack.push(Value::Str(format!("unregistered_{}", name))),
                        Err(e) => self.stack.push(Value::Str(format!("unregister_failed_{}", e))),
                    }
                } else {
                    self.stack.push(Value::Str("unregister_failed_no_registry".to_string()));
                }
            }
            OpCode::Whereis(name) => {
                // Find PID by name (returns 0 if not found)
                if let Some(registry) = &self.name_registry {
                    match registry.whereis(name) {
                        Some(proc_id) => self.stack.push(Value::Int(proc_id as i64)),
                        None => self.stack.push(Value::Int(0)),
                    }
                } else {
                    self.stack.push(Value::Int(0));
                }
            }
            OpCode::SendNamed(name) => {
                // Send message to named process
                let message_value = self.pop_stack("SEND_NAMED")?;
                
                if let Some(registry) = &self.name_registry {
                    let message = Message::Value(message_value);
                    match registry.send_to_named(name, message) {
                        Ok(_) => self.stack.push(Value::Str(format!("sent_to_{}", name))),
                        Err(e) => self.stack.push(Value::Str(format!("send_failed_{}", e))),
                    }
                } else {
                    self.stack.push(Value::Str("send_failed_no_registry".to_string()));
                }
            }
            OpCode::StartSupervisor => {
                // This opcode is used to start supervisor functionality
                // The actual supervisor is created with new_supervisor()
                self.stack.push(Value::Str("supervisor_started".to_string()));
            }
            OpCode::SuperviseChild(child_name) => {
                // Supervise a child process - for now just mark it as supervised
                // The actual child process would be spawned separately
                self.stack.push(Value::Str(format!("supervising_{}", child_name)));
            }
            OpCode::RestartChild(child_name) => {
                // Restart a specific child process with safety measures
                if self.supervisor_spec.is_some() {
                    if self.supervised_children.contains_key(child_name) {
                        // Check if we can restart this child (prevents infinite loops)
                        if self.can_restart_supervisor() {
                            self.record_restart_supervisor();
                            
                            // In a real implementation, this would:
                            // 1. Terminate the old child process
                            // 2. Spawn a new child process with the same instructions
                            // 3. Update the supervised_children mapping
                            self.stack.push(Value::Str(format!("restarting_{}", child_name)));
                            
                            // For now, just simulate the restart
                            eprintln!("Supervisor {} safely restarting child {}", self.id, child_name);
                        } else {
                            self.stack.push(Value::Str(format!("restart_limit_exceeded_{}", child_name)));
                            eprintln!("Supervisor {} cannot restart child {} - too many restarts", self.id, child_name);
                        }
                    } else {
                        self.stack.push(Value::Str(format!("child_not_found_{}", child_name)));
                    }
                } else {
                    self.stack.push(Value::Str("not_supervisor".to_string()));
                }
            }
            OpCode::Import(path) => {
                // Handle module imports with circular dependency detection
                if self.loading_stack.contains(path) {
                    return Err(VMError::CircularDependency(path.clone()));
                }
                
                // If module is already loaded, skip
                if self.loaded_modules.contains_key(path) {
                    // Module already loaded, nothing to do
                } else {
                    // Load the module
                    self.loading_stack.push(path.clone());
                    
                    // Load and parse the module file
                    let module_result = self.load_module(path);
                    
                    // Remove from loading stack
                    self.loading_stack.pop();
                    
                    match module_result {
                        Ok(exports) => {
                            self.loaded_modules.insert(path.clone(), exports);
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            _ => {
                // For now, just advance IP for unsupported instructions
                // TODO: Implement full instruction set
                self.ip += 1;
                return Ok(());
            }
        }
        
        self.ip += 1;
        Ok(())
    }
    
    fn load_module(&mut self, path: &str) -> VMResult<HashMap<String, Value>> {
        // Try to load the module file
        let _content = std::fs::read_to_string(path)
            .map_err(|e| VMError::FileError { 
                filename: path.to_string(), 
                error: e.to_string() 
            })?;
        
        // Parse the module
        let module_instructions = crate::bytecode::parse_program(path)
            .map_err(|e| VMError::ParseError { 
                line: 0, 
                instruction: format!("Failed to parse module {}: {}", path, e) 
            })?;
        
        // Create a new TinyProc to execute the module
        let (mut module_proc, _sender) = TinyProc::new(9999, module_instructions);
        
        // Copy the current loading stack to detect circular dependencies
        module_proc.loading_stack = self.loading_stack.clone();
        
        // Execute the module by running its instructions
        while module_proc.ip < module_proc.instructions.len() {
            let instruction = module_proc.instructions[module_proc.ip].clone();
            module_proc.execute_instruction_safe(&instruction)?;
            
            // Check for potential infinite loops
            if module_proc.instruction_count > 1000000 {
                return Err(VMError::InfiniteLoop);
            }
        }
        
        // Return the module's exports
        Ok(module_proc.exports)
    }
}