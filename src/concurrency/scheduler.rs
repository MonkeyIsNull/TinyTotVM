use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use crossbeam_deque::{Worker, Stealer};
use crate::concurrency::{TinyProc, ProcessRegistry};
use crate::vm::ProcId;
use crate::ProcState;

#[derive(Debug)]
pub struct Scheduler {
    pub id: usize,
    pub local_queue: Worker<Arc<Mutex<TinyProc>>>,
    pub remote_stealers: Vec<Stealer<Arc<Mutex<TinyProc>>>>,
    pub running: bool,
}

impl Scheduler {
    #[allow(dead_code)]
    pub fn new(id: usize) -> Self {
        let local_queue = Worker::new_fifo();
        Scheduler {
            id,
            local_queue,
            remote_stealers: Vec::new(),
            running: true,
        }
    }
    
    #[allow(dead_code)]
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
    
    pub fn run_scheduler_loop(&mut self, submission_queue: Arc<Mutex<Vec<Arc<Mutex<TinyProc>>>>>, shutdown_flag: Arc<AtomicBool>, running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>, registry: Arc<Mutex<ProcessRegistry>>) {
        loop {
            // Check for shutdown first - atomic read is fast and lock-free
            if shutdown_flag.load(Ordering::Relaxed) {
                break;
            }
            
            // Try to get new processes from submission queue FIRST (higher priority)
            if let Ok(mut queue) = submission_queue.try_lock() {
                if let Some(proc_arc) = queue.pop() {
                    let _proc_id = {
                        let proc = proc_arc.lock().unwrap();
                        proc.id
                    };
                    // println!("DEBUG: Scheduler {} picked up process {} from submission queue", self.id, proc_id);
                    // Release the queue lock immediately before executing the process
                    drop(queue);
                    self.execute_process_with_cleanup(proc_arc.clone(), running_processes.clone(), registry.clone());
                    
                    // Check shutdown flag after processing each process
                    if shutdown_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    continue;
                }
                // Queue is empty, release the lock
                drop(queue);
            }
            
            // Try to get a process from local queue
            if let Some(proc_arc) = self.get_next_process() {
                self.execute_process_with_cleanup(proc_arc.clone(), running_processes.clone(), registry.clone());
                
                // Check shutdown flag after processing each process
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }
                continue;
            }
            
            // If no local work, try to steal from other schedulers
            if let Some(proc_arc) = self.steal_from_others() {
                self.execute_process_with_cleanup(proc_arc.clone(), running_processes.clone(), registry.clone());
                
                // Check shutdown flag after processing each process
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }
                continue;
            }
            
            // No work available, sleep very briefly and check shutdown again soon
            thread::sleep(Duration::from_millis(1));
            
            // Check shutdown again after sleep to be more responsive
            if shutdown_flag.load(Ordering::Relaxed) {
                break;
            }
        }
    }
    
    
    fn execute_process_with_cleanup(&mut self, proc_arc: Arc<Mutex<TinyProc>>, running_processes: Arc<Mutex<HashMap<ProcId, Arc<Mutex<TinyProc>>>>>, registry: Arc<Mutex<ProcessRegistry>>) {
        let proc_id = {
            let proc = proc_arc.lock().unwrap();
            proc.id
        };
        
        let mut proc = proc_arc.lock().unwrap();
        
        // Show debug info about which core is processing which process (only on first execution)
        if matches!(proc.state, ProcState::Ready) && !proc.waiting_for_message {
            println!("Core {}: Starting execution of process {}", self.id, proc_id);
        }
        
        match proc.state {
            ProcState::Ready | ProcState::Waiting => {
                // If process is waiting for a message, check if it has one now
                if proc.waiting_for_message && !proc.has_messages() {
                    // Still waiting for a message, don't requeue immediately to prevent tight loops
                    // This allows the scheduler to check shutdown flag more frequently
                    drop(proc);
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
                        reg.unregister_process(proc_id).ok();
                    }
                    Err(e) => {
                        eprintln!("Process {} error: {:?}", proc_id, e);
                        proc.state = ProcState::Exited;
                        drop(proc);
                        let mut running = running_processes.lock().unwrap();
                        running.remove(&proc_id);
                        let mut reg = registry.lock().unwrap();
                        reg.unregister_process(proc_id).ok();
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
                reg.unregister_process(proc_id).ok();
            }
            _ => {
                // Re-queue for other states
                drop(proc);
                self.local_queue.push(proc_arc);
            }
        }
    }
}