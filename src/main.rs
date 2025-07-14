use std::collections::{HashMap, HashSet};
use std::fs;
use std::fmt;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::thread;
use comfy_table::{Table, Cell, presets::UTF8_FULL, modifiers::UTF8_SOLID_INNER_BORDERS, Color, Attribute};
use colored::*;
use crossbeam::channel::{Receiver, Sender};
use crossbeam_deque::{Worker, Stealer};
mod bytecode;
mod compiler;
mod lisp_compiler;
mod optimizer;

#[derive(Clone, Copy, Debug)]
pub enum OutputMode {
    PrettyTable,
    Plain,
}

#[derive(Clone, Debug)]
pub struct VMConfig {
    pub output_mode: OutputMode,
    pub run_tests: bool,
    pub gc_debug: bool,
    pub gc_stats: bool,
    pub debug_mode: bool,
    pub optimize_mode: bool,
    pub gc_type: String,
    pub trace_enabled: bool,
    pub profile_enabled: bool,
    pub smp_enabled: bool,
    pub trace_procs: bool,
    pub profile_procs: bool,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
}

// Concurrency data structures
pub type ProcId = u64;

// Trait for sending messages to processes
pub trait MessageSender: Send + Sync + std::fmt::Debug {
    fn send_message(&self, target_proc_id: ProcId, message: Message) -> Result<(), String>;
}

