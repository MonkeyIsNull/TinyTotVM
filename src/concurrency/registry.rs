use std::collections::{HashMap, HashSet};
use std::time::Instant;
use crossbeam::channel::Sender;

use crate::vm::ProcId;
use crate::concurrency::Message;
use crate::ProcState;

#[derive(Debug)]
pub struct ProcessRegistry {
    pub process_senders: HashMap<ProcId, Sender<Message>>,
    pub name_to_pid: HashMap<String, ProcId>,
    pub pid_to_names: HashMap<ProcId, HashSet<String>>,
    pub process_info: HashMap<ProcId, ProcessInfo>,
    pub message_sequences: HashMap<(ProcId, ProcId), u64>, // (from_pid, to_pid) -> next_sequence_number
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: ProcId,
    pub start_time: Instant,
    pub message_count: usize,
    pub state: ProcState,
    pub supervisor: Option<ProcId>,
    pub children: HashSet<ProcId>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        ProcessRegistry {
            process_senders: HashMap::new(),
            name_to_pid: HashMap::new(),
            pid_to_names: HashMap::new(),
            process_info: HashMap::new(),
            message_sequences: HashMap::new(),
        }
    }
    
    pub fn register_process(&mut self, pid: ProcId, sender: Sender<Message>) -> Result<(), String> {
        if self.process_senders.contains_key(&pid) {
            return Err(format!("Process {} already registered", pid));
        }
        
        self.process_senders.insert(pid, sender);
        self.process_info.insert(pid, ProcessInfo {
            pid,
            start_time: Instant::now(),
            message_count: 0,
            state: ProcState::Ready,
            supervisor: None,
            children: HashSet::new(),
        });
        
        Ok(())
    }
    
    pub fn unregister_process(&mut self, pid: ProcId) -> Result<(), String> {
        // Remove from process senders
        self.process_senders.remove(&pid);
        
        // Remove all names associated with this process
        if let Some(names) = self.pid_to_names.remove(&pid) {
            for name in names {
                self.name_to_pid.remove(&name);
            }
        }
        
        // Remove process info
        self.process_info.remove(&pid);
        
        Ok(())
    }
    
    pub fn register_name(&mut self, name: String, pid: ProcId) -> Result<(), String> {
        if !self.process_senders.contains_key(&pid) {
            return Err(format!("Process {} not found", pid));
        }
        
        if self.name_to_pid.contains_key(&name) {
            return Err(format!("Name '{}' already registered", name));
        }
        
        self.name_to_pid.insert(name.clone(), pid);
        self.pid_to_names.entry(pid).or_insert_with(HashSet::new).insert(name);
        
        Ok(())
    }
    
    pub fn unregister_name(&mut self, name: &str) -> Result<(), String> {
        if let Some(pid) = self.name_to_pid.remove(name) {
            if let Some(names) = self.pid_to_names.get_mut(&pid) {
                names.remove(name);
                if names.is_empty() {
                    self.pid_to_names.remove(&pid);
                }
            }
            Ok(())
        } else {
            Err(format!("Name '{}' not found", name))
        }
    }
    
    pub fn whereis(&self, name: &str) -> Option<ProcId> {
        self.name_to_pid.get(name).copied()
    }
    
    pub fn send_message(&mut self, from_pid: ProcId, to_pid: ProcId, message: Message) -> Result<(), String> {
        if let Some(sender) = self.process_senders.get(&to_pid) {
            // Get next sequence number for this process pair
            let key = (from_pid, to_pid);
            let sequence_number = self.message_sequences.entry(key).or_insert(0);
            *sequence_number += 1;
            
            // Update message count
            if let Some(info) = self.process_info.get_mut(&to_pid) {
                info.message_count += 1;
            }
            
            sender.send(message).map_err(|e| format!("Failed to send message: {}", e))
        } else {
            Err(format!("Process {} not found", to_pid))
        }
    }
    
    pub fn send_message_simple(&mut self, pid: ProcId, message: Message) -> Result<(), String> {
        // For backward compatibility - assume system sender (pid 0)
        self.send_message(0, pid, message)
    }
    
    pub fn send_to_named(&mut self, name: &str, message: Message) -> Result<(), String> {
        let pid = self.whereis(name).ok_or_else(|| format!("Process '{}' not found", name))?;
        self.send_message_simple(pid, message)
    }
    
    #[allow(dead_code)]
    pub fn get_process_info(&self, pid: ProcId) -> Option<&ProcessInfo> {
        self.process_info.get(&pid)
    }
    
    #[allow(dead_code)]
    pub fn update_process_state(&mut self, pid: ProcId, state: ProcState) {
        if let Some(info) = self.process_info.get_mut(&pid) {
            info.state = state;
        }
    }
    
    #[allow(dead_code)]
    pub fn set_supervisor(&mut self, child_pid: ProcId, supervisor_pid: ProcId) {
        if let Some(info) = self.process_info.get_mut(&child_pid) {
            info.supervisor = Some(supervisor_pid);
        }
        if let Some(supervisor_info) = self.process_info.get_mut(&supervisor_pid) {
            supervisor_info.children.insert(child_pid);
        }
    }
}