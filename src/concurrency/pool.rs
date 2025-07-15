use std::collections::HashMap;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use crossbeam::channel::Sender;
use crossbeam_deque::{Worker, Stealer};

use crate::vm::{OpCode, ProcId};
use super::{TinyProc, MessageSender, ProcessSpawner, NameRegistry, ProcessRegistry, Scheduler, Message};

pub struct SchedulerPool {
    pub schedulers: Vec<thread::JoinHandle<()>>,
    pub global_stealers: Vec<Stealer<Arc<Mutex<TinyProc>>>>,
    pub next_proc_id: Arc<Mutex<ProcId>>,
    pub process_submission_queue: Arc<Mutex<Vec<Arc<Mutex<TinyProc>>>>>,
    pub running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>,
    pub shutdown_flag: Arc<AtomicBool>,
    pub process_registry: Arc<Mutex<ProcessRegistry>>,
}

#[derive(Debug, Clone)]
pub struct SchedulerPoolMessageSender {
    pub process_registry: Arc<Mutex<ProcessRegistry>>,
}

#[derive(Debug, Clone)]
pub struct SchedulerPoolProcessSpawner {
    pub next_proc_id: Arc<Mutex<ProcId>>,
    pub process_submission_queue: Arc<Mutex<Vec<Arc<Mutex<TinyProc>>>>>,
    pub running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>,
    pub process_registry: Arc<Mutex<ProcessRegistry>>,
    pub message_sender: Arc<dyn MessageSender>,
}

impl MessageSender for SchedulerPoolMessageSender {
    fn send_message(&self, target_proc_id: ProcId, message: Message) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.send_message_simple(target_proc_id, message)
    }
}

impl NameRegistry for SchedulerPoolMessageSender {
    fn register_name(&self, name: String, proc_id: ProcId) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.register_name(name, proc_id)
    }
    
    fn unregister_name(&self, name: &str) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.unregister_name(name)
    }
    
    fn whereis(&self, name: &str) -> Option<ProcId> {
        let registry = self.process_registry.lock().unwrap();
        registry.whereis(name)
    }
    
    fn send_to_named(&self, name: &str, message: Message) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.send_to_named(name, message)
    }
}

impl NameRegistry for SchedulerPoolProcessSpawner {
    fn register_name(&self, name: String, proc_id: ProcId) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.register_name(name, proc_id)
    }
    
    fn unregister_name(&self, name: &str) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.unregister_name(name)
    }
    
    fn whereis(&self, name: &str) -> Option<ProcId> {
        let registry = self.process_registry.lock().unwrap();
        registry.whereis(name)
    }
    
    fn send_to_named(&self, name: &str, message: Message) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.send_to_named(name, message)
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
        proc.name_registry = Some(Arc::new(self.clone()));
        
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
            registry.register_process(proc_id, sender.clone()).expect("Failed to register process");
        }
        
        {
            // Use blocking lock to ensure process gets added to submission queue
            let mut queue = self.process_submission_queue.lock().unwrap();
            queue.push(proc_arc);
            // println!("DEBUG: Added process {} to submission queue", proc_id);
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
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            process_registry: Arc::new(Mutex::new(ProcessRegistry::new())),
        }
    }
    
    pub fn new_with_threads(num_threads: usize) -> Self {
        let mut pool = Self::new();
        pool.spawn_smp_schedulers(num_threads);
        pool
    }
    
    pub fn new_with_default_threads() -> Self {
        // Use all available CPU cores for optimal performance
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4); // fallback to 4 threads if detection fails
        println!("Creating scheduler pool with {} threads (CPU cores)", num_threads);
        Self::new_with_threads(num_threads)
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
        
        // Set the name registry for this process
        proc.name_registry = Some(message_sender.clone());
        
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
            registry.register_process(proc_id, sender.clone()).expect("Failed to register process");
        }
        
        {
            let mut queue = self.process_submission_queue.lock().unwrap();
            queue.push(proc_arc);
        }
        
        (proc_id, sender)
    }
    
    #[allow(dead_code)]
    pub fn send_message(&self, target_proc_id: ProcId, message: Message) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.send_message_simple(target_proc_id, message)
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
            
            let _next_proc_id = self.next_proc_id.clone();
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
        let start_time = std::time::Instant::now();
        let max_wait_time = Duration::from_secs(3); // Maximum wait time for processes
        
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
            
            // Check if we've been waiting too long
            if start_time.elapsed() > max_wait_time {
                println!("Scheduler timeout reached - shutting down {} remaining processes", running_count);
                break;
            }
            
            // Only print debug info if we're waiting for a while
            if start_time.elapsed() > Duration::from_secs(1) {
                static mut LAST_DEBUG: Option<std::time::Instant> = None;
                unsafe {
                    let now = std::time::Instant::now();
                    if LAST_DEBUG.is_none() || now.duration_since(LAST_DEBUG.unwrap()) > Duration::from_secs(2) {
                        println!("Waiting for {} processes to complete...", running_count);
                        LAST_DEBUG = Some(now);
                    }
                }
            }
            
            thread::sleep(Duration::from_millis(10));
        }
        
        // Signal shutdown to all schedulers
        self.shutdown_flag.store(true, Ordering::Relaxed);
        
        Ok(())
    }
    
    pub fn wait_for_completion(self) {
        for (_i, handle) in self.schedulers.into_iter().enumerate() {
            // Use a timeout approach - if threads don't join within reasonable time, force exit
            let start_time = std::time::Instant::now();
            let mut handle_option = Some(handle);
            
            while let Some(h) = handle_option.take() {
                if h.is_finished() {
                    h.join().unwrap();
                    break;
                } else if start_time.elapsed() >= Duration::from_millis(100) {
                    // Short timeout since scheduler should respond quickly to shutdown flag
                    std::mem::forget(h);
                    break;
                } else {
                    handle_option = Some(h);
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
    
    // Process name registry methods
    #[allow(dead_code)]
    pub fn register_name(&self, name: String, proc_id: ProcId) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.register_name(name, proc_id)
    }
    
    #[allow(dead_code)]
    pub fn unregister_name(&self, name: &str) -> Result<(), String> {
        let mut registry = self.process_registry.lock().unwrap();
        registry.unregister_name(name)
    }
    
    #[allow(dead_code)]
    pub fn whereis(&self, name: &str) -> Option<ProcId> {
        let registry = self.process_registry.lock().unwrap();
        registry.whereis(name)
    }
    
    #[allow(dead_code)]
    pub fn send_to_named(&self, name: &str, message: Message) -> Result<(), String> {
        let proc_id = self.whereis(name).ok_or_else(|| {
            format!("Process '{}' not found", name)
        })?;
        
        self.send_message(proc_id, message)
    }
    
    // Clean up name registrations when process exits
    #[allow(dead_code)]
    pub fn cleanup_process_names(&self, proc_id: ProcId) {
        let mut registry = self.process_registry.lock().unwrap();
        registry.unregister_process(proc_id).ok();
    }
}