// Trait for spawning new processes
pub trait ProcessSpawner: Send + Sync + std::fmt::Debug {
    fn spawn_process(&self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcState {
    Ready,
    Running,
    Waiting,
    Exited,
}

#[derive(Debug, Clone)]
pub enum Message {
    Value(Value),
    Signal(String),
    Exit(ProcId),
    Monitor(ProcId, String), // Monitor request: (target_pid, monitor_ref)
    Down(ProcId, String, String), // Down message: (monitored_pid, monitor_ref, reason)
    Link(ProcId), // Link request
    Unlink(ProcId), // Unlink request
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
    pub waiting_for_message: bool,
    pub monitors: HashMap<String, ProcId>, // monitor_ref -> monitored_pid
    pub monitored_by: HashMap<ProcId, String>, // monitoring_pid -> monitor_ref
    pub linked_processes: HashSet<ProcId>, // bidirectional links
    pub exit_reason: Option<String>, // reason for exit
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

#[derive(Debug)]
pub struct Scheduler {
    pub id: usize,
    pub local_queue: Worker<Arc<Mutex<TinyProc>>>,
    pub remote_stealers: Vec<Stealer<Arc<Mutex<TinyProc>>>>,
    pub running: bool,
}

#[derive(Debug)]
pub struct SchedulerPool {
    pub schedulers: Vec<thread::JoinHandle<()>>,
    pub global_stealers: Vec<Stealer<Arc<Mutex<TinyProc>>>>,
    pub next_proc_id: Arc<Mutex<ProcId>>,
    pub process_submission_queue: Arc<Mutex<Vec<Arc<Mutex<TinyProc>>>>>,
    pub running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>,
    pub shutdown_flag: Arc<Mutex<bool>>,
    pub process_registry: Arc<Mutex<HashMap<ProcId, Sender<Message>>>>,
}

#[derive(Debug, Clone)]
pub struct SchedulerPoolMessageSender {
    pub process_registry: Arc<Mutex<HashMap<ProcId, Sender<Message>>>>,
}

#[derive(Debug, Clone)]
pub struct SchedulerPoolProcessSpawner {
    pub next_proc_id: Arc<Mutex<ProcId>>,
    pub process_submission_queue: Arc<Mutex<Vec<Arc<Mutex<TinyProc>>>>>,
    pub running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>,
    pub process_registry: Arc<Mutex<HashMap<ProcId, Sender<Message>>>>,
    pub message_sender: Arc<dyn MessageSender>,
}

impl Scheduler {
    pub fn new(id: usize) -> Self {
        let local_queue = Worker::new_fifo();
        Scheduler {
            id,
            local_queue,
            remote_stealers: Vec::new(),
            running: true,
        }
    }
    
    pub fn add_process(&self, proc: Arc<Mutex<TinyProc>>) {
        self.local_queue.push(proc);
    }
    
    pub fn get_next_process(&self) -> Option<Arc<Mutex<TinyProc>>> {
        self.local_queue.pop()
    }
    
    pub fn steal_from_others(&self) -> Option<Arc<Mutex<TinyProc>>> {
        for stealer in &self.remote_stealers {
            if let crossbeam_deque::Steal::Success(proc) = stealer.steal() {
                return Some(proc);
            }
        }
        None
    }
    
    pub fn run_scheduler_loop(&mut self, submission_queue: Arc<Mutex<Vec<Arc<Mutex<TinyProc>>>>>, shutdown_flag: Arc<Mutex<bool>>, running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>, registry: Arc<Mutex<HashMap<ProcId, Sender<Message>>>>) {
        while self.running {
            // Check for shutdown
            if let Ok(shutdown) = shutdown_flag.try_lock() {
                if *shutdown {
                    break;
                }
            }
            
            // Try to get a process from local queue
            if let Some(proc_arc) = self.get_next_process() {
                self.execute_process_with_cleanup(proc_arc.clone(), running_processes.clone(), registry.clone());
                continue;
            }
            
            // Try to get new processes from submission queue
            if let Ok(mut queue) = submission_queue.try_lock() {
                if let Some(proc_arc) = queue.pop() {
                    self.execute_process_with_cleanup(proc_arc.clone(), running_processes.clone(), registry.clone());
                    continue;
                }
            }
            
            // If no local work, try to steal from other schedulers
            if let Some(proc_arc) = self.steal_from_others() {
                self.execute_process_with_cleanup(proc_arc.clone(), running_processes.clone(), registry.clone());
                continue;
            }
            
            // No work available, yield CPU
            thread::yield_now();
        }
    }
    
    fn execute_process(&mut self, proc_arc: Arc<Mutex<TinyProc>>) {
        let mut proc = proc_arc.lock().unwrap();
        
        match proc.state {
            ProcState::Ready | ProcState::Waiting => {
                match proc.run_until_yield() {
                    Ok(ProcState::Waiting) => {
                        // Process yielded, put it back in queue for next round
                        proc.state = ProcState::Ready;
                        drop(proc); // Release lock before pushing back
                        self.local_queue.push(proc_arc);
                    }
                    Ok(ProcState::Exited) => {
                        // Process finished, don't re-queue
                    }
                    Err(e) => {
                        eprintln!("Process {} error: {:?}", proc.id, e);
                        proc.state = ProcState::Exited;
                    }
                    _ => {
                        // Other states, re-queue
                        drop(proc);
                        self.local_queue.push(proc_arc);
                    }
                }
            }
            ProcState::Exited => {
                // Process is done, don't re-queue
            }
            _ => {
                // Re-queue for other states
                drop(proc);
                self.local_queue.push(proc_arc);
            }
        }
    }
    
    fn execute_process_with_cleanup(&mut self, proc_arc: Arc<Mutex<TinyProc>>, running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>, registry: Arc<Mutex<HashMap<ProcId, Sender<Message>>>>) {
        let proc_id = {
            let proc = proc_arc.lock().unwrap();
            proc.id
        };
        
        let mut proc = proc_arc.lock().unwrap();
        
        match proc.state {
            ProcState::Ready | ProcState::Waiting => {
                // If process is waiting for a message, check if it has one now
                if proc.waiting_for_message && !proc.has_messages() {
                    // Still waiting for a message, requeue with low priority
                    drop(proc);
                    self.local_queue.push(proc_arc);
                    return;
                }
                
                match proc.run_until_yield() {
                    Ok(ProcState::Waiting) => {
                        // Process yielded, put it back in queue for next round
                        proc.state = ProcState::Ready;
                        drop(proc); // Release lock before pushing back
                        self.local_queue.push(proc_arc);
                    }
                    Ok(ProcState::Exited) => {
                        // Process finished, remove from running processes and registry
                        drop(proc);
                        let mut running = running_processes.lock().unwrap();
                        running.remove(&proc_id);
                        let mut reg = registry.lock().unwrap();
                        reg.remove(&proc_id);
                    }
                    Err(e) => {
                        eprintln!("Process {} error: {:?}", proc_id, e);
                        proc.state = ProcState::Exited;
                        drop(proc);
                        let mut running = running_processes.lock().unwrap();
                        running.remove(&proc_id);
                        let mut reg = registry.lock().unwrap();
                        reg.remove(&proc_id);
                    }
                    _ => {
                        // Other states, re-queue
                        drop(proc);
                        self.local_queue.push(proc_arc);
                    }
                }
            }
            ProcState::Exited => {
                // Process is done, remove from running processes and registry
                drop(proc);
                let mut running = running_processes.lock().unwrap();
                running.remove(&proc_id);
                let mut reg = registry.lock().unwrap();
                reg.remove(&proc_id);
            }
            _ => {
                // Re-queue for other states
                drop(proc);
                self.local_queue.push(proc_arc);
            }
        }
    }
}

impl MessageSender for SchedulerPoolMessageSender {
    fn send_message(&self, target_proc_id: ProcId, message: Message) -> Result<(), String> {
        let registry = self.process_registry.lock().unwrap();
        match registry.get(&target_proc_id) {
            Some(sender) => {
                sender.send(message).map_err(|e| format!("Failed to send message: {}", e))
            }
            None => Err(format!("Process {} not found", target_proc_id))
        }
    }
}

impl ProcessSpawner for SchedulerPoolProcessSpawner {
    fn spawn_process(&self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>) {
        // Get next process ID
        let proc_id = {
            let mut id = self.next_proc_id.lock().unwrap();
            let current_id = *id;
            *id += 1;
            current_id
        };
        
        let (mut proc, sender) = TinyProc::new(proc_id, instructions);
        
        // Set the message sender and process spawner for the new process
        proc.message_sender = Some(self.message_sender.clone());
        proc.process_spawner = Some(Arc::new(self.clone()));
        
        // Add process to submission queue for schedulers to pick up
        let proc_arc = Arc::new(Mutex::new(proc));
        
        // Track running processes
        {
            let mut running = self.running_processes.lock().unwrap();
            running.insert(proc_id, proc_arc.clone());
        }
        
        // Register process in the registry for message delivery
        {
            let mut registry = self.process_registry.lock().unwrap();
            registry.insert(proc_id, sender.clone());
        }
        
        {
            let mut queue = self.process_submission_queue.lock().unwrap();
            queue.push(proc_arc);
        }
        
        (proc_id, sender)
    }
}

impl SchedulerPool {
    pub fn new() -> Self {
        SchedulerPool {
            schedulers: Vec::new(),
            global_stealers: Vec::new(),
            next_proc_id: Arc::new(Mutex::new(1)),
            process_submission_queue: Arc::new(Mutex::new(Vec::new())),
            running_processes: Arc::new(Mutex::new(HashMap::new())),
            shutdown_flag: Arc::new(Mutex::new(false)),
            process_registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub fn new_with_threads(num_threads: usize) -> Self {
        let mut pool = Self::new();
        pool.spawn_smp_schedulers(num_threads);
        pool
    }
    
    pub fn get_next_proc_id(&self) -> ProcId {
        let mut id = self.next_proc_id.lock().unwrap();
        let current_id = *id;
        *id += 1;
        current_id
    }
    
    pub fn spawn_process(&self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>) {
        let proc_id = self.get_next_proc_id();
        let (mut proc, sender) = TinyProc::new(proc_id, instructions);
        
        // Create message sender
        let message_sender = Arc::new(SchedulerPoolMessageSender {
            process_registry: self.process_registry.clone(),
        });
        
        // Set the message sender for this process
        proc.message_sender = Some(message_sender.clone());
        
        // Set the process spawner for this process
        proc.process_spawner = Some(Arc::new(SchedulerPoolProcessSpawner {
            next_proc_id: self.next_proc_id.clone(),
            process_submission_queue: self.process_submission_queue.clone(),
            running_processes: self.running_processes.clone(),
            process_registry: self.process_registry.clone(),
            message_sender: message_sender,
        }));
        
        // Add process to submission queue for schedulers to pick up
        let proc_arc = Arc::new(Mutex::new(proc));
        
        // Track running processes
        {
            let mut running = self.running_processes.lock().unwrap();
            running.insert(proc_id, proc_arc.clone());
        }
        
        // Register process in the registry for message delivery
        {
            let mut registry = self.process_registry.lock().unwrap();
            registry.insert(proc_id, sender.clone());
        }
        
        {
            let mut queue = self.process_submission_queue.lock().unwrap();
            queue.push(proc_arc);
        }
        
        (proc_id, sender)
    }
    
    pub fn send_message(&self, target_proc_id: ProcId, message: Message) -> Result<(), String> {
        let registry = self.process_registry.lock().unwrap();
        match registry.get(&target_proc_id) {
            Some(sender) => {
                sender.send(message).map_err(|e| format!("Failed to send message: {}", e))
            }
            None => Err(format!("Process {} not found", target_proc_id))
        }
    }
    
    pub fn spawn_smp_schedulers(&mut self, num_threads: usize) {
        let mut stealers = Vec::new();
        let mut workers = Vec::new();
        
        // Create workers and stealers
        for _ in 0..num_threads {
            let worker = Worker::new_fifo();
            let stealer = worker.stealer();
            workers.push(worker);
            stealers.push(stealer);
        }
        
        self.global_stealers = stealers.clone();
        
        // Spawn scheduler threads
        for (id, worker) in workers.into_iter().enumerate() {
            let remote_stealers = stealers.iter()
                .enumerate()
                .filter(|(i, _)| *i != id)
                .map(|(_, stealer)| stealer.clone())
                .collect();
            
            let next_proc_id = self.next_proc_id.clone();
            let submission_queue = self.process_submission_queue.clone();
            let shutdown_flag = self.shutdown_flag.clone();
            let running_processes = self.running_processes.clone();
            let registry = self.process_registry.clone();
            
            let handle = thread::spawn(move || {
                let mut scheduler = Scheduler {
                    id,
                    local_queue: worker,
                    remote_stealers,
                    running: true,
                };
                
                scheduler.run_scheduler_loop(submission_queue, shutdown_flag, running_processes, registry);
            });
            
            self.schedulers.push(handle);
        }
    }
    
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Keep running until all processes complete
        loop {
            // Check if there are any processes still running
            let (queue_len, running_count) = {
                let queue = self.process_submission_queue.lock().unwrap();
                let running = self.running_processes.lock().unwrap();
                (queue.len(), running.len())
            };
            
            if queue_len == 0 && running_count == 0 {
                // No processes queued and no processes running, we're done
                break;
            }
            
            thread::sleep(Duration::from_millis(10));
        }
        
        // Signal shutdown to all schedulers
        {
            let mut shutdown = self.shutdown_flag.lock().unwrap();
            *shutdown = true;
        }
        
        Ok(())
    }
    
    pub fn wait_for_completion(self) {
        for handle in self.schedulers {
            handle.join().unwrap();
        }
    }
}

// Single-threaded scheduler implementation
pub struct SingleThreadScheduler {
    processes: Vec<Arc<Mutex<TinyProc>>>,
    current_index: usize,
    next_proc_id: ProcId,
}

impl SingleThreadScheduler {
    pub fn new() -> Self {
        SingleThreadScheduler {
            processes: Vec::new(),
            current_index: 0,
            next_proc_id: 1,
        }
    }
    
    pub fn spawn_process(&mut self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>) {
        let proc_id = self.next_proc_id;
        self.next_proc_id += 1;
        
        let (proc, sender) = TinyProc::new(proc_id, instructions);
        self.processes.push(Arc::new(Mutex::new(proc)));
        
        (proc_id, sender)
    }
    
    pub fn run_round_robin(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut active_processes = true;
        
        while active_processes {
            active_processes = false;
            let mut processes_to_remove = Vec::new();
            
            for (i, proc_arc) in self.processes.iter().enumerate() {
                let mut proc = proc_arc.lock().unwrap();
                
                match proc.state {
                    ProcState::Ready => {
                        let new_state = proc.run_until_yield()?;
                        
                        match new_state {
                            ProcState::Exited => {
                                processes_to_remove.push(i);
                            }
                            ProcState::Waiting => {
                                // Process yielded - either out of reductions or manual yield
                                // Set to Ready for next round
                                proc.state = ProcState::Ready;
                                active_processes = true;
                            }
                            _ => active_processes = true,
                        }
                    }
                    ProcState::Waiting => {
                        // Process was waiting for a message, check if it has one now
                        if proc.waiting_for_message && proc.has_messages() {
                            proc.waiting_for_message = false;
                            proc.state = ProcState::Ready;
                            active_processes = true;
                        } else if !proc.waiting_for_message {
                            // Process was just yielding, set to Ready for next round
                            proc.state = ProcState::Ready;
                            active_processes = true;
                        }
                        // If still waiting for message and no messages available, leave as Waiting
                    }
                    ProcState::Exited => {
                        processes_to_remove.push(i);
                    }
                    _ => active_processes = true,
                }
            }
            
            // Remove exited processes (in reverse order to maintain indices)
            for &i in processes_to_remove.iter().rev() {
                self.processes.remove(i);
            }
            
            if self.processes.is_empty() {
                break;
            }
        }
        
        Ok(())
    }
}

// Basic test function for concurrency
pub fn test_concurrency() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing basic concurrency features...");
    
    // Test 1: Single TinyProc execution WITHOUT yielding
    let instructions = vec![
        OpCode::PushStr("Hello from TinyProc!".to_string()),
        OpCode::Print,
        OpCode::PushInt(42),
        OpCode::PushInt(8),
        OpCode::Add,
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (mut proc, _sender) = TinyProc::new(1, instructions);
    proc.trace_enabled = true;
    
    println!("=== Test 1: Single TinyProc (no yield) ===");
    match proc.run_until_yield()? {
        ProcState::Exited => println!("Process completed successfully"),
        ProcState::Waiting => println!("Process yielded"),
        _ => println!("Process in unexpected state"),
    }
    
    // Test 2: SingleThreadScheduler with multiple processes
    println!("\n=== Test 2: SingleThreadScheduler ===");
    let mut scheduler = SingleThreadScheduler::new();
    
    // Create two simple processes
    let proc1_instructions = vec![
        OpCode::PushStr("Process 1 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 1 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let proc2_instructions = vec![
        OpCode::PushStr("Process 2 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 2 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    scheduler.spawn_process(proc1_instructions);
    scheduler.spawn_process(proc2_instructions);
    
    scheduler.run_round_robin()?;
    
    println!("Concurrency tests completed successfully!");
    Ok(())
}

// Multi-threaded scheduler test function
pub fn test_multithreaded_scheduler() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing multi-threaded scheduler...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create some test processes
    let process1_instructions = vec![
        OpCode::PushStr("Thread Process 1 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Thread Process 1 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let process2_instructions = vec![
        OpCode::PushStr("Thread Process 2 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Thread Process 2 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let process3_instructions = vec![
        OpCode::PushStr("Thread Process 3 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Thread Process 3 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn processes
    let (proc1_id, _sender1) = scheduler_pool.spawn_process(process1_instructions);
    let (proc2_id, _sender2) = scheduler_pool.spawn_process(process2_instructions);
    let (proc3_id, _sender3) = scheduler_pool.spawn_process(process3_instructions);
    
    println!("Spawned processes: {}, {}, {}", proc1_id, proc2_id, proc3_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Multi-threaded scheduler test completed!");
    Ok(())
}

// Message passing test function
pub fn test_message_passing() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing message passing between processes...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create process that will send messages (sender will be process 1, receiver will be process 2)
    let sender_process_instructions = vec![
        OpCode::Yield,      // Let receiver start first
        OpCode::PushInt(2), // Target process ID
        OpCode::PushStr("Hello from Process 1!".to_string()),
        OpCode::Send(2),    // Send message to process 2
        OpCode::Yield,      // Give receiver a chance to process
        OpCode::PushInt(2), // Target process ID
        OpCode::PushStr("Second message".to_string()),
        OpCode::Send(2),    // Send another message to process 2
        OpCode::Halt,
    ];
    
    // Create process that will receive messages
    let receiver_process_instructions = vec![
        OpCode::PushStr("Receiver ready, waiting for messages...".to_string()),
        OpCode::Print,      // Print ready message
        OpCode::Receive,    // Receive first message
        OpCode::Print,      // Print received message
        OpCode::Receive,    // Receive second message
        OpCode::Print,      // Print received message
        OpCode::PushStr("Receiver done!".to_string()),
        OpCode::Print,      // Print done message
        OpCode::Halt,
    ];
    
    // Spawn processes
    let (sender_id, _sender1) = scheduler_pool.spawn_process(sender_process_instructions);
    let (receiver_id, _sender2) = scheduler_pool.spawn_process(receiver_process_instructions);
    
    println!("Spawned sender process: {}, receiver process: {}", sender_id, receiver_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Message passing test completed!");
    Ok(())
}

// Process monitoring and linking test function
pub fn test_process_monitoring_linking() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing process monitoring and linking...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // First spawn the monitored process that will wait longer
    let monitored_process_instructions = vec![
        OpCode::PushStr("Monitored process starting".to_string()),
        OpCode::Print,
        OpCode::Yield,  // Let other processes monitor/link
        OpCode::Yield,  // Give more time for setup
        OpCode::Yield,  // Give even more time for setup
        OpCode::Yield,  // Give even more time for setup
        OpCode::PushStr("Monitored process about to exit".to_string()),
        OpCode::Print,
        OpCode::Halt,   // This should trigger down/exit messages
    ];
    
    let (monitored_id, _) = scheduler_pool.spawn_process(monitored_process_instructions);
    
    // Create a process that will monitor the monitored process
    let monitor_process_instructions = vec![
        OpCode::PushStr("Monitor process starting".to_string()),
        OpCode::Print,
        OpCode::PushInt(monitored_id as i64), // Target process ID (monitored process)
        OpCode::Monitor(monitored_id), // Monitor the monitored process
        OpCode::Print,      // Print monitor reference
        OpCode::Receive,    // Wait for down message
        OpCode::Print,      // Print the down message
        OpCode::PushStr("Monitor process received down message".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Create a process that will link to the monitored process
    let link_process_instructions = vec![
        OpCode::PushStr("Link process starting".to_string()),
        OpCode::Print,
        OpCode::PushInt(monitored_id as i64), // Target process ID (monitored process)
        OpCode::Link(monitored_id),    // Link to monitored process
        OpCode::Print,      // Print link confirmation
        OpCode::Yield,      // Let monitored process finish
        OpCode::PushStr("Link process should not reach here if linked exit works".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn monitor and link processes
    let (monitor_id, _) = scheduler_pool.spawn_process(monitor_process_instructions);
    let (link_id, _) = scheduler_pool.spawn_process(link_process_instructions);
    
    println!("Spawned monitor process: {}, monitored process: {}, link process: {}", 
             monitor_id, monitored_id, link_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Process monitoring and linking test completed!");
    Ok(())
}

// Process spawning test function
pub fn test_process_spawning() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing process spawning...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create a process that will spawn other processes
    let parent_process_instructions = vec![
        OpCode::PushStr("Parent process starting".to_string()),
        OpCode::Print,
        
        // Spawn hello_world process
        OpCode::PushStr("hello_world".to_string()),
        OpCode::Spawn,
        OpCode::PushStr("Spawned hello_world process with ID: ".to_string()),
        OpCode::Print,
        OpCode::Print, // Print the spawned process ID
        
        OpCode::PushStr("Parent process done".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn the parent process
    let (parent_id, _sender) = scheduler_pool.spawn_process(parent_process_instructions);
    
    println!("Spawned parent process: {}", parent_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Process spawning test completed!");
    Ok(())
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
            waiting_for_message: false,
            monitors: HashMap::new(),
            monitored_by: HashMap::new(),
            linked_processes: HashSet::new(),
            exit_reason: None,
            
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
                        println!("Process {} is linked to {}, exiting due to exit signal", self.id, pid);
                        // In BEAM, linked processes normally exit when receiving exit signals
                        self.handle_process_exit(format!("exit_from_{}", pid));
                        exit_signal_received = true;
                        break;
                    } else {
                        println!("Process {} not linked to {}, discarding exit signal", self.id, pid);
                        // Not linked, just discard the message
                        // In a real implementation, we might handle this differently
                    }
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
            OpCode::Halt => {
                self.handle_process_exit("normal".to_string());
                // Don't advance IP for Halt - process is done
                return Ok(());
            }
            OpCode::Spawn => {
                // For now, spawn a simple process with basic instructions
                // In a real implementation, we'd parse the function from the stack
                let function_value = self.pop_stack("SPAWN")?;
                
                // Create a simple process based on the function value
                let new_process_instructions = match function_value {
                    Value::Str(ref s) if s == "hello_world" => {
                        vec![
                            OpCode::PushStr("Hello from spawned process!".to_string()),
                            OpCode::Print,
                            OpCode::Halt,
                        ]
                    }
                    Value::Str(ref s) if s == "counter" => {
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
            OpCode::Yield => {
                // Manually yield to scheduler
                self.ip += 1; // Advance IP before yielding
                self.state = ProcState::Waiting;
                return Ok(());
            }
            OpCode::Send(_proc_id) => {
                // Get the message and target process ID from the stack
                let message_value = self.pop_stack("SEND")?;
                let target_proc_id = self.pop_stack("SEND")?;
                
                // Convert target process ID to ProcId
                let target_id = match target_proc_id {
                    Value::Int(id) => id as ProcId,
                    _ => return Err(VMError::TypeMismatch {
                        expected: "int (process ID)".to_string(),
                        got: format!("{:?}", target_proc_id),
                        operation: "SEND".to_string(),
                    }),
                };
                
                // Convert Value to Message
                let message = Message::Value(message_value);
                
                // Send message using the message sender
                if let Some(sender) = &self.message_sender {
                    match sender.send_message(target_id, message) {
                        Ok(_) => {
                            // Message sent successfully
                        }
                        Err(e) => {
                            eprintln!("Failed to send message to process {}: {}", target_id, e);
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
}

#[derive(Debug, Clone)]
pub struct FunctionProfiler {
    pub start_time: Instant,
    pub instruction_count: usize,
}

impl FunctionProfiler {
    pub fn new() -> Self {
        FunctionProfiler {
            start_time: Instant::now(),
            instruction_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Profiler {
    pub function_timings: HashMap<String, Duration>,
    pub instruction_counts: HashMap<String, usize>,
    pub call_counts: HashMap<String, usize>,
    pub peak_stack_depth: usize,
    pub total_allocations: usize,
    pub peak_heap_size: usize,
    pub current_function_stack: Vec<(String, FunctionProfiler)>,
    pub call_depth: usize,
}

impl Profiler {
    pub fn new() -> Self {
        Profiler {
            function_timings: HashMap::new(),
            instruction_counts: HashMap::new(),
            call_counts: HashMap::new(),
            peak_stack_depth: 0,
            total_allocations: 0,
            peak_heap_size: 0,
            current_function_stack: Vec::new(),
            call_depth: 0,
        }
    }

    pub fn start_function(&mut self, function_name: String) {
        self.call_depth += 1;
        let profiler = FunctionProfiler::new();
        self.current_function_stack.push((function_name.clone(), profiler));
        
        // Update call count
        *self.call_counts.entry(function_name).or_insert(0) += 1;
    }

    pub fn end_function(&mut self) -> Option<String> {
        if let Some((function_name, profiler)) = self.current_function_stack.pop() {
            self.call_depth = self.call_depth.saturating_sub(1);
            
            let elapsed = profiler.start_time.elapsed();
            
            // Add to total timing for this function
            *self.function_timings.entry(function_name.clone()).or_insert(Duration::ZERO) += elapsed;
            
            // Add to total instruction count for this function
            *self.instruction_counts.entry(function_name.clone()).or_insert(0) += profiler.instruction_count;
            
            Some(function_name)
        } else {
            None
        }
    }

    pub fn record_instruction(&mut self) {
        // Increment instruction count for current function
        if let Some((_, profiler)) = self.current_function_stack.last_mut() {
            profiler.instruction_count += 1;
        }
    }

    pub fn update_stack_depth(&mut self, depth: usize) {
        if depth > self.peak_stack_depth {
            self.peak_stack_depth = depth;
        }
    }

    pub fn record_allocation(&mut self, size: usize) {
        self.total_allocations += 1;
        if size > self.peak_heap_size {
            self.peak_heap_size = size;
        }
    }

    pub fn print_results(&self, config: &VMConfig) {
        if self.function_timings.is_empty() && self.call_counts.is_empty() {
            return;
        }

        println!("\n{}", "═══ Profiling Results ═══".bright_cyan().bold());
        
        match config.output_mode {
            OutputMode::PrettyTable => {
                // Function Summary Table
                let mut table = Table::new();
                table.load_preset(UTF8_FULL)
                     .apply_modifier(UTF8_SOLID_INNER_BORDERS);
                
                // Set colored headers
                table.set_header(vec![
                    Cell::new("Function").add_attribute(Attribute::Bold).fg(Color::Cyan),
                    Cell::new("Calls").add_attribute(Attribute::Bold).fg(Color::Green),
                    Cell::new("Time (ms)").add_attribute(Attribute::Bold).fg(Color::Yellow),
                    Cell::new("Instructions").add_attribute(Attribute::Bold).fg(Color::Blue),
                    Cell::new("Avg Time/Call (μs)").add_attribute(Attribute::Bold).fg(Color::Magenta),
                ]);
                
                let mut functions: Vec<_> = self.function_timings.keys().collect();
                functions.sort();
                
                for function_name in functions {
                    let timing = self.function_timings.get(function_name).unwrap();
                    let instructions = self.instruction_counts.get(function_name).unwrap_or(&0);
                    let calls = self.call_counts.get(function_name).unwrap_or(&0);
                    let avg_time_us = if *calls > 0 {
                        timing.as_micros() as f64 / *calls as f64
                    } else {
                        0.0
                    };
                    
                    // Color code performance metrics
                    let time_ms = timing.as_secs_f64() * 1000.0;
                    let time_color = if time_ms > 10.0 { Color::Red } 
                                   else if time_ms > 1.0 { Color::Yellow } 
                                   else { Color::Green };
                    
                    let calls_color = if *calls > 100 { Color::Red }
                                    else if *calls > 10 { Color::Yellow }
                                    else { Color::Green };
                    
                    table.add_row(vec![
                        Cell::new(function_name).fg(Color::White),
                        Cell::new(calls.to_string()).fg(calls_color),
                        Cell::new(format!("{:.3}", time_ms)).fg(time_color),
                        Cell::new(instructions.to_string()).fg(Color::Blue),
                        Cell::new(format!("{:.1}", avg_time_us)).fg(Color::Magenta),
                    ]);
                }
                
                println!("{}", table);
                
                // Performance Summary Table
                let mut summary_table = Table::new();
                summary_table.load_preset(UTF8_FULL)
                             .apply_modifier(UTF8_SOLID_INNER_BORDERS);
                summary_table.set_header(vec![
                    Cell::new("Performance Metric").add_attribute(Attribute::Bold).fg(Color::Cyan),
                    Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::White),
                ]);
                
                // Color code memory metrics
                let stack_color = if self.peak_stack_depth > 50 { Color::Red }
                                else if self.peak_stack_depth > 20 { Color::Yellow }
                                else { Color::Green };
                
                let heap_color = if self.peak_heap_size > 10000 { Color::Red }
                               else if self.peak_heap_size > 1000 { Color::Yellow }
                               else { Color::Green };
                
                summary_table.add_row(vec![
                    Cell::new("Peak Stack Depth").fg(Color::White),
                    Cell::new(format!("{} frames", self.peak_stack_depth)).fg(stack_color),
                ]);
                summary_table.add_row(vec![
                    Cell::new("Total Allocations").fg(Color::White),
                    Cell::new(self.total_allocations.to_string()).fg(Color::Blue),
                ]);
                summary_table.add_row(vec![
                    Cell::new("Peak Heap Size").fg(Color::White),
                    Cell::new(format!("{} bytes", self.peak_heap_size)).fg(heap_color),
                ]);
                
                println!("{}", summary_table);
            }
            OutputMode::Plain => {
                println!("{}", "Function Summary:".bright_cyan().bold());
                let mut functions: Vec<_> = self.function_timings.keys().collect();
                functions.sort();
                
                for function_name in functions {
                    let timing = self.function_timings.get(function_name).unwrap();
                    let instructions = self.instruction_counts.get(function_name).unwrap_or(&0);
                    let calls = self.call_counts.get(function_name).unwrap_or(&0);
                    let time_ms = timing.as_secs_f64() * 1000.0;
                    
                    println!("  {} - {} calls - {} ms - {} instructions", 
                        function_name.white(),
                        format!("{}", calls).green(),
                        format!("{:.3}", time_ms).yellow(),
                        format!("{}", instructions).blue());
                }
                
                println!("\n{}: {} frames", "Peak Stack Depth".bright_cyan(), 
                         format!("{}", self.peak_stack_depth).green());
                println!("{}: {}", "Total Allocations".bright_cyan(), 
                         format!("{}", self.total_allocations).blue());
                println!("{}: {} bytes", "Peak Heap Size".bright_cyan(), 
                         format!("{}", self.peak_heap_size).yellow());
            }
        }
    }
}

fn run_vm_tests(config: &VMConfig) {
    if !config.run_tests {
        return;
    }

    let mut results = Vec::new();
    
    // Test 1: Basic arithmetic
    let program = vec![
        OpCode::PushInt(5),
        OpCode::PushInt(3),
        OpCode::Add,
        OpCode::Halt,
    ];
    let mut vm = VM::new(program);
    if vm.run().is_ok() && vm.stack.len() == 1 {
        let result = format!("{}", vm.stack[0]);
        results.push(TestResult {
            name: "Basic addition".to_string(),
            expected: "8".to_string(),
            actual: result.clone(),
            passed: result == "8",
        });
    } else {
        results.push(TestResult {
            name: "Basic addition".to_string(),
            expected: "8".to_string(),
            actual: "ERROR".to_string(),
            passed: false,
        });
    }

    // Test 2: String concatenation
    let program = vec![
        OpCode::PushStr("Hello".to_string()),
        OpCode::PushStr(" World".to_string()),
        OpCode::Concat,
        OpCode::Halt,
    ];
    let mut vm = VM::new(program);
    if vm.run().is_ok() && vm.stack.len() == 1 {
        let result = format!("{}", vm.stack[0]);
        results.push(TestResult {
            name: "String concat".to_string(),
            expected: "Hello World".to_string(),
            actual: result.clone(),
            passed: result == "Hello World",
        });
    } else {
        results.push(TestResult {
            name: "String concat".to_string(),
            expected: "Hello World".to_string(),
            actual: "ERROR".to_string(),
            passed: false,
        });
    }

    // Test 3: Variable storage and retrieval
    let program = vec![
        OpCode::PushInt(42),
        OpCode::Store("x".to_string()),
        OpCode::Load("x".to_string()),
        OpCode::Halt,
    ];
    let mut vm = VM::new(program);
    if vm.run().is_ok() && vm.stack.len() == 1 {
        let result = format!("{}", vm.stack[0]);
        results.push(TestResult {
            name: "Variable store/load".to_string(),
            expected: "42".to_string(),
            actual: result.clone(),
            passed: result == "42",
        });
    } else {
        results.push(TestResult {
            name: "Variable store/load".to_string(),
            expected: "42".to_string(),
            actual: "ERROR".to_string(),
            passed: false,
        });
    }

    report_test_results(&results, config);
}

fn report_test_results(results: &[TestResult], config: &VMConfig) {
    match config.output_mode {
        OutputMode::PrettyTable => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                 .apply_modifier(UTF8_SOLID_INNER_BORDERS);
            table.set_header(vec!["Test", "Expected", "Actual", "Result"]);

            for r in results {
                let status = if r.passed { "PASS" } else { "FAIL" };
                table.add_row(vec![
                    Cell::new(&r.name),
                    Cell::new(&r.expected),
                    Cell::new(&r.actual),
                    Cell::new(status),
                ]);
            }

            println!("=== Unit Test Results ===");
            println!("{table}");
        }
        OutputMode::Plain => {
            println!("=== Unit Test Results ===");
            for r in results {
                let status = if r.passed { "PASS" } else { "FAIL" };
                println!(
                    "{} | expected: {} | actual: {} | {}",
                    r.name, r.expected, r.actual, status
                );
            }
        }
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let total = results.len();
    println!("Tests passed: {}/{}", passed, total);
}

fn report_gc_stats(stats: &GcStats, config: &VMConfig) {
    match config.output_mode {
        OutputMode::PrettyTable => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                 .apply_modifier(UTF8_SOLID_INNER_BORDERS);
            table.set_header(vec![
                Cell::new("GC Metric").add_attribute(Attribute::Bold).fg(Color::Cyan),
                Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::White),
            ]);

            // Color code memory metrics
            let current_color = if stats.current_allocated > 10000 { Color::Red }
                              else if stats.current_allocated > 1000 { Color::Yellow }
                              else { Color::Green };

            table.add_row(vec![
                Cell::new("Total Allocated").fg(Color::White),
                Cell::new(&stats.total_allocated.to_string()).fg(Color::Blue),
            ]);
            table.add_row(vec![
                Cell::new("Total Freed").fg(Color::White),
                Cell::new(&stats.total_freed.to_string()).fg(Color::Green),
            ]);
            table.add_row(vec![
                Cell::new("Currently Allocated").fg(Color::White),
                Cell::new(&stats.current_allocated.to_string()).fg(current_color),
            ]);
            table.add_row(vec![
                Cell::new("Collections Performed").fg(Color::White),
                Cell::new(&stats.collections_performed.to_string()).fg(Color::Magenta),
            ]);

            println!("{}", "═══ GC Statistics ═══".bright_cyan().bold());
            println!("{table}");
        }
        OutputMode::Plain => {
            println!("{}", "═══ GC Statistics ═══".bright_cyan().bold());
            println!("{}: {}", "Total allocated".bright_cyan(), 
                     format!("{}", stats.total_allocated).blue());
            println!("{}: {}", "Total freed".bright_cyan(), 
                     format!("{}", stats.total_freed).green());
            println!("{}: {}", "Currently allocated".bright_cyan(), 
                     format!("{}", stats.current_allocated).yellow());
            println!("{}: {}", "Collections performed".bright_cyan(), 
                     format!("{}", stats.collections_performed).magenta());
        }
    }
}

#[derive(Debug, Clone)]
pub enum VMError {
    StackUnderflow(String),
    TypeMismatch { expected: String, got: String, operation: String },
    UndefinedVariable(String),
    IndexOutOfBounds { index: usize, length: usize },
    CallStackUnderflow,
    NoVariableScope,
    FileError { filename: String, error: String },
    ParseError { line: usize, instruction: String },
    InsufficientStackItems { needed: usize, available: usize },
    UnknownLabel(String),
    UnsupportedOperation(String),
}

impl fmt::Display for VMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VMError::StackUnderflow(op) => write!(f, "Stack underflow during {}", op),
            VMError::TypeMismatch { expected, got, operation } => 
                write!(f, "{} expects {} but got {}", operation, expected, got),
            VMError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            VMError::IndexOutOfBounds { index, length } => 
                write!(f, "Index {} out of bounds for list of length {}", index, length),
            VMError::CallStackUnderflow => write!(f, "Call stack underflow"),
            VMError::NoVariableScope => write!(f, "No variable scope available"),
            VMError::FileError { filename, error } => 
                write!(f, "File operation failed on {}: {}", filename, error),
            VMError::ParseError { line, instruction } => 
                write!(f, "Parse error on line {}: {}", line, instruction),
            VMError::InsufficientStackItems { needed, available } => 
                write!(f, "Need {} stack items but only {} available", needed, available),
            VMError::UnknownLabel(label) => write!(f, "Unknown label: {}", label),
            VMError::UnsupportedOperation(op) => write!(f, "Unsupported operation: {}", op),
        }
    }
}

impl std::error::Error for VMError {}

type VMResult<T> = Result<T, VMError>;

#[derive(Debug, Clone)]
enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
    List(Vec<Value>),
    Object(HashMap<String, Value>),
    Bytes(Vec<u8>),
    Connection(String), // Network connection handle (simplified as string ID)
    Stream(String),     // Data stream handle (simplified as string ID)
    Future(String),     // Async operation handle (simplified as string ID)
    Function { addr: usize, params: Vec<String> },
    Closure { addr: usize, params: Vec<String>, captured: HashMap<String, Value> },
    Exception { message: String, stack_trace: Vec<String> },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            },
            Value::Object(map) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in map {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}: {}", key, value)?;
                    first = false;
                }
                write!(f, "}}")
            },
            Value::Bytes(bytes) => {
                write!(f, "Bytes({})", bytes.len())
            },
            Value::Connection(id) => {
                write!(f, "Connection({})", id)
            },
            Value::Stream(id) => {
                write!(f, "Stream({})", id)
            },
            Value::Future(id) => {
                write!(f, "Future({})", id)
            },
            Value::Function { addr, params } => {
                write!(f, "function@{} ({})", addr, params.join(", "))
            },
            Value::Closure { addr, params, captured } => {
                write!(f, "closure@{} ({}) [captured: {}]", 
                    addr, 
                    params.join(", "),
                    captured.len()
                )
            },
            Value::Exception { message, stack_trace } => {
                write!(f, "Exception: {}", message)?;
                if !stack_trace.is_empty() {
                    write!(f, "\nStack trace:")?;
                    for trace in stack_trace {
                        write!(f, "\n  {}", trace)?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
enum OpCode {
    PushInt(i64),
    PushFloat(f64),
    PushStr(String),
    Add,
    AddF,
    Sub,
    SubF,
    MulF,
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
    Yield,             // yield control to scheduler
    Send(ProcId),      // send message to process
    Monitor(ProcId),   // monitor a process
    Demonitor(String), // stop monitoring (using monitor reference)
    Link(ProcId),      // link to a process
    Unlink(ProcId),    // unlink from a process
}

// Garbage Collection Engine Trait
trait GcEngine: std::fmt::Debug + Send + Sync {
    fn alloc(&mut self, value: Value) -> GcRef;
    fn mark_from_roots(&mut self, roots: &[&Value]);
    fn sweep(&mut self) -> usize; // returns number of objects collected
    fn stats(&self) -> GcStats;
}

#[derive(Debug, Clone)]
pub struct GcStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_allocated: usize,
    pub collections_performed: usize,
}

// GC Reference wrapper
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GcRef {
    id: usize,
    generation: usize,
}

impl GcRef {
    fn new(id: usize) -> Self {
        GcRef { id, generation: 0 }
    }
}

// Mark and Sweep Garbage Collector
#[derive(Debug)]
struct MarkSweepGc {
    objects: HashMap<usize, (Value, bool)>, // id -> (value, marked)
    next_id: usize,
    stats: GcStats,
    debug_mode: bool,
}

impl MarkSweepGc {
    fn new(debug_mode: bool) -> Self {
        MarkSweepGc {
            objects: HashMap::new(),
            next_id: 0,
            stats: GcStats {
                total_allocated: 0,
                total_freed: 0,
                current_allocated: 0,
                collections_performed: 0,
            },
            debug_mode,
        }
    }
}

impl GcEngine for MarkSweepGc {
    fn alloc(&mut self, value: Value) -> GcRef {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, (value, false));
        self.stats.total_allocated += 1;
        self.stats.current_allocated += 1;
        
        if self.debug_mode {
            println!("GC: Allocated object {} (total: {})", id, self.stats.current_allocated);
        }
        
        GcRef::new(id)
    }

    fn mark_from_roots(&mut self, _roots: &[&Value]) {
        // Mark all objects for now (simplified marking)
        for (_, (_, marked)) in self.objects.iter_mut() {
            *marked = true;
        }
        
        if self.debug_mode {
            println!("GC: Marked {} objects", self.objects.len());
        }
    }

    fn sweep(&mut self) -> usize {
        let initial_count = self.objects.len();
        self.objects.retain(|id, (_, marked)| {
            if *marked {
                true
            } else {
                if self.debug_mode {
                    println!("GC: Collecting object {}", id);
                }
                false
            }
        });
        
        // Reset marks for next collection
        for (_, (_, marked)) in self.objects.iter_mut() {
            *marked = false;
        }
        
        let collected = initial_count - self.objects.len();
        self.stats.total_freed += collected;
        self.stats.current_allocated -= collected;
        self.stats.collections_performed += 1;
        
        if self.debug_mode {
            println!("GC: Collected {} objects, {} remaining", collected, self.objects.len());
        }
        
        collected
    }

    fn stats(&self) -> GcStats {
        self.stats.clone()
    }
}

// No-op Garbage Collector (for testing and comparison)
#[derive(Debug)]
struct NoGc {
    next_id: usize,
    stats: GcStats,
}

impl NoGc {
    fn new() -> Self {
        NoGc {
            next_id: 0,
            stats: GcStats {
                total_allocated: 0,
                total_freed: 0,
                current_allocated: 0,
                collections_performed: 0,
            },
        }
    }
}

impl GcEngine for NoGc {
    fn alloc(&mut self, _value: Value) -> GcRef {
        let id = self.next_id;
        self.next_id += 1;
        self.stats.total_allocated += 1;
        self.stats.current_allocated += 1;
        GcRef::new(id)
    }

    fn mark_from_roots(&mut self, _roots: &[&Value]) {
        // No-op
    }

    fn sweep(&mut self) -> usize {
        self.stats.collections_performed += 1;
        0 // Never collect anything
    }

    fn stats(&self) -> GcStats {
        self.stats.clone()
    }
}

#[derive(Debug, Clone)]
struct ExceptionHandler {
    catch_addr: usize,           // address to jump to on exception
    stack_size: usize,           // stack size when try block started
    call_stack_size: usize,      // call stack size when try block started
    variable_frames: usize,      // number of variable frames when try block started
}

struct VM {
    stack: Vec<Value>,
    instructions: Vec<OpCode>,
    ip: usize,                              // instruction pointer
    call_stack: Vec<usize>,                 // return addresses for CALL/RET
    variables: Vec<HashMap<String, Value>>, // call frame stack
    // Exception handling
    try_stack: Vec<ExceptionHandler>,       // stack of try blocks
    // Module system
    exports: HashMap<String, Value>,        // exported symbols from this module
    loaded_modules: HashMap<String, HashMap<String, Value>>, // module_path -> exports
    loading_stack: Vec<String>,             // for circular dependency detection
    // Closure support
    lambda_captures: HashMap<String, Value>, // variables captured for current lambda
    // Performance improvements
    max_stack_size: usize,                  // Track maximum stack usage
    instruction_count: usize,               // Count of executed instructions
    // Debugging support
    debug_mode: bool,
    breakpoints: Vec<usize>,
    // Garbage Collection
    gc_engine: Box<dyn GcEngine>,           // Pluggable GC engine
    _gc_stats_enabled: bool,                 // Whether to show GC stats
    // Profiling and Tracing
    profiler: Option<Profiler>,             // Optional profiler for performance analysis
    trace_enabled: bool,                    // Whether to enable tracing
}

impl VM {
    fn new(instructions: Vec<OpCode>) -> Self {
        Self::new_with_gc(instructions, "mark-sweep", false, false)
    }

    #[allow(dead_code)]
    fn new_with_debug(instructions: Vec<OpCode>, debug_mode: bool) -> Self {
        Self::new_with_gc(instructions, "mark-sweep", debug_mode, false)
    }

    fn new_with_gc(instructions: Vec<OpCode>, gc_type: &str, debug_mode: bool, gc_stats_enabled: bool) -> Self {
        Self::new_with_config(instructions, gc_type, debug_mode, gc_stats_enabled, false, false)
    }

    fn new_with_config(instructions: Vec<OpCode>, gc_type: &str, debug_mode: bool, gc_stats_enabled: bool, trace_enabled: bool, profile_enabled: bool) -> Self {
        let gc_engine: Box<dyn GcEngine> = match gc_type {
            "no-gc" => Box::new(NoGc::new()),
            "mark-sweep" => Box::new(MarkSweepGc::new(debug_mode)),
            _ => Box::new(MarkSweepGc::new(debug_mode)), // Default to mark-sweep
        };

        VM {
            stack: Vec::with_capacity(1024), // Pre-allocate stack capacity
            instructions,
            ip: 0,
            call_stack: Vec::with_capacity(64), // Pre-allocate call stack
            variables: vec![HashMap::new()], // global frame
            try_stack: Vec::new(),
            exports: HashMap::new(),
            loaded_modules: HashMap::new(),
            loading_stack: Vec::new(),
            lambda_captures: HashMap::new(),
            max_stack_size: 0,
            instruction_count: 0,
            debug_mode,
            breakpoints: Vec::new(),
            gc_engine,
            _gc_stats_enabled: gc_stats_enabled,
            profiler: if profile_enabled { Some(Profiler::new()) } else { None },
            trace_enabled,
        }
    }

    #[allow(dead_code)]
    fn add_breakpoint(&mut self, address: usize) {
        if !self.breakpoints.contains(&address) {
            self.breakpoints.push(address);
            self.breakpoints.sort();
        }
    }

    #[allow(dead_code)]
    fn remove_breakpoint(&mut self, address: usize) {
        self.breakpoints.retain(|&x| x != address);
    }

    fn get_stats(&self) -> (usize, usize, usize) {
        (self.instruction_count, self.max_stack_size, self.stack.len())
    }

    fn get_gc_stats(&self) -> GcStats {
        self.gc_engine.stats()
    }

    #[allow(dead_code)]
    fn trigger_gc(&mut self) {
        // Collect roots from stack and variables
        let mut roots: Vec<&Value> = Vec::new();
        
        // Add stack values as roots
        for value in &self.stack {
            roots.push(value);
        }
        
        // Add variables as roots
        for frame in &self.variables {
            for (_name, value) in frame {
                roots.push(value);
            }
        }
        
        // Mark from roots
        self.gc_engine.mark_from_roots(&roots);
        
        // Sweep unreachable objects
        let collected = self.gc_engine.sweep();
        
        if self.debug_mode {
            println!("GC triggered: collected {} objects", collected);
        }
    }

    // Safe stack operations
    fn pop_stack(&mut self, operation: &str) -> VMResult<Value> {
        self.stack.pop().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }

    fn peek_stack(&self, operation: &str) -> VMResult<&Value> {
        self.stack.last().ok_or_else(|| VMError::StackUnderflow(operation.to_string()))
    }

    fn check_stack_size(&self, needed: usize, _operation: &str) -> VMResult<()> {
        if self.stack.len() < needed {
            Err(VMError::InsufficientStackItems { 
                needed, 
                available: self.stack.len() 
            })
        } else {
            Ok(())
        }
    }

    fn get_variable(&self, name: &str) -> VMResult<Value> {
        self.variables
            .last()
            .ok_or(VMError::NoVariableScope)?
            .get(name)
            .cloned()
            .ok_or_else(|| VMError::UndefinedVariable(name.to_string()))
    }

    fn set_variable(&mut self, name: String, value: Value) -> VMResult<()> {
        self.variables
            .last_mut()
            .ok_or(VMError::NoVariableScope)?
            .insert(name, value);
        Ok(())
    }

    fn pop_call_stack(&mut self) -> VMResult<usize> {
        self.call_stack.pop().ok_or(VMError::CallStackUnderflow)
    }

    fn pop_variable_frame(&mut self) -> VMResult<()> {
        if self.variables.len() <= 1 {
            Err(VMError::NoVariableScope)
        } else {
            self.variables.pop();
            Ok(())
        }
    }

    // Exception handling methods
    fn push_exception_handler(&mut self, catch_addr: usize) {
        let handler = ExceptionHandler {
            catch_addr,
            stack_size: self.stack.len(),
            call_stack_size: self.call_stack.len(),
            variable_frames: self.variables.len(),
        };
        self.try_stack.push(handler);
    }

    fn pop_exception_handler(&mut self) -> Option<ExceptionHandler> {
        self.try_stack.pop()
    }

    fn unwind_to_exception_handler(&mut self, handler: &ExceptionHandler) {
        // Unwind stack to the state when try block started
        self.stack.truncate(handler.stack_size);
        
        // Unwind call stack
        self.call_stack.truncate(handler.call_stack_size);
        
        // Unwind variable frames
        self.variables.truncate(handler.variable_frames);
    }

    fn throw_exception(&mut self, exception: Value) -> VMResult<()> {
        if let Some(handler) = self.pop_exception_handler() {
            // Unwind to the try block state
            self.unwind_to_exception_handler(&handler);
            
            // Push the exception onto the stack for the catch block
            self.stack.push(exception);
            
            // Jump to the catch block
            self.ip = handler.catch_addr;
            Ok(())
        } else {
            // No exception handler found, convert to VM error
            match exception {
                Value::Exception { message, .. } => {
                    Err(VMError::ParseError { line: self.ip, instruction: format!("Unhandled exception: {}", message) })
                }
                _ => {
                    Err(VMError::ParseError { line: self.ip, instruction: format!("Unhandled exception: {:?}", exception) })
                }
            }
        }
    }

    fn run(&mut self) -> VMResult<()> {
        while self.ip < self.instructions.len() {
            // Performance tracking
            self.instruction_count += 1;
            if self.stack.len() > self.max_stack_size {
                self.max_stack_size = self.stack.len();
            }

            // Profiling support
            if let Some(ref mut profiler) = self.profiler {
                profiler.record_instruction();
                profiler.update_stack_depth(self.stack.len());
            }

            // Tracing support
            if self.trace_enabled {
                let instruction = &self.instructions[self.ip];
                let indent = if let Some(ref profiler) = self.profiler {
                    "  ".repeat(profiler.call_depth)
                } else {
                    String::new()
                };
                println!("{} {}{} @ {}", 
                         "[trace]".bright_blue(),
                         indent, 
                         format!("{:?}", instruction).white(),
                         format!("0x{:04X}", self.ip).cyan());
            }

            // Debugging support
            if self.debug_mode {
                println!("IP: {}, Instruction: {:?}, Stack size: {}", 
                    self.ip, self.instructions[self.ip], self.stack.len());
            }

            // Breakpoint support
            if self.breakpoints.contains(&self.ip) {
                println!("Breakpoint hit at instruction {}: {:?}", 
                    self.ip, self.instructions[self.ip]);
                println!("Stack: {:?}", self.stack);
                println!("Variables: {:?}", self.variables.last());
                // In a real debugger, we'd wait for user input here
            }

            let instruction = &self.instructions[self.ip].clone();
            
            // Store original IP to detect jumps
            let original_ip = self.ip;
            
            // Check for HALT instruction first
            if matches!(instruction, OpCode::Halt) {
                break;
            }
            
            // Execute instruction and catch VM errors in try blocks
            match self.execute_instruction_safe(instruction) {
                Ok(()) => {
                    // Only increment IP if instruction didn't change it
                    if self.ip == original_ip {
                        self.ip += 1;
                    }
                }
                Err(vm_error) => {
                    // If we're in a try block, convert VM error to exception
                    if !self.try_stack.is_empty() {
                        let exception = Value::Exception {
                            message: vm_error.to_string(),
                            stack_trace: vec![format!("at instruction {}", self.ip)]
                        };
                        self.throw_exception(exception)?;
                        continue;
                    } else {
                        return Err(vm_error);
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_instruction_safe(&mut self, instruction: &OpCode) -> VMResult<()> {
        match instruction {
                OpCode::PushInt(n) => self.stack.push(Value::Int(*n)),
                OpCode::PushFloat(f) => self.stack.push(Value::Float(*f)),
                OpCode::PushStr(s) => self.stack.push(Value::Str(s.clone())),
                OpCode::Add => {
                    let b = self.pop_stack("ADD")?;
                    let a = self.pop_stack("ADD")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x + y)),
                        // Type coercion: int + float = float
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
                OpCode::AddF => {
                    let b = self.pop_stack("ADD_F")?;
                    let a = self.pop_stack("ADD_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x + y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "ADD_F".to_string() 
                        }),
                    }
                }
                OpCode::SubF => {
                    let b = self.pop_stack("SUB_F")?;
                    let a = self.pop_stack("SUB_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x - y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "SUB_F".to_string() 
                        }),
                    }
                }
                OpCode::MulF => {
                    let b = self.pop_stack("MUL_F")?;
                    let a = self.pop_stack("MUL_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x * y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "MUL_F".to_string() 
                        }),
                    }
                }
                OpCode::DivF => {
                    let b = self.pop_stack("DIV_F")?;
                    let a = self.pop_stack("DIV_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            if *y == 0.0 {
                                return Err(VMError::TypeMismatch { 
                                    expected: "non-zero divisor".to_string(), 
                                    got: "zero".to_string(), 
                                    operation: "DIV_F".to_string() 
                                });
                            }
                            self.stack.push(Value::Float(x / y));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "DIV_F".to_string() 
                        }),
                    }
                }
                OpCode::Concat => {
                    let b = self.pop_stack("CONCAT")?;
                    let a = self.pop_stack("CONCAT")?;
                    match (&a, &b) {
                        (Value::Str(x), Value::Str(y)) => self.stack.push(Value::Str(x.clone() + y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two strings".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "CONCAT".to_string() 
                        }),
                    }
                }
                OpCode::Print => {
                    let val = self.pop_stack("PRINT")?;
                    println!("{}", val);
                }
                OpCode::Jmp(target) => {
                    self.ip = *target;
                }
                OpCode::Jz(target) => {
                    let val = self.pop_stack("JZ")?;
                    let is_zero = match val {
                        Value::Int(0) => true,
                        Value::Bool(false) => true,
                        Value::Null => true,
                        _ => false,
                    };
                    if is_zero {
                        self.ip = *target;
                    }
                }
                OpCode::Call{ addr, params } => {
                    self.check_stack_size(params.len(), "CALL")?;
                    
                    // Create function name for profiling/tracing
                    let function_name = format!("fn@0x{:04X}", addr);
                    
                    // Function call tracing
                    if self.trace_enabled {
                        let indent = if let Some(ref profiler) = self.profiler {
                            "  ".repeat(profiler.call_depth)
                        } else {
                            String::new()
                        };
                        println!("{} {}CALL {} with {} params", 
                                 "[trace]".bright_blue(),
                                 indent, 
                                 function_name.yellow(),
                                 format!("{}", params.len()).green());
                    }
                    
                    // Function profiling
                    if let Some(ref mut profiler) = self.profiler {
                        profiler.start_function(function_name);
                    }
                    
                    self.call_stack.push(self.ip + 1);
                    let mut frame = HashMap::new();
                    for name in params.iter().rev() {
                        let value = self.pop_stack("CALL")?;
                        frame.insert(name.clone(), value);
                    }
                    self.variables.push(frame);
                    self.ip = *addr;
                }
                OpCode::Ret => {
                    // Function return tracing and profiling
                    if let Some(ref mut profiler) = self.profiler {
                        if let Some(function_name) = profiler.end_function() {
                            if self.trace_enabled {
                                let indent = "  ".repeat(profiler.call_depth);
                                let return_value = if !self.stack.is_empty() {
                                    format!(" → {:?}", self.stack.last().unwrap())
                                } else {
                                    String::new()
                                };
                                println!("{} {}RETURN from {}{}", 
                                         "[trace]".bright_blue(),
                                         indent, 
                                         function_name.yellow(),
                                         return_value.green());
                            }
                        }
                    }
                    
                    self.pop_variable_frame()?;
                    self.ip = self.pop_call_stack()?;
                }
                OpCode::Sub => {
                    let b = self.pop_stack("SUB")?;
                    let a = self.pop_stack("SUB")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x - y)),
                        // Type coercion: mixed int/float = float
                        (Value::Int(x), Value::Float(y)) => self.stack.push(Value::Float(*x as f64 - y)),
                        (Value::Float(x), Value::Int(y)) => self.stack.push(Value::Float(x - *y as f64)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x - y)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two numbers (int or float)".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "SUB".to_string() 
                        }),
                    }
                }
                OpCode::Dup => {
                    let val = self.peek_stack("DUP")?.clone();
                    self.stack.push(val);
                }
                OpCode::Store(name) => {
                    let val = self.pop_stack("STORE")?;
                    self.set_variable(name.clone(), val)?;
                }
                OpCode::Load(name) => {
                    let val = self.get_variable(&name)?;
                    self.stack.push(val);
                }
                OpCode::Delete(name) => {
                    let removed = self
                        .variables
                        .last_mut()
                        .ok_or(VMError::NoVariableScope)?
                        .remove(name);
                    if removed.is_none() {
                        eprintln!("Warning: tried to DELETE unknown variable '{}'", name);
                    }
                }
                OpCode::Eq => {
                    let b = self.pop_stack("EQ")?;
                    let a = self.pop_stack("EQ")?;
                    let result = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => x == y,
                        (Value::Float(x), Value::Float(y)) => (x - y).abs() < f64::EPSILON,
                        (Value::Str(x), Value::Str(y)) => x == y,
                        (Value::Bool(x), Value::Bool(y)) => x == y,
                        (Value::Null, Value::Null) => true,
                        (Value::Function { addr: addr1, params: params1 }, Value::Function { addr: addr2, params: params2 }) => {
                            addr1 == addr2 && params1 == params2
                        },
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "values of the same type".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "EQ".to_string() 
                        }),
                    };
                    self.stack.push(Value::Int(if result { 1 } else { 0 }));
                }
                OpCode::Gt => {
                    let b = self.pop_stack("GT")?;
                    let a = self.pop_stack("GT")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x > y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GT".to_string() 
                        }),
                    }
                }
                OpCode::Lt => {
                    let b = self.pop_stack("LT")?;
                    let a = self.pop_stack("LT")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x < y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LT".to_string() 
                        }),
                    }
                }
                OpCode::Ne => {
                    let b = self.pop_stack("NE")?;
                    let a = self.pop_stack("NE")?;
                    let result = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => x != y,
                        (Value::Float(x), Value::Float(y)) => (x - y).abs() >= f64::EPSILON,
                        (Value::Str(x), Value::Str(y)) => x != y,
                        (Value::Bool(x), Value::Bool(y)) => x != y,
                        (Value::Null, Value::Null) => false,
                        (Value::Function { addr: addr1, params: params1 }, Value::Function { addr: addr2, params: params2 }) => {
                            addr1 != addr2 || params1 != params2
                        },
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "values of the same type".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "NE".to_string() 
                        }),
                    };
                    self.stack.push(Value::Int(if result { 1 } else { 0 }));
                }
                OpCode::Ge => {
                    let b = self.pop_stack("GE")?;
                    let a = self.pop_stack("GE")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x >= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GE".to_string() 
                        }),
                    }
                }
                OpCode::Le => {
                    let b = self.pop_stack("LE")?;
                    let a = self.pop_stack("LE")?;
                    match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => {
                            self.stack.push(Value::Int(if x <= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two integers".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LE".to_string() 
                        }),
                    }
                }
                OpCode::EqF => {
                    let b = self.pop_stack("EQ_F")?;
                    let a = self.pop_stack("EQ_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if (x - y).abs() < f64::EPSILON { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "EQ_F".to_string() 
                        }),
                    }
                }
                OpCode::NeF => {
                    let b = self.pop_stack("NE_F")?;
                    let a = self.pop_stack("NE_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if (x - y).abs() >= f64::EPSILON { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "NE_F".to_string() 
                        }),
                    }
                }
                OpCode::GtF => {
                    let b = self.pop_stack("GT_F")?;
                    let a = self.pop_stack("GT_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x > y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GT_F".to_string() 
                        }),
                    }
                }
                OpCode::LtF => {
                    let b = self.pop_stack("LT_F")?;
                    let a = self.pop_stack("LT_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x < y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LT_F".to_string() 
                        }),
                    }
                }
                OpCode::GeF => {
                    let b = self.pop_stack("GE_F")?;
                    let a = self.pop_stack("GE_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x >= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "GE_F".to_string() 
                        }),
                    }
                }
                OpCode::LeF => {
                    let b = self.pop_stack("LE_F")?;
                    let a = self.pop_stack("LE_F")?;
                    match (&a, &b) {
                        (Value::Float(x), Value::Float(y)) => {
                            self.stack.push(Value::Int(if x <= y { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two floats".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "LE_F".to_string() 
                        }),
                    }
                }
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::False => self.stack.push(Value::Bool(false)),
                OpCode::Not => {
                    let val = self.pop_stack("NOT")?;
                    let result = match val {
                        Value::Bool(b) => !b,
                        Value::Int(i) => i == 0,
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "Bool or Int".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "NOT".to_string() 
                        }),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::And => {
                    let b = self.pop_stack("AND")?;
                    let a = self.pop_stack("AND")?;
                    let result = match (&a, &b) {
                        (Value::Bool(x), Value::Bool(y)) => *x && *y,
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two Booleans".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "AND".to_string() 
                        }),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::Or => {
                    let b = self.pop_stack("OR")?;
                    let a = self.pop_stack("OR")?;
                    let result = match (&a, &b) {
                        (Value::Bool(x), Value::Bool(y)) => *x || *y,
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "two Booleans".to_string(), 
                            got: format!("{:?}, {:?}", a, b), 
                            operation: "OR".to_string() 
                        }),
                    };
                    self.stack.push(Value::Bool(result));
                }
                OpCode::Null => {
                    self.stack.push(Value::Null);
                }
                OpCode::MakeList(n) => {
                    self.check_stack_size(*n, "MAKE_LIST")?;
                    let mut list = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        list.push(self.pop_stack("MAKE_LIST")?);
                    }
                    list.reverse();
                    self.stack.push(Value::List(list));
                }
                OpCode::Len => {
                    let val = self.pop_stack("LEN")?;
                    match val {
                        Value::List(l) => self.stack.push(Value::Int(l.len() as i64)),
                        Value::Object(o) => self.stack.push(Value::Int(o.len() as i64)),
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a list or object".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "LEN".to_string() 
                        }),
                    }
                }
                OpCode::Index => {
                    let index = match self.pop_stack("INDEX")? {
                        Value::Int(i) => i as usize,
                        val => return Err(VMError::TypeMismatch { 
                            expected: "an integer index".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "INDEX".to_string() 
                        }),
                    };
                    let list = match self.pop_stack("INDEX")? {
                        Value::List(l) => l,
                        val => return Err(VMError::TypeMismatch { 
                            expected: "a list".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "INDEX".to_string() 
                        }),
                    };
                    if index >= list.len() {
                        return Err(VMError::IndexOutOfBounds { index, length: list.len() });
                    }
                    self.stack.push(list[index].clone());
                }
                OpCode::MakeObject => {
                    let obj = HashMap::new();
                    self.stack.push(Value::Object(obj));
                }
                OpCode::SetField(field_name) => {
                    let value = self.pop_stack("SET_FIELD")?;
                    let obj = self.pop_stack("SET_FIELD")?;
                    match obj {
                        Value::Object(mut map) => {
                            map.insert(field_name.clone(), value);
                            self.stack.push(Value::Object(map));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "SET_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::GetField(field_name) => {
                    let obj = self.pop_stack("GET_FIELD")?;
                    match obj {
                        Value::Object(map) => {
                            let value = map.get(field_name).cloned().unwrap_or(Value::Null);
                            self.stack.push(value);
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "GET_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::HasField(field_name) => {
                    let obj = self.pop_stack("HAS_FIELD")?;
                    match obj {
                        Value::Object(map) => {
                            let has_field = map.contains_key(field_name);
                            self.stack.push(Value::Int(if has_field { 1 } else { 0 }));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "HAS_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::DeleteField(field_name) => {
                    let obj = self.pop_stack("DELETE_FIELD")?;
                    match obj {
                        Value::Object(mut map) => {
                            map.remove(field_name);
                            self.stack.push(Value::Object(map));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "DELETE_FIELD".to_string() 
                        }),
                    }
                }
                OpCode::Keys => {
                    let obj = self.pop_stack("KEYS")?;
                    match obj {
                        Value::Object(map) => {
                            let keys: Vec<Value> = map.keys().map(|k| Value::Str(k.clone())).collect();
                            self.stack.push(Value::List(keys));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "an object".to_string(), 
                            got: format!("{:?}", obj), 
                            operation: "KEYS".to_string() 
                        }),
                    }
                }
                OpCode::MakeFunction { addr, params } => {
                    let function = Value::Function { addr: *addr, params: params.clone() };
                    self.stack.push(function);
                }
                OpCode::MakeLambda { addr, params } => {
                    // Create closure with currently captured variables
                    let closure = Value::Closure { 
                        addr: *addr, 
                        params: params.clone(),
                        captured: self.lambda_captures.clone() 
                    };
                    self.stack.push(closure);
                    
                    // Clear captures for next lambda
                    self.lambda_captures.clear();
                }
                OpCode::Capture(var_name) => {
                    // Capture current value of variable for lambda
                    let value = self.get_variable(var_name)?.clone();
                    self.lambda_captures.insert(var_name.clone(), value);
                }
                OpCode::CallFunction => {
                    let function = self.pop_stack("CALL_FUNCTION")?;
                    match function {
                        Value::Function { addr, params } => {
                            // Check if we have enough arguments on the stack
                            self.check_stack_size(params.len(), "CALL_FUNCTION")?;
                            
                            // Save return address
                            self.call_stack.push(self.ip + 1);
                            
                            // Create new variable frame for function parameters
                            let mut frame = HashMap::new();
                            for name in params.iter().rev() {
                                let value = self.pop_stack("CALL_FUNCTION")?;
                                frame.insert(name.clone(), value);
                            }
                            self.variables.push(frame);
                            
                            // Jump to function
                            self.ip = addr;
                        }
                        Value::Closure { addr, params, captured } => {
                            // Check if we have enough arguments on the stack
                            self.check_stack_size(params.len(), "CALL_FUNCTION")?;
                            
                            // Save return address
                            self.call_stack.push(self.ip + 1);
                            
                            // Create new variable frame with captured variables and parameters
                            let mut frame = captured; // Start with captured environment
                            for name in params.iter().rev() {
                                let value = self.pop_stack("CALL_FUNCTION")?;
                                frame.insert(name.clone(), value); // Parameters override captured vars
                            }
                            self.variables.push(frame);
                            
                            // Jump to closure body
                            self.ip = addr;
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a function or closure".to_string(), 
                            got: format!("{:?}", function), 
                            operation: "CALL_FUNCTION".to_string() 
                        }),
                    }
                }
                OpCode::ReadFile => {
                    let val = self.pop_stack("READ_file")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::read_to_string(&filename) {
                                Ok(content) => self.stack.push(Value::Str(content)),
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "a string filename".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "READ_FILE".to_string() 
                        }),
                    }
                }
                OpCode::WriteFile => {
                    let filename = self.pop_stack("WRITE_FILE")?;
                    let content = self.pop_stack("WRITE_FILE")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(body)) => {
                            if let Err(e) = std::fs::write(&fname, &body) {
                                return Err(VMError::FileError { 
                                    filename: fname, 
                                    error: e.to_string() 
                                });
                            }
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (filename, content)".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "WRITE_FILE".to_string() 
                        }),
                    }
                }
                // Enhanced I/O operations
                OpCode::ReadLine => {
                    use std::io::{self, BufRead};
                    let stdin = io::stdin();
                    let mut line = String::new();
                    match stdin.lock().read_line(&mut line) {
                        Ok(_) => {
                            // Remove trailing newline
                            if line.ends_with('\n') {
                                line.pop();
                                if line.ends_with('\r') {
                                    line.pop();
                                }
                            }
                            self.stack.push(Value::Str(line));
                        }
                        Err(e) => return Err(VMError::FileError { 
                            filename: "stdin".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::ReadChar => {
                    use std::io::{self, Read};
                    let mut stdin = io::stdin();
                    let mut buffer = [0; 1];
                    match stdin.read_exact(&mut buffer) {
                        Ok(_) => {
                            let ch = buffer[0] as char;
                            self.stack.push(Value::Str(ch.to_string()));
                        }
                        Err(e) => return Err(VMError::FileError { 
                            filename: "stdin".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::ReadInput => {
                    use std::io::{self, Read};
                    let mut stdin = io::stdin();
                    let mut buffer = String::new();
                    match stdin.read_to_string(&mut buffer) {
                        Ok(_) => self.stack.push(Value::Str(buffer)),
                        Err(e) => return Err(VMError::FileError { 
                            filename: "stdin".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::AppendFile => {
                    let filename = self.pop_stack("APPEND_FILE")?;
                    let content = self.pop_stack("APPEND_FILE")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(body)) => {
                            use std::fs::OpenOptions;
                            use std::io::Write;
                            match OpenOptions::new().create(true).append(true).open(&fname) {
                                Ok(mut file) => {
                                    if let Err(e) = file.write_all(body.as_bytes()) {
                                        return Err(VMError::FileError { 
                                            filename: fname, 
                                            error: e.to_string() 
                                        });
                                    }
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: fname, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (filename, content)".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "APPEND_FILE".to_string() 
                        }),
                    }
                }
                OpCode::FileExists => {
                    let val = self.pop_stack("FILE_EXISTS")?;
                    match val {
                        Value::Str(filename) => {
                            let exists = std::path::Path::new(&filename).exists();
                            self.stack.push(Value::Bool(exists));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "FILE_EXISTS".to_string() 
                        }),
                    }
                }
                OpCode::FileSize => {
                    let val = self.pop_stack("FILE_SIZE")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::metadata(&filename) {
                                Ok(metadata) => self.stack.push(Value::Int(metadata.len() as i64)),
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "FILE_SIZE".to_string() 
                        }),
                    }
                }
                OpCode::DeleteFile => {
                    let val = self.pop_stack("DELETE_FILE")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::remove_file(&filename) {
                                Ok(_) => {}
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DELETE_FILE".to_string() 
                        }),
                    }
                }
                OpCode::ListDir => {
                    let val = self.pop_stack("LIST_DIR")?;
                    match val {
                        Value::Str(dirname) => {
                            match std::fs::read_dir(&dirname) {
                                Ok(entries) => {
                                    let mut files = Vec::new();
                                    for entry in entries {
                                        match entry {
                                            Ok(entry) => {
                                                if let Some(name) = entry.file_name().to_str() {
                                                    files.push(Value::Str(name.to_string()));
                                                }
                                            }
                                            Err(e) => return Err(VMError::FileError { 
                                                filename: dirname, 
                                                error: e.to_string() 
                                            }),
                                        }
                                    }
                                    self.stack.push(Value::List(files));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: dirname, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (directory name)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "LIST_DIR".to_string() 
                        }),
                    }
                }
                OpCode::ReadBytes => {
                    let val = self.pop_stack("READ_bytes")?;
                    match val {
                        Value::Str(filename) => {
                            match std::fs::read(&filename) {
                                Ok(bytes) => self.stack.push(Value::Bytes(bytes)),
                                Err(e) => return Err(VMError::FileError { 
                                    filename, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "read_bytes".to_string() 
                        }),
                    }
                }
                OpCode::WriteBytes => {
                    let filename = self.pop_stack("WRITE_BYTES")?;
                    let content = self.pop_stack("WRITE_BYTES")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Bytes(bytes)) => {
                            if let Err(e) = std::fs::write(&fname, &bytes) {
                                return Err(VMError::FileError { 
                                    filename: fname, 
                                    error: e.to_string() 
                                });
                            }
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "string (filename) and bytes".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "WRITE_BYTES".to_string() 
                        }),
                    }
                }
                // Environment and system operations
                OpCode::GetEnv => {
                    let val = self.pop_stack("GET_ENV")?;
                    match val {
                        Value::Str(var_name) => {
                            match std::env::var(&var_name) {
                                Ok(value) => self.stack.push(Value::Str(value)),
                                Err(_) => self.stack.push(Value::Null),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (variable name)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "GET_ENV".to_string() 
                        }),
                    }
                }
                OpCode::SetEnv => {
                    let value = self.pop_stack("SET_ENV")?;
                    let var_name = self.pop_stack("SET_ENV")?;
                    match (var_name, value) {
                        (Value::Str(name), Value::Str(val)) => {
                            std::env::set_var(&name, &val);
                        }
                        (n, v) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (variable name, value)".to_string(), 
                            got: format!("{:?}, {:?}", n, v), 
                            operation: "SET_ENV".to_string() 
                        }),
                    }
                }
                OpCode::GetArgs => {
                    let args: Vec<Value> = std::env::args()
                        .map(|arg| Value::Str(arg))
                        .collect();
                    self.stack.push(Value::List(args));
                }
                OpCode::Exec => {
                    let args = self.pop_stack("EXEC")?;
                    let command = self.pop_stack("EXEC")?;
                    match (command, args) {
                        (Value::Str(cmd), Value::List(arg_list)) => {
                            use std::process::Command;
                            let mut cmd_obj = Command::new(&cmd);
                            for arg in arg_list {
                                if let Value::Str(arg_str) = arg {
                                    cmd_obj.arg(arg_str);
                                } else {
                                    return Err(VMError::TypeMismatch { 
                                        expected: "list of strings (arguments)".to_string(), 
                                        got: format!("{:?}", arg), 
                                        operation: "EXEC".to_string() 
                                    });
                                }
                            }
                            match cmd_obj.status() {
                                Ok(status) => {
                                    let exit_code = status.code().unwrap_or(-1);
                                    self.stack.push(Value::Int(exit_code as i64));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: cmd, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (c, a) => return Err(VMError::TypeMismatch { 
                            expected: "string (command) and list (arguments)".to_string(), 
                            got: format!("{:?}, {:?}", c, a), 
                            operation: "EXEC".to_string() 
                        }),
                    }
                }
                OpCode::ExecCapture => {
                    let args = self.pop_stack("EXEC_CAPTURE")?;
                    let command = self.pop_stack("EXEC_CAPTURE")?;
                    match (command, args) {
                        (Value::Str(cmd), Value::List(arg_list)) => {
                            use std::process::Command;
                            let mut cmd_obj = Command::new(&cmd);
                            for arg in arg_list {
                                if let Value::Str(arg_str) = arg {
                                    cmd_obj.arg(arg_str);
                                } else {
                                    return Err(VMError::TypeMismatch { 
                                        expected: "list of strings (arguments)".to_string(), 
                                        got: format!("{:?}", arg), 
                                        operation: "EXEC_CAPTURE".to_string() 
                                    });
                                }
                            }
                            match cmd_obj.output() {
                                Ok(output) => {
                                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                    let exit_code = output.status.code().unwrap_or(-1);
                                    
                                    // Create result object
                                    let mut result = HashMap::new();
                                    result.insert("stdout".to_string(), Value::Str(stdout));
                                    result.insert("stderr".to_string(), Value::Str(stderr));
                                    result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
                                    self.stack.push(Value::Object(result));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: cmd, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (c, a) => return Err(VMError::TypeMismatch { 
                            expected: "string (command) and list (arguments)".to_string(), 
                            got: format!("{:?}, {:?}", c, a), 
                            operation: "EXEC_CAPTURE".to_string() 
                        }),
                    }
                }
                OpCode::Exit => {
                    let val = self.pop_stack("EXIT")?;
                    match val {
                        Value::Int(code) => {
                            std::process::exit(code as i32);
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int (exit code)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "EXIT".to_string() 
                        }),
                    }
                }
                // Time operations
                OpCode::GetTime => {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    match SystemTime::now().duration_since(UNIX_EPOCH) {
                        Ok(duration) => {
                            self.stack.push(Value::Int(duration.as_secs() as i64));
                        }
                        Err(e) => return Err(VMError::FileError { 
                            filename: "system_time".to_string(), 
                            error: e.to_string() 
                        }),
                    }
                }
                OpCode::Sleep => {
                    let val = self.pop_stack("SLEEP")?;
                    match val {
                        Value::Int(millis) => {
                            let duration = std::time::Duration::from_millis(millis as u64);
                            std::thread::sleep(duration);
                        }
                        Value::Float(millis) => {
                            let duration = std::time::Duration::from_millis(millis as u64);
                            std::thread::sleep(duration);
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int or float (milliseconds)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "SLEEP".to_string() 
                        }),
                    }
                }
                OpCode::FormatTime => {
                    let format_str = self.pop_stack("FORMAT_TIME")?;
                    let timestamp = self.pop_stack("FORMAT_TIME")?;
                    match (timestamp, format_str) {
                        (Value::Int(ts), Value::Str(_format)) => {
                            // Simplified time formatting - just return ISO format
                            use std::time::UNIX_EPOCH;
                            let _system_time = UNIX_EPOCH + Duration::from_secs(ts as u64);
                            // For simplicity, just return the timestamp as string
                            // In a real implementation, we'd use chrono or similar for formatting
                            self.stack.push(Value::Str(format!("{}", ts)));
                        }
                        (t, f) => return Err(VMError::TypeMismatch { 
                            expected: "int (timestamp) and string (format)".to_string(), 
                            got: format!("{:?}, {:?}", t, f), 
                            operation: "FORMAT_TIME".to_string() 
                        }),
                    }
                }
                // Network operations
                OpCode::HttpGet => {
                    let val = self.pop_stack("HTTP_GET")?;
                    match val {
                        Value::Str(url) => {
                            // Simplified HTTP GET using std library (in real implementation would use reqwest)
                            // For now, just return a placeholder response
                            let response = format!("HTTP response from {}", url);
                            self.stack.push(Value::Str(response));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (URL)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "HTTP_GET".to_string() 
                        }),
                    }
                }
                OpCode::HttpPost => {
                    let data = self.pop_stack("HTTP_POST")?;
                    let url = self.pop_stack("HTTP_POST")?;
                    match (url, data) {
                        (Value::Str(url_str), Value::Str(data_str)) => {
                            // Simplified HTTP POST (in real implementation would use reqwest)
                            let response = format!("HTTP POST to {} with data: {}", url_str, data_str);
                            self.stack.push(Value::Str(response));
                        }
                        (u, d) => return Err(VMError::TypeMismatch { 
                            expected: "two strings (URL, data)".to_string(), 
                            got: format!("{:?}, {:?}", u, d), 
                            operation: "HTTP_POST".to_string() 
                        }),
                    }
                }
                OpCode::TcpConnect => {
                    let port = self.pop_stack("TCP_CONNECT")?;
                    let host = self.pop_stack("TCP_CONNECT")?;
                    match (host, port) {
                        (Value::Str(host_str), Value::Int(port_num)) => {
                            // Simplified TCP connect - in real implementation would create actual socket
                            use std::net::TcpStream;
                            let address = format!("{}:{}", host_str, port_num);
                            match TcpStream::connect(&address) {
                                Ok(_stream) => {
                                    // In real implementation, we'd store the stream
                                    // For now, just return a connection ID
                                    let conn_id = format!("tcp://{}:{}", host_str, port_num);
                                    self.stack.push(Value::Connection(conn_id));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: address, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        (h, p) => return Err(VMError::TypeMismatch { 
                            expected: "string (host) and int (port)".to_string(), 
                            got: format!("{:?}, {:?}", h, p), 
                            operation: "TCP_CONNECT".to_string() 
                        }),
                    }
                }
                OpCode::TcpListen => {
                    let val = self.pop_stack("TCP_LISTEN")?;
                    match val {
                        Value::Int(port) => {
                            // Simplified TCP listen - in real implementation would bind and listen
                            use std::net::TcpListener;
                            let address = format!("127.0.0.1:{}", port);
                            match TcpListener::bind(&address) {
                                Ok(_listener) => {
                                    // In real implementation, we'd store the listener
                                    let conn_id = format!("tcp://listener:{}", port);
                                    self.stack.push(Value::Connection(conn_id));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: address, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int (port)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "TCP_LISTEN".to_string() 
                        }),
                    }
                }
                OpCode::TcpSend => {
                    let data = self.pop_stack("TCP_SEND")?;
                    let conn = self.pop_stack("TCP_SEND")?;
                    match (conn, data) {
                        (Value::Connection(conn_id), Value::Str(data_str)) => {
                            // Simplified TCP send - in real implementation would send via actual socket
                            println!("TCP Send to {}: {}", conn_id, data_str);
                            self.stack.push(Value::Int(data_str.len() as i64));
                        }
                        (Value::Connection(conn_id), Value::Bytes(data_bytes)) => {
                            // Send binary data
                            println!("TCP Send to {}: {} bytes", conn_id, data_bytes.len());
                            self.stack.push(Value::Int(data_bytes.len() as i64));
                        }
                        (c, d) => return Err(VMError::TypeMismatch { 
                            expected: "connection and string/bytes".to_string(), 
                            got: format!("{:?}, {:?}", c, d), 
                            operation: "TCP_SEND".to_string() 
                        }),
                    }
                }
                OpCode::TcpRecv => {
                    let size = self.pop_stack("TCP_RECV")?;
                    let conn = self.pop_stack("TCP_RECV")?;
                    match (conn, size) {
                        (Value::Connection(conn_id), Value::Int(buffer_size)) => {
                            // Simplified TCP recv - in real implementation would receive from actual socket
                            let received_data = format!("Data from {}", conn_id);
                            if buffer_size > 0 {
                                self.stack.push(Value::Str(received_data));
                            } else {
                                self.stack.push(Value::Bytes(vec![1, 2, 3, 4])); // Mock binary data
                            }
                        }
                        (c, s) => return Err(VMError::TypeMismatch { 
                            expected: "connection and int (buffer size)".to_string(), 
                            got: format!("{:?}, {:?}", c, s), 
                            operation: "TCP_RECV".to_string() 
                        }),
                    }
                }
                OpCode::UdpBind => {
                    let val = self.pop_stack("UDP_BIND")?;
                    match val {
                        Value::Int(port) => {
                            // Simplified UDP bind - in real implementation would bind UDP socket
                            use std::net::UdpSocket;
                            let address = format!("127.0.0.1:{}", port);
                            match UdpSocket::bind(&address) {
                                Ok(_socket) => {
                                    let conn_id = format!("udp://bind:{}", port);
                                    self.stack.push(Value::Connection(conn_id));
                                }
                                Err(e) => return Err(VMError::FileError { 
                                    filename: address, 
                                    error: e.to_string() 
                                }),
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "int (port)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "UDP_BIND".to_string() 
                        }),
                    }
                }
                OpCode::UdpSend => {
                    let data = self.pop_stack("UDP_SEND")?;
                    let port = self.pop_stack("UDP_SEND")?;
                    let host = self.pop_stack("UDP_SEND")?;
                    let socket = self.pop_stack("UDP_SEND")?;
                    match (socket, host, port, data) {
                        (Value::Connection(conn_id), Value::Str(host_str), Value::Int(port_num), Value::Str(data_str)) => {
                            // Simplified UDP send
                            println!("UDP Send from {} to {}:{}: {}", conn_id, host_str, port_num, data_str);
                            self.stack.push(Value::Int(data_str.len() as i64));
                        }
                        (s, h, p, d) => return Err(VMError::TypeMismatch { 
                            expected: "connection, string (host), int (port), string (data)".to_string(), 
                            got: format!("{:?}, {:?}, {:?}, {:?}", s, h, p, d), 
                            operation: "UDP_SEND".to_string() 
                        }),
                    }
                }
                OpCode::UdpRecv => {
                    let size = self.pop_stack("UDP_RECV")?;
                    let socket = self.pop_stack("UDP_RECV")?;
                    match (socket, size) {
                        (Value::Connection(_conn_id), Value::Int(_buffer_size)) => {
                            // Simplified UDP recv - return mock data and sender info
                            let mut result = HashMap::new();
                            result.insert("data".to_string(), Value::Str("UDP packet data".to_string()));
                            result.insert("sender_host".to_string(), Value::Str("192.168.1.100".to_string()));
                            result.insert("sender_port".to_string(), Value::Int(12345));
                            self.stack.push(Value::Object(result));
                        }
                        (s, sz) => return Err(VMError::TypeMismatch { 
                            expected: "connection and int (buffer size)".to_string(), 
                            got: format!("{:?}, {:?}", s, sz), 
                            operation: "UDP_RECV".to_string() 
                        }),
                    }
                }
                OpCode::DnsResolve => {
                    let val = self.pop_stack("DNS_RESOLVE")?;
                    match val {
                        Value::Str(hostname) => {
                            // Simplified DNS resolution using std library
                            use std::net::ToSocketAddrs;
                            let address_with_port = format!("{}:80", hostname); // Add dummy port for resolution
                            match address_with_port.to_socket_addrs() {
                                Ok(mut addrs) => {
                                    if let Some(addr) = addrs.next() {
                                        let ip = addr.ip().to_string();
                                        self.stack.push(Value::Str(ip));
                                    } else {
                                        self.stack.push(Value::Null);
                                    }
                                }
                                Err(_) => {
                                    // Return mock IP for demonstration
                                    self.stack.push(Value::Str("192.168.1.1".to_string()));
                                }
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (hostname)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DNS_RESOLVE".to_string() 
                        }),
                    }
                }
                // Advanced I/O operations
                OpCode::AsyncRead => {
                    let val = self.pop_stack("ASYNC_READ")?;
                    match val {
                        Value::Str(filename) => {
                            // Simplified async read - in real implementation would use tokio or async-std
                            let future_id = format!("async_read:{}", filename);
                            self.stack.push(Value::Future(future_id));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (filename)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "ASYNC_READ".to_string() 
                        }),
                    }
                }
                OpCode::AsyncWrite => {
                    let filename = self.pop_stack("ASYNC_WRITE")?;
                    let content = self.pop_stack("ASYNC_WRITE")?;
                    match (filename, content) {
                        (Value::Str(fname), Value::Str(data)) => {
                            // Simplified async write - encode filename and content in future ID
                            let future_id = format!("async_write:{}:{}", fname, data);
                            self.stack.push(Value::Future(future_id));
                        }
                        (f, c) => return Err(VMError::TypeMismatch { 
                            expected: "string (filename) and string (content)".to_string(), 
                            got: format!("{:?}, {:?}", f, c), 
                            operation: "ASYNC_WRITE".to_string() 
                        }),
                    }
                }
                OpCode::Await => {
                    let val = self.pop_stack("AWAIT")?;
                    match val {
                        Value::Future(future_id) => {
                            // Simplified await - simulate completion
                            if future_id.starts_with("async_read:") {
                                let filename = future_id.strip_prefix("async_read:").unwrap_or("unknown");
                                // Simulate reading file
                                match std::fs::read_to_string(filename) {
                                    Ok(content) => self.stack.push(Value::Str(content)),
                                    Err(e) => return Err(VMError::FileError { 
                                        filename: filename.to_string(), 
                                        error: e.to_string() 
                                    }),
                                }
                            } else if future_id.starts_with("async_write:") {
                                // Parse the async_write future format: "async_write:filename:content"
                                let content_part = future_id.strip_prefix("async_write:").unwrap_or("");
                                if let Some(separator_index) = content_part.find(':') {
                                    let filename = &content_part[..separator_index];
                                    let data = &content_part[separator_index + 1..];
                                    match std::fs::write(filename, data) {
                                        Ok(()) => self.stack.push(Value::Bool(true)),
                                        Err(e) => return Err(VMError::FileError { 
                                            filename: filename.to_string(), 
                                            error: e.to_string() 
                                        }),
                                    }
                                } else {
                                    self.stack.push(Value::Bool(true));
                                }
                            } else {
                                self.stack.push(Value::Null);
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "future".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "AWAIT".to_string() 
                        }),
                    }
                }
                OpCode::StreamCreate => {
                    let val = self.pop_stack("STREAM_CREATE")?;
                    match val {
                        Value::Str(stream_type) => {
                            let stream_id = format!("stream:{}:{}", stream_type, self.instruction_count);
                            self.stack.push(Value::Stream(stream_id));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (stream type)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "STREAM_CREATE".to_string() 
                        }),
                    }
                }
                OpCode::StreamRead => {
                    let size = self.pop_stack("STREAM_READ")?;
                    let stream = self.pop_stack("STREAM_READ")?;
                    match (stream, size) {
                        (Value::Stream(_stream_id), Value::Int(read_size)) => {
                            // Simplified stream read
                            let data = format!("stream_data_{}", read_size);
                            self.stack.push(Value::Str(data));
                        }
                        (s, sz) => return Err(VMError::TypeMismatch { 
                            expected: "stream and int (size)".to_string(), 
                            got: format!("{:?}, {:?}", s, sz), 
                            operation: "STREAM_READ".to_string() 
                        }),
                    }
                }
                OpCode::StreamWrite => {
                    let data = self.pop_stack("STREAM_WRITE")?;
                    let stream = self.pop_stack("STREAM_WRITE")?;
                    match (stream, data) {
                        (Value::Stream(_stream_id), Value::Str(write_data)) => {
                            // Simplified stream write
                            self.stack.push(Value::Int(write_data.len() as i64));
                        }
                        (Value::Stream(_stream_id), Value::Bytes(write_bytes)) => {
                            // Write binary data to stream
                            self.stack.push(Value::Int(write_bytes.len() as i64));
                        }
                        (s, d) => return Err(VMError::TypeMismatch { 
                            expected: "stream and string/bytes".to_string(), 
                            got: format!("{:?}, {:?}", s, d), 
                            operation: "STREAM_WRITE".to_string() 
                        }),
                    }
                }
                OpCode::StreamClose => {
                    let val = self.pop_stack("STREAM_CLOSE")?;
                    match val {
                        Value::Stream(_stream_id) => {
                            // Simplified stream close
                            self.stack.push(Value::Bool(true));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "stream".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "STREAM_CLOSE".to_string() 
                        }),
                    }
                }
                OpCode::JsonParse => {
                    let val = self.pop_stack("JSON_PARSE")?;
                    match val {
                        Value::Str(json_str) => {
                            // Simplified JSON parsing - in real implementation would use serde_json
                            if json_str.starts_with('{') && json_str.ends_with('}') {
                                let mut obj = HashMap::new();
                                obj.insert("parsed".to_string(), Value::Bool(true));
                                obj.insert("data".to_string(), Value::Str("json_data".to_string()));
                                self.stack.push(Value::Object(obj));
                            } else if json_str.starts_with('[') && json_str.ends_with(']') {
                                let list = vec![
                                    Value::Str("item1".to_string()),
                                    Value::Str("item2".to_string()),
                                ];
                                self.stack.push(Value::List(list));
                            } else {
                                self.stack.push(Value::Str(json_str));
                            }
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (JSON)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "JSON_PARSE".to_string() 
                        }),
                    }
                }
                OpCode::JsonStringify => {
                    let val = self.pop_stack("JSON_STRINGIFY")?;
                    match val {
                        Value::Object(_) => {
                            // Simplified JSON stringification
                            self.stack.push(Value::Str("{\"key\":\"value\"}".to_string()));
                        }
                        Value::List(_) => {
                            self.stack.push(Value::Str("[\"item1\",\"item2\"]".to_string()));
                        }
                        Value::Str(s) => {
                            self.stack.push(Value::Str(format!("\"{}\"", s)));
                        }
                        Value::Int(n) => {
                            self.stack.push(Value::Str(n.to_string()));
                        }
                        Value::Bool(b) => {
                            self.stack.push(Value::Str(b.to_string()));
                        }
                        Value::Null => {
                            self.stack.push(Value::Str("null".to_string()));
                        }
                        _ => {
                            self.stack.push(Value::Str("{}".to_string()));
                        }
                    }
                }
                OpCode::CsvParse => {
                    let val = self.pop_stack("CSV_PARSE")?;
                    match val {
                        Value::Str(csv_str) => {
                            // Simplified CSV parsing
                            let rows: Vec<Value> = csv_str.lines()
                                .map(|line| {
                                    let columns: Vec<Value> = line.split(',')
                                        .map(|col| Value::Str(col.trim().to_string()))
                                        .collect();
                                    Value::List(columns)
                                })
                                .collect();
                            self.stack.push(Value::List(rows));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (CSV)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "CSV_PARSE".to_string() 
                        }),
                    }
                }
                OpCode::CsvWrite => {
                    let val = self.pop_stack("CSV_WRITE")?;
                    match val {
                        Value::List(rows) => {
                            // Simplified CSV writing
                            let mut csv_output = String::new();
                            for (i, row) in rows.iter().enumerate() {
                                if i > 0 {
                                    csv_output.push('\n');
                                }
                                if let Value::List(columns) = row {
                                    for (j, col) in columns.iter().enumerate() {
                                        if j > 0 {
                                            csv_output.push(',');
                                        }
                                        csv_output.push_str(&format!("{}", col));
                                    }
                                }
                            }
                            self.stack.push(Value::Str(csv_output));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "list of lists (CSV data)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "CSV_WRITE".to_string() 
                        }),
                    }
                }
                OpCode::Compress => {
                    let val = self.pop_stack("COMPRESS")?;
                    match val {
                        Value::Str(data) => {
                            // Simplified compression - in real implementation would use flate2
                            let compressed = format!("compressed({})", data.len());
                            self.stack.push(Value::Bytes(compressed.into_bytes()));
                        }
                        Value::Bytes(data) => {
                            let compressed = format!("compressed({})", data.len());
                            self.stack.push(Value::Bytes(compressed.into_bytes()));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string or bytes".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "COMPRESS".to_string() 
                        }),
                    }
                }
                OpCode::Decompress => {
                    let val = self.pop_stack("DECOMPRESS")?;
                    match val {
                        Value::Bytes(data) => {
                            // Simplified decompression
                            let decompressed = format!("decompressed:{}", String::from_utf8_lossy(&data));
                            self.stack.push(Value::Str(decompressed));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "bytes (compressed data)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DECOMPRESS".to_string() 
                        }),
                    }
                }
                OpCode::Encrypt => {
                    let key = self.pop_stack("ENCRYPT")?;
                    let data = self.pop_stack("ENCRYPT")?;
                    match (data, key) {
                        (Value::Str(plaintext), Value::Str(encryption_key)) => {
                            // Simplified encryption - in real implementation would use proper crypto
                            let encrypted = format!("encrypted:{}:key:{}", plaintext.len(), encryption_key.len());
                            self.stack.push(Value::Bytes(encrypted.into_bytes()));
                        }
                        (d, k) => return Err(VMError::TypeMismatch { 
                            expected: "string (data) and string (key)".to_string(), 
                            got: format!("{:?}, {:?}", d, k), 
                            operation: "ENCRYPT".to_string() 
                        }),
                    }
                }
                OpCode::Decrypt => {
                    let key = self.pop_stack("DECRYPT")?;
                    let data = self.pop_stack("DECRYPT")?;
                    match (data, key) {
                        (Value::Bytes(ciphertext), Value::Str(_decryption_key)) => {
                            // Simplified decryption
                            let decrypted = format!("decrypted:{}", String::from_utf8_lossy(&ciphertext));
                            self.stack.push(Value::Str(decrypted));
                        }
                        (d, k) => return Err(VMError::TypeMismatch { 
                            expected: "bytes (encrypted data) and string (key)".to_string(), 
                            got: format!("{:?}, {:?}", d, k), 
                            operation: "DECRYPT".to_string() 
                        }),
                    }
                }
                OpCode::Hash => {
                    let val = self.pop_stack("HASH")?;
                    match val {
                        Value::Str(data) => {
                            // Simplified hashing - in real implementation would use sha2, md5, etc.
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut hasher = DefaultHasher::new();
                            data.hash(&mut hasher);
                            let hash_value = hasher.finish();
                            self.stack.push(Value::Str(format!("{:x}", hash_value)));
                        }
                        Value::Bytes(data) => {
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut hasher = DefaultHasher::new();
                            data.hash(&mut hasher);
                            let hash_value = hasher.finish();
                            self.stack.push(Value::Str(format!("{:x}", hash_value)));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string or bytes".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "HASH".to_string() 
                        }),
                    }
                }
                OpCode::DbConnect => {
                    let val = self.pop_stack("DB_CONNECT")?;
                    match val {
                        Value::Str(connection_string) => {
                            // Simplified database connection - in real implementation would use sqlx, rusqlite, etc.
                            let db_id = format!("db:{}", connection_string);
                            self.stack.push(Value::Connection(db_id));
                        }
                        _ => return Err(VMError::TypeMismatch { 
                            expected: "string (connection string)".to_string(), 
                            got: format!("{:?}", val), 
                            operation: "DB_CONNECT".to_string() 
                        }),
                    }
                }
                OpCode::DbQuery => {
                    let query = self.pop_stack("DB_QUERY")?;
                    let db = self.pop_stack("DB_QUERY")?;
                    match (db, query) {
                        (Value::Connection(_db_id), Value::Str(_sql_query)) => {
                            // Simplified database query
                            let mut result = HashMap::new();
                            result.insert("rows".to_string(), Value::Int(3));
                            result.insert("columns".to_string(), Value::List(vec![
                                Value::Str("id".to_string()),
                                Value::Str("name".to_string()),
                            ]));
                            result.insert("data".to_string(), Value::List(vec![
                                Value::List(vec![Value::Int(1), Value::Str("Alice".to_string())]),
                                Value::List(vec![Value::Int(2), Value::Str("Bob".to_string())]),
                                Value::List(vec![Value::Int(3), Value::Str("Charlie".to_string())]),
                            ]));
                            self.stack.push(Value::Object(result));
                        }
                        (d, q) => return Err(VMError::TypeMismatch { 
                            expected: "connection and string (SQL query)".to_string(), 
                            got: format!("{:?}, {:?}", d, q), 
                            operation: "DB_QUERY".to_string() 
                        }),
                    }
                }
                OpCode::DbExec => {
                    let command = self.pop_stack("DB_EXEC")?;
                    let db = self.pop_stack("DB_EXEC")?;
                    match (db, command) {
                        (Value::Connection(_db_id), Value::Str(_sql_command)) => {
                            // Simplified database execution
                            self.stack.push(Value::Int(1)); // affected rows
                        }
                        (d, c) => return Err(VMError::TypeMismatch { 
                            expected: "connection and string (SQL command)".to_string(), 
                            got: format!("{:?}, {:?}", d, c), 
                            operation: "DB_EXEC".to_string() 
                        }),
                    }
                }
                OpCode::DumpScope => {
                    println!("Current scope: {:?}", self.variables.last());
                }
                // Exception handling opcodes
                OpCode::Try { catch_addr } => {
                    self.push_exception_handler(*catch_addr);
                }
                OpCode::Catch => {
                    // The exception should already be on the stack from throw_exception
                    // Nothing to do here, just mark that we're in the catch block
                }
                OpCode::Throw => {
                    let exception_value = self.pop_stack("THROW")?;
                    
                    // Convert value to exception if it's not already one
                    let exception = match exception_value {
                        Value::Exception { .. } => exception_value,
                        Value::Str(msg) => Value::Exception { 
                            message: msg,
                            stack_trace: vec![format!("at instruction {}", self.ip)]
                        },
                        other => Value::Exception {
                            message: format!("Thrown value: {:?}", other),
                            stack_trace: vec![format!("at instruction {}", self.ip)]
                        }
                    };
                    
                    self.throw_exception(exception)?;
                }
                OpCode::EndTry => {
                    // Pop the exception handler when exiting try block normally
                    self.pop_exception_handler();
                }
                OpCode::Import(path) => {
                    self.import_module(path)?;
                }
                OpCode::Export(name) => {
                    self.export_symbol(name)?;
                }
                OpCode::Spawn => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("SPAWN not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Receive => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("RECEIVE not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Yield => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("YIELD not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Send(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("SEND not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Monitor(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("MONITOR not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Demonitor(_monitor_ref) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("DEMONITOR not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Link(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("LINK not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Unlink(_proc_id) => {
                    // For VM struct, not supported (use TinyProc instead)
                    return Err(VMError::UnsupportedOperation("UNLINK not supported in VM, use TinyProc scheduler".to_string()));
                }
                OpCode::Halt => {
                    // This should never be reached since HALT is handled in run()
                    unreachable!("HALT instruction should be handled in run() method")
                }
            }
            Ok(())
        }

    fn import_module(&mut self, path: &str) -> VMResult<()> {
        // Check for circular dependencies using global loading stack
        if self.loading_stack.contains(&path.to_string()) {
            return Err(VMError::FileError {
                filename: path.to_string(),
                error: "Circular dependency detected".to_string(),
            });
        }

        // Check if module is already loaded
        if let Some(exports) = self.loaded_modules.get(path).cloned() {
            // Module already loaded, import its exports into current scope
            for (name, value) in exports {
                self.set_variable(name, value)?;
            }
            return Ok(());
        }

        // Add to loading stack to detect circular dependencies
        self.loading_stack.push(path.to_string());

        // Load and parse the module
        let module_instructions = parse_program(path)?;
        
        // Merge module instructions into main VM's instruction space
        let base_addr = self.instructions.len();
        let mut adjusted_exports = HashMap::new();
        
        // Adjust addresses in module instructions and append to main instruction space
        let adjusted_instructions = module_instructions.iter()
            .map(|inst| self.adjust_instruction_addresses(inst, base_addr))
            .collect::<Vec<_>>();
        self.instructions.extend(adjusted_instructions);
        
        // Create a new VM with the module instructions to get exports
        // Share the loading context to detect circular dependencies
        let mut module_vm = VM::new(module_instructions);
        module_vm.debug_mode = self.debug_mode;
        module_vm.loading_stack = self.loading_stack.clone(); // Share loading context
        module_vm.loaded_modules = self.loaded_modules.clone(); // Share loaded modules
        
        // Run the module to generate exports
        module_vm.run()?;
        
        // Update our loaded modules with any new modules the sub-module loaded
        self.loaded_modules.extend(module_vm.loaded_modules);
        
        // Adjust function addresses in exports to point to merged instruction space
        for (name, value) in module_vm.exports {
            let adjusted_value = self.adjust_value_addresses(value, base_addr);
            adjusted_exports.insert(name, adjusted_value);
        }
        
        // Cache the loaded module
        self.loaded_modules.insert(path.to_string(), adjusted_exports.clone());
        
        // Import the exports into current scope
        if self.debug_mode {
            println!("Importing {} exports from module {}", adjusted_exports.len(), path);
        }
        for (name, value) in adjusted_exports {
            if self.debug_mode {
                println!("Importing export: {} = {:?}", name, value);
            }
            self.set_variable(name, value)?;
        }
        
        // Remove from loading stack
        self.loading_stack.pop();
        
        Ok(())
    }

    fn export_symbol(&mut self, name: &str) -> VMResult<()> {
        // Get the value from current scope
        let value = self.get_variable(name)?.clone();
        
        // Add to exports
        self.exports.insert(name.to_string(), value);
        
        Ok(())
    }

    fn adjust_value_addresses(&self, value: Value, base_addr: usize) -> Value {
        match value {
            Value::Function { addr, params } => {
                Value::Function { 
                    addr: addr + base_addr,
                    params 
                }
            }
            Value::Closure { addr, params, captured } => {
                // Recursively adjust addresses in captured environment
                let adjusted_captured = captured.into_iter()
                    .map(|(name, val)| (name, self.adjust_value_addresses(val, base_addr)))
                    .collect();
                
                Value::Closure { 
                    addr: addr + base_addr,
                    params,
                    captured: adjusted_captured
                }
            }
            Value::List(items) => {
                let adjusted_items = items.into_iter()
                    .map(|item| self.adjust_value_addresses(item, base_addr))
                    .collect();
                Value::List(adjusted_items)
            }
            Value::Object(map) => {
                let adjusted_map = map.into_iter()
                    .map(|(key, val)| (key, self.adjust_value_addresses(val, base_addr)))
                    .collect();
                Value::Object(adjusted_map)
            }
            // Other value types don't contain addresses
            other => other
        }
    }

    fn adjust_instruction_addresses(&self, instruction: &OpCode, base_addr: usize) -> OpCode {
        match instruction {
            OpCode::Jmp(addr) => OpCode::Jmp(addr + base_addr),
            OpCode::Jz(addr) => OpCode::Jz(addr + base_addr),
            OpCode::Call { addr, params } => OpCode::Call { 
                addr: addr + base_addr, 
                params: params.clone() 
            },
            OpCode::MakeFunction { addr, params } => OpCode::MakeFunction { 
                addr: addr + base_addr, 
                params: params.clone() 
            },
            OpCode::MakeLambda { addr, params } => OpCode::MakeLambda { 
                addr: addr + base_addr, 
                params: params.clone() 
            },
            OpCode::Try { catch_addr } => OpCode::Try { 
                catch_addr: catch_addr + base_addr 
            },
            // All other instructions don't contain addresses
            other => other.clone()
        }
    }
}

fn parse_program(path: &str) -> VMResult<Vec<OpCode>> {
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
            "ADD" => OpCode::Add,
            "ADD_F" => OpCode::AddF,
            "SUB" => OpCode::Sub,
            "SUB_F" => OpCode::SubF,
            "MUL_F" => OpCode::MulF,
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
            _ => return Err(VMError::ParseError { line: line_num, instruction: line.to_string() }),
        };
        program.push(opcode);
    }

    Ok(program)
}

fn optimize_program(input_file: &str, output_file: &str) {
    let program = match parse_program(input_file) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    let mut optimizer = optimizer::Optimizer::new(optimizer::OptimizationOptions::default());
    let analysis_before = optimizer.analyze_program(&program);
    
    println!("=== Program Analysis (Before Optimization) ===");
    println!("Total instructions: {}", analysis_before.total_instructions);
    println!("Constants: {}", analysis_before.constant_count);
    println!("Function calls: {}", analysis_before.call_count);
    println!("Memory operations: {}", analysis_before.memory_op_count);
    println!("Jumps: {}", analysis_before.jump_count);
    println!();

    let (optimized_program, stats) = optimizer.optimize(program);
    let analysis_after = optimizer.analyze_program(&optimized_program);

    println!("=== Optimization Results ===");
    println!("Instructions: {} -> {} ({})", 
        analysis_before.total_instructions, 
        analysis_after.total_instructions,
        analysis_before.total_instructions as i32 - analysis_after.total_instructions as i32);
    println!("Constants folded: {}", stats.constants_folded);
    println!("Dead instructions removed: {}", stats.dead_instructions_removed);
    println!("Tail calls optimized: {}", stats.tail_calls_optimized);
    println!("Memory operations optimized: {}", stats.memory_operations_optimized);
    println!("Peephole optimizations: {}", stats.peephole_optimizations_applied);
    println!("Constants propagated: {}", stats.constants_propagated);
    println!("Instructions combined: {}", stats.instructions_combined);
    println!("Jumps threaded: {}", stats.jumps_threaded);
    println!();

    // Write optimized program to file
    match write_optimized_program(&optimized_program, output_file) {
        Ok(_) => println!("Optimized program written to {}", output_file),
        Err(e) => {
            eprintln!("Failed to write optimized program: {}", e);
            std::process::exit(1);
        }
    }
}

fn write_optimized_program(program: &[OpCode], output_file: &str) -> std::io::Result<()> {
    let mut output = String::new();
    
    for (_i, instruction) in program.iter().enumerate() {
        let line = match instruction {
            OpCode::PushInt(n) => format!("PUSH_INT {}", n),
            OpCode::PushFloat(f) => format!("PUSH_FLOAT {}", f),
            OpCode::PushStr(s) => format!("PUSH_STR \"{}\"", s.replace("\"", "\\\"")),
            OpCode::Add => "ADD".to_string(),
            OpCode::AddF => "ADD_F".to_string(),
            OpCode::Sub => "SUB".to_string(),
            OpCode::SubF => "SUB_F".to_string(),
            OpCode::MulF => "MUL_F".to_string(),
            OpCode::DivF => "DIV_F".to_string(),
            OpCode::Concat => "CONCAT".to_string(),
            OpCode::Print => "PRINT".to_string(),
            OpCode::Halt => "HALT".to_string(),
            OpCode::Jmp(addr) => format!("JMP {}", addr),
            OpCode::Jz(addr) => format!("JZ {}", addr),
            OpCode::Call { addr, params } => format!("CALL {} {}", addr, params.join(" ")),
            OpCode::Ret => "RET".to_string(),
            OpCode::Dup => "DUP".to_string(),
            OpCode::Store(var) => format!("STORE {}", var),
            OpCode::Load(var) => format!("LOAD {}", var),
            OpCode::Delete(var) => format!("DELETE {}", var),
            OpCode::Eq => "EQ".to_string(),
            OpCode::Ne => "NE".to_string(),
            OpCode::Gt => "GT".to_string(),
            OpCode::Lt => "LT".to_string(),
            OpCode::Ge => "GE".to_string(),
            OpCode::Le => "LE".to_string(),
            OpCode::EqF => "EQ_F".to_string(),
            OpCode::NeF => "NE_F".to_string(),
            OpCode::GtF => "GT_F".to_string(),
            OpCode::LtF => "LT_F".to_string(),
            OpCode::GeF => "GE_F".to_string(),
            OpCode::LeF => "LE_F".to_string(),
            OpCode::True => "TRUE".to_string(),
            OpCode::False => "FALSE".to_string(),
            OpCode::Not => "NOT".to_string(),
            OpCode::And => "AND".to_string(),
            OpCode::Or => "OR".to_string(),
            OpCode::Null => "NULL".to_string(),
            OpCode::MakeList(n) => format!("MAKE_LIST {}", n),
            OpCode::Len => "LEN".to_string(),
            OpCode::Index => "INDEX".to_string(),
            OpCode::DumpScope => "DUMP_SCOPE".to_string(),
            OpCode::ReadFile => "READ_FILE".to_string(),
            OpCode::WriteFile => "WRITE_FILE".to_string(),
            // Enhanced I/O operations
            OpCode::ReadLine => "READ_LINE".to_string(),
            OpCode::ReadChar => "READ_CHAR".to_string(),
            OpCode::ReadInput => "READ_INPUT".to_string(),
            OpCode::AppendFile => "APPEND_FILE".to_string(),
            OpCode::FileExists => "FILE_EXISTS".to_string(),
            OpCode::FileSize => "FILE_SIZE".to_string(),
            OpCode::DeleteFile => "DELETE_FILE".to_string(),
            OpCode::ListDir => "LIST_DIR".to_string(),
            OpCode::ReadBytes => "READ_BYTES".to_string(),
            OpCode::WriteBytes => "WRITE_BYTES".to_string(),
            // Environment and system
            OpCode::GetEnv => "GET_ENV".to_string(),
            OpCode::SetEnv => "SET_ENV".to_string(),
            OpCode::GetArgs => "GET_ARGS".to_string(),
            OpCode::Exec => "EXEC".to_string(),
            OpCode::ExecCapture => "EXEC_CAPTURE".to_string(),
            OpCode::Exit => "EXIT".to_string(),
            // Time operations
            OpCode::GetTime => "GET_TIME".to_string(),
            OpCode::Sleep => "SLEEP".to_string(),
            OpCode::FormatTime => "FORMAT_TIME".to_string(),
            // Network operations
            OpCode::HttpGet => "HTTP_GET".to_string(),
            OpCode::HttpPost => "HTTP_POST".to_string(),
            OpCode::TcpConnect => "TCP_CONNECT".to_string(),
            OpCode::TcpListen => "TCP_LISTEN".to_string(),
            OpCode::TcpSend => "TCP_SEND".to_string(),
            OpCode::TcpRecv => "TCP_RECV".to_string(),
            OpCode::UdpBind => "UDP_BIND".to_string(),
            OpCode::UdpSend => "UDP_SEND".to_string(),
            OpCode::UdpRecv => "UDP_RECV".to_string(),
            OpCode::DnsResolve => "DNS_RESOLVE".to_string(),
            // Advanced I/O operations
            OpCode::AsyncRead => "ASYNC_READ".to_string(),
            OpCode::AsyncWrite => "ASYNC_WRITE".to_string(),
            OpCode::Await => "AWAIT".to_string(),
            OpCode::StreamCreate => "STREAM_CREATE".to_string(),
            OpCode::StreamRead => "STREAM_READ".to_string(),
            OpCode::StreamWrite => "STREAM_WRITE".to_string(),
            OpCode::StreamClose => "STREAM_CLOSE".to_string(),
            OpCode::JsonParse => "JSON_PARSE".to_string(),
            OpCode::JsonStringify => "JSON_STRINGIFY".to_string(),
            OpCode::CsvParse => "CSV_PARSE".to_string(),
            OpCode::CsvWrite => "CSV_WRITE".to_string(),
            OpCode::Compress => "COMPRESS".to_string(),
            OpCode::Decompress => "DECOMPRESS".to_string(),
            OpCode::Encrypt => "ENCRYPT".to_string(),
            OpCode::Decrypt => "DECRYPT".to_string(),
            OpCode::Hash => "HASH".to_string(),
            OpCode::DbConnect => "DB_CONNECT".to_string(),
            OpCode::DbQuery => "DB_QUERY".to_string(),
            OpCode::DbExec => "DB_EXEC".to_string(),
            OpCode::MakeObject => "MAKE_OBJECT".to_string(),
            OpCode::SetField(field) => format!("SET_FIELD {}", field),
            OpCode::GetField(field) => format!("GET_FIELD {}", field),
            OpCode::HasField(field) => format!("HAS_FIELD {}", field),
            OpCode::DeleteField(field) => format!("DELETE_FIELD {}", field),
            OpCode::Keys => "KEYS".to_string(),
            OpCode::MakeFunction { addr, params } => format!("MAKE_FUNCTION {} {}", addr, params.join(" ")),
            OpCode::CallFunction => "CALL_FUNCTION".to_string(),
            OpCode::MakeLambda { addr, params } => format!("MAKE_LAMBDA {} {}", addr, params.join(" ")),
            OpCode::Capture(var) => format!("CAPTURE {}", var),
            OpCode::Try { catch_addr } => format!("TRY {}", catch_addr),
            OpCode::Catch => "CATCH".to_string(),
            OpCode::Throw => "THROW".to_string(),
            OpCode::EndTry => "END_TRY".to_string(),
            OpCode::Import(path) => format!("IMPORT {}", path),
            OpCode::Export(name) => format!("EXPORT {}", name),
            OpCode::Spawn => format!("SPAWN"),
            OpCode::Receive => format!("RECEIVE"),
            OpCode::Yield => format!("YIELD"),
            OpCode::Send(proc_id) => format!("SEND {}", proc_id),
            OpCode::Monitor(proc_id) => format!("MONITOR {}", proc_id),
            OpCode::Demonitor(monitor_ref) => format!("DEMONITOR {}", monitor_ref),
            OpCode::Link(proc_id) => format!("LINK {}", proc_id),
            OpCode::Unlink(proc_id) => format!("UNLINK {}", proc_id),
        };
        output.push_str(&line);
        output.push('\n');
    }
    
    std::fs::write(output_file, output)
}

fn run_comprehensive_tests() {
    use std::path::Path;
    use std::io::Write;
    
    println!("=== TinyTotVM Comprehensive Test Suite ===");
    println!();
    
    // List of test files to run
    let test_files = vec![
        // Core functionality tests
        ("showcase.ttvm", "Basic VM showcase"),
        ("float_test.ttvm", "Float operations"),
        ("object_test.ttvm", "Object manipulation"),
        ("list_test.ttvm", "List operations"),
        ("string_utils.ttvm", "String utilities"),
        ("variables.ttvm", "Variable operations"),
        ("null_test.ttvm", "Null handling"),
        ("bool_test.ttvm", "Boolean operations"),
        ("coercion_test.ttvm", "Type coercion"),
        ("comparison.ttvm", "Comparison operations"),
        ("comparison-le.ttvm", "Less-than-equal comparison"),
        
        // Function and closure tests
        ("function_test.ttvm", "Basic functions"),
        ("function_args_test.ttvm", "Function arguments"),
        ("function_pointer_test.ttvm", "Function pointers"),
        ("higher_order_test.ttvm", "Higher-order functions"),
        ("call_test.ttvm", "Function calls"),
        ("scoped_call.ttvm", "Scoped function calls"),
        ("closure_test.ttvm", "Closures"),
        ("simple_closure_test.ttvm", "Simple closures"),
        ("nested_closure_test.ttvm", "Nested closures"),
        ("lambda_test.ttvm", "Lambda functions"),
        
        // Module system tests
        ("module_test.ttvm", "Basic modules"),
        ("comprehensive_module_test.ttvm", "Advanced modules"),
        ("closure_module_test.ttvm", "Module closures"),
        ("complex_closure_test.ttvm", "Complex closures"),
        
        // Exception handling tests
        ("exception_test.ttvm", "Exception handling"),
        ("function_exception_test.ttvm", "Function exceptions"),
        ("nested_exception_test.ttvm", "Nested exceptions"),
        ("vm_error_exception_test.ttvm", "VM error exceptions"),
        
        // Standard library tests
        ("stdlib_test.ttvm", "Standard library - math"),
        ("stdlib_string_test.ttvm", "Standard library - strings"),
        ("stdlib_prelude_test.ttvm", "Standard library - prelude"),
        ("stdlib_comprehensive_test.ttvm", "Standard library - comprehensive"),
        ("stdlib_enhanced_io_test.ttvm", "Standard library - enhanced I/O"),
        ("stdlib_network_test.ttvm", "Standard library - network"),
        ("stdlib_advanced_test.ttvm", "Standard library - advanced"),
        
        // I/O tests
        ("io_simple_test.ttvm", "Simple I/O"),
        ("io_comprehensive_test.ttvm", "Comprehensive I/O"),
        ("advanced_io_test.ttvm", "Advanced I/O"),
        
        // Network tests
        ("network_simple_test.ttvm", "Simple network"),
        ("network_tcp_test.ttvm", "TCP operations"),
        ("network_udp_test.ttvm", "UDP operations"),
        ("network_comprehensive_test.ttvm", "Comprehensive network"),
        
        // Optimization tests
        ("optimization_test.ttvm", "Basic optimization"),
        ("constant_folding_test.ttvm", "Constant folding"),
        ("dead_code_test.ttvm", "Dead code elimination"),
        ("tail_call_test.ttvm", "Tail call optimization"),
        ("memory_optimization_test.ttvm", "Memory optimization"),
        ("advanced_optimization_test.ttvm", "Advanced optimization"),
        ("safe_advanced_optimization_test.ttvm", "Safe advanced optimization"),
        ("comprehensive_optimization_test.ttvm", "Comprehensive optimization"),
        ("complete_optimization_showcase.ttvm", "Complete optimization showcase"),
        
        // Control flow tests
        ("if_else.ttvm", "If-else statements"),
        ("countdown.ttvm", "Countdown loop"),
        ("countdown_label.ttvm", "Countdown with labels"),
        ("delete.ttvm", "Delete operations"),
        ("nested_object_test.ttvm", "Nested objects"),
        
        // Additional files
        ("circular_a.ttvm", "Circular dependency A"),
        ("circular_b.ttvm", "Circular dependency B"),
        ("closure_module.ttvm", "Closure module"),
        ("complex_closure_module.ttvm", "Complex closure module"),
        ("io_interactive_test.ttvm", "Interactive I/O test"),
        ("io_test.ttvm", "Basic I/O test"),
        ("math_module.ttvm", "Math module"),
        ("program.ttvm", "Basic program"),
        ("showcase_lisp.ttvm", "Lisp showcase"),
        ("simple_profiling_test.ttvm", "Simple profiling test"),
    ];
    
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    
    let mut results = Vec::new();
    
    // Tests that are expected to fail with specific error messages
    let expected_failures = std::collections::HashMap::from([
        ("circular_a.ttvm", "Circular dependency detected"),
        ("circular_b.ttvm", "Circular dependency detected"),
    ]);
    
    for (filename, description) in &test_files {
        let path = format!("examples/{}", filename);
        
        if !Path::new(&path).exists() {
            println!("SKIP: {} (file not found)", description);
            skipped += 1;
            results.push(TestResult {
                name: description.to_string(),
                expected: "File exists".to_string(),
                actual: "File not found".to_string(),
                passed: false,
            });
            continue;
        }
        
        print!("Testing {}: ", description);
        Write::flush(&mut std::io::stdout()).unwrap();
        
        // Parse and run the test
        match parse_program(&path) {
            Ok(program) => {
                let mut vm = VM::new(program);
                match vm.run() {
                    Ok(()) => {
                        // Check if this test was expected to fail
                        if let Some(expected_error) = expected_failures.get(filename) {
                            println!("FAIL: Expected error '{}' but test passed", expected_error);
                            failed += 1;
                            results.push(TestResult {
                                name: description.to_string(),
                                expected: format!("Error: {}", expected_error),
                                actual: "Success".to_string(),
                                passed: false,
                            });
                        } else {
                            println!("PASS");
                            passed += 1;
                            results.push(TestResult {
                                name: description.to_string(),
                                expected: "Success".to_string(),
                                actual: "Success".to_string(),
                                passed: true,
                            });
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        // Check if this is an expected failure
                        if let Some(expected_error) = expected_failures.get(filename) {
                            if error_msg.contains(expected_error) {
                                println!("PASS (Expected failure)");
                                passed += 1;
                                results.push(TestResult {
                                    name: description.to_string(),
                                    expected: format!("Error: {}", expected_error),
                                    actual: format!("Error: {}", error_msg),
                                    passed: true,
                                });
                            } else {
                                println!("FAIL: Expected '{}' but got '{}'", expected_error, error_msg);
                                failed += 1;
                                results.push(TestResult {
                                    name: description.to_string(),
                                    expected: format!("Error: {}", expected_error),
                                    actual: format!("Error: {}", error_msg),
                                    passed: false,
                                });
                            }
                        } else {
                            println!("FAIL: {}", e);
                            failed += 1;
                            results.push(TestResult {
                                name: description.to_string(),
                                expected: "Success".to_string(),
                                actual: format!("Error: {}", e),
                                passed: false,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                println!("FAIL: Parse error: {}", e);
                failed += 1;
                results.push(TestResult {
                    name: description.to_string(),
                    expected: "Success".to_string(),
                    actual: format!("Parse error: {}", e),
                    passed: false,
                });
            }
        }
    }
    
    println!();
    println!("{}", "═══ Test Summary ═══".bright_cyan().bold());
    
    // Create a table for the summary
    let mut table = Table::new();
    table.load_preset(UTF8_FULL)
         .apply_modifier(UTF8_SOLID_INNER_BORDERS);
    table.set_header(vec![
        Cell::new("Result").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Count").add_attribute(Attribute::Bold).fg(Color::White),
    ]);
    
    table.add_row(vec![
        Cell::new("Passed").fg(Color::White),
        Cell::new(&passed.to_string()).fg(Color::Green),
    ]);
    table.add_row(vec![
        Cell::new("Failed").fg(Color::White),
        Cell::new(&failed.to_string()).fg(if failed > 0 { Color::Red } else { Color::Green }),
    ]);
    table.add_row(vec![
        Cell::new("Skipped").fg(Color::White),
        Cell::new(&skipped.to_string()).fg(Color::Yellow),
    ]);
    table.add_row(vec![
        Cell::new("Total").fg(Color::White),
        Cell::new(&(passed + failed + skipped).to_string()).fg(Color::Cyan),
    ]);
    
    println!("{table}");
    
    if failed > 0 {
        println!();
        println!("{}", "Failed Tests:".bright_red().bold());
        for result in &results {
            if !result.passed {
                println!("   - {}: {}", result.name.red(), result.actual.yellow());
            }
        }
        std::process::exit(1);
    } else {
        println!();
        println!("{}", "All tests passed!".bright_green().bold());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: ttvm [--debug] [--optimize] [--gc <type>] [--gc-debug] [--gc-stats] [--run-tests] [--no-table] [--trace] [--profile] [--smp] [--trace-procs] [--profile-procs] <program.ttvm|program.ttb>");
        eprintln!("       ttvm compile <input.ttvm> <output.ttb>");
        eprintln!("       ttvm compile-lisp <input.lisp> <output.ttvm>");
        eprintln!("       ttvm optimize <input.ttvm> <output.ttvm>");
        eprintln!("       ttvm test-all                                    # Run all examples and tests");
        eprintln!("       ttvm test-concurrency                           # Run concurrency tests");
        eprintln!("       ttvm test-multithreaded                         # Run multi-threaded scheduler tests");
        eprintln!("       ttvm test-message-passing                       # Run message passing tests");
        eprintln!("       ttvm test-process-spawning                      # Run process spawning tests");
        eprintln!("");
        eprintln!("GC Types: mark-sweep (default), no-gc");
        eprintln!("Debug Output: --run-tests enables unit test tables, --gc-debug enables GC debug tables");
        eprintln!("Table Control: --no-table disables formatted output in favor of plain text");
        eprintln!("Performance: --trace enables instruction tracing, --profile enables function profiling");
        eprintln!("Concurrency: --smp enables multi-core execution, --trace-procs enables process tracing, --profile-procs enables process profiling");
        std::process::exit(1);
    }

    let mut debug_mode = false;
    let mut optimize_mode = false;
    let mut gc_type = "mark-sweep";
    let mut gc_debug = false;
    let mut gc_stats = false;
    let mut run_tests = false;
    let mut no_table = false;
    let mut trace_enabled = false;
    let mut profile_enabled = false;
    let mut smp_enabled = false;
    let mut trace_procs = false;
    let mut profile_procs = false;
    let mut file_index = 1;

    // Check for flags
    while file_index < args.len() && args[file_index].starts_with("--") {
        match args[file_index].as_str() {
            "--debug" => {
                debug_mode = true;
                file_index += 1;
            }
            "--optimize" => {
                optimize_mode = true;
                file_index += 1;
            }
            "--gc" => {
                if file_index + 1 >= args.len() {
                    eprintln!("--gc flag requires a garbage collector type");
                    std::process::exit(1);
                }
                gc_type = &args[file_index + 1];
                if gc_type != "mark-sweep" && gc_type != "no-gc" {
                    eprintln!("Unknown GC type: {}. Valid options: mark-sweep, no-gc", gc_type);
                    std::process::exit(1);
                }
                file_index += 2;
            }
            "--gc-debug" => {
                gc_debug = true;
                file_index += 1;
            }
            "--gc-stats" => {
                gc_stats = true;
                file_index += 1;
            }
            "--run-tests" => {
                run_tests = true;
                file_index += 1;
            }
            "--no-table" => {
                no_table = true;
                file_index += 1;
            }
            "--trace" => {
                trace_enabled = true;
                file_index += 1;
            }
            "--profile" => {
                profile_enabled = true;
                file_index += 1;
            }
            "--smp" => {
                smp_enabled = true;
                file_index += 1;
            }
            "--trace-procs" => {
                trace_procs = true;
                file_index += 1;
            }
            "--profile-procs" => {
                profile_procs = true;
                file_index += 1;
            }
            _ => {
                eprintln!("Unknown flag: {}", args[file_index]);
                std::process::exit(1);
            }
        }
    }

    // Handle special commands
    if file_index < args.len() {
        match args[file_index].as_str() {
            "compile" => {
                if args.len() != file_index + 3 {
                    eprintln!("Usage: tinytotvm compile <input.ttvm> <output.ttb>");
                    std::process::exit(1);
                }
                let input = &args[file_index + 1];
                let output = &args[file_index + 2];
                compiler::compile(input, output).expect("Compilation failed");
                println!("Compiled to {}", output);
                return;
            }
            "optimize" => {
                if args.len() != file_index + 3 {
                    eprintln!("Usage: tinytotvm optimize <input.ttvm> <output.ttvm>");
                    std::process::exit(1);
                }
                let input = &args[file_index + 1];
                let output = &args[file_index + 2];
                optimize_program(input, output);
                return;
            }
            "compile-lisp" => {
                if args.len() != file_index + 3 {
                    eprintln!("Usage: tinytotvm compile-lisp <input.lisp> <output.ttvm>");
                    std::process::exit(1);
                }
                let input = &args[file_index + 1];
                let output = &args[file_index + 2];
                lisp_compiler::compile_lisp(input, output);
                println!("Compiled Lisp to {}", output);
                return;
            }
            "test-all" => {
                run_comprehensive_tests();
                return;
            }
            "test-concurrency" => {
                match test_concurrency() {
                    Ok(_) => println!("All concurrency tests passed!"),
                    Err(e) => {
                        eprintln!("Concurrency tests failed: {}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }
            "test-monitoring-linking" => {
                match test_process_monitoring_linking() {
                    Ok(_) => println!("All process monitoring and linking tests passed!"),
                    Err(e) => {
                        eprintln!("Process monitoring and linking tests failed: {}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }
            "test-multithreaded" => {
                match test_multithreaded_scheduler() {
                    Ok(_) => println!("All multi-threaded scheduler tests passed!"),
                    Err(e) => {
                        eprintln!("Multi-threaded scheduler tests failed: {}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }
            "test-message-passing" => {
                match test_message_passing() {
                    Ok(_) => println!("All message passing tests passed!"),
                    Err(e) => {
                        eprintln!("Message passing tests failed: {}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }
            "test-process-spawning" => {
                match test_process_spawning() {
                    Ok(_) => println!("All process spawning tests passed!"),
                    Err(e) => {
                        eprintln!("Process spawning tests failed: {}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }
            _ => {
                // Normal execution, continue below
            }
        }
    }

    // Create VM configuration early
    let output_mode = if no_table {
        OutputMode::Plain
    } else {
        OutputMode::PrettyTable
    };

    let config = VMConfig {
        output_mode,
        run_tests,
        gc_debug,
        gc_stats,
        debug_mode,
        optimize_mode,
        gc_type: gc_type.to_string(),
        trace_enabled,
        profile_enabled,
        smp_enabled,
        trace_procs,
        profile_procs,
    };

    // Run unit tests if requested (no program file needed)
    if config.run_tests {
        run_vm_tests(&config);
        return; // Exit after running tests
    }

    // Check if we have a program file for normal execution
    if file_index >= args.len() {
        eprintln!("No program file specified");
        std::process::exit(1);
    }

    // Normal execution
    let mut program = if args[file_index].ends_with(".ttb") {
        bytecode::load_bytecode(&args[file_index]).expect("Failed to load bytecode")
    } else {
        match parse_program(&args[file_index]) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Parse error: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Apply optimizations if requested
    if optimize_mode {
        let mut optimizer = optimizer::Optimizer::new(optimizer::OptimizationOptions::default());
        let analysis_before = optimizer.analyze_program(&program);
        
        let (optimized_program, stats) = optimizer.optimize(program);
        program = optimized_program;
        
        let analysis_after = optimizer.analyze_program(&program);
        
        println!("=== Optimization Results ===");
        println!("Instructions: {} -> {} ({})", 
            analysis_before.total_instructions, 
            analysis_after.total_instructions,
            analysis_before.total_instructions as i32 - analysis_after.total_instructions as i32);
        println!("Constants folded: {}", stats.constants_folded);
        println!("Dead instructions removed: {}", stats.dead_instructions_removed);
        println!("Tail calls optimized: {}", stats.tail_calls_optimized);
        println!("Memory operations optimized: {}", stats.memory_operations_optimized);
        println!("Peephole optimizations: {}", stats.peephole_optimizations_applied);
        println!("Constants propagated: {}", stats.constants_propagated);
        println!("Instructions combined: {}", stats.instructions_combined);
        println!("Jumps threaded: {}", stats.jumps_threaded);
        println!();
    }

    
    let mut vm = VM::new_with_config(program, &config.gc_type, config.debug_mode || config.gc_debug, config.gc_stats, config.trace_enabled, config.profile_enabled);
    if let Err(e) = vm.run() {
        eprintln!("VM runtime error: {}", e);
        std::process::exit(1);
    }

    // Output profiling results if enabled
    if config.profile_enabled {
        if let Some(profiler) = &vm.profiler {
            profiler.print_results(&config);
        }
    }
    
    if debug_mode {
        let (instructions, max_stack, final_stack) = vm.get_stats();
        println!("Performance stats - Instructions: {}, Max stack: {}, Final stack: {}", 
            instructions, max_stack, final_stack);
    }
    
    if gc_stats {
        let stats = vm.get_gc_stats();
        report_gc_stats(&stats, &config);
    }
}
