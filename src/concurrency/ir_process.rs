use std::collections::{HashMap, HashSet};
use std::time::Instant;
use std::sync::Arc;
use crossbeam::channel::{Receiver, Sender};

use crate::vm::{VMResult, ProcId};
use crate::ir::{RegBlock, vm::RegisterVM};
use crate::concurrency::{Message, SupervisorSpec, ChildState};
use crate::concurrency::process::{MessageSender, ProcessSpawner, NameRegistry, RunnableProcess};
use crate::ProcState;

/// IR-based process that executes register-based instructions
#[derive(Debug)]
pub struct IRProc {
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
    pub monitors: HashMap<String, ProcId>,
    pub monitored_by: HashMap<ProcId, String>,
    pub linked_processes: HashSet<ProcId>,
    pub exit_reason: Option<String>,
    pub trap_exit: bool,
    
    // Supervision data
    pub supervisor_spec: Option<SupervisorSpec>,
    pub supervised_children: HashMap<String, ChildState>,
    pub supervisor_pid: Option<ProcId>,
    pub restart_intensity_count: usize,
    pub restart_period_start: Instant,
    
    // IR execution engine
    pub ir_vm: RegisterVM,
    pub debug_mode: bool,
    pub trace_enabled: bool,
}

impl IRProc {
    pub fn new(id: ProcId, ir_block: RegBlock) -> (Self, Sender<Message>) {
        let (sender, receiver) = crossbeam::channel::unbounded();
        let ir_vm = RegisterVM::new_with_process_id(ir_block, id);
        
        (Self {
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
            trap_exit: false,
            supervisor_spec: None,
            supervised_children: HashMap::new(),
            supervisor_pid: None,
            restart_intensity_count: 0,
            restart_period_start: Instant::now(),
            ir_vm,
            debug_mode: false,
            trace_enabled: false,
        }, sender)
    }
    
    pub fn run_until_yield(&mut self) -> VMResult<ProcState> {
        // Process any pending messages first
        while let Ok(message) = self.mailbox.try_recv() {
            if self.debug_mode {
                println!("[IR Process {}] Received message: {:?}", self.id, message);
            }
            self.ir_vm.add_message(message);
        }
        
        // Run IR instructions until yield or halt
        self.ir_vm.run_until_yield()?;
        
        if self.ir_vm.is_halted() {
            self.state = ProcState::Exited;
        } else if self.ir_vm.is_yielded() {
            self.state = ProcState::Ready;
        }
        
        Ok(self.state)
    }
    
    pub fn has_messages(&self) -> bool {
        !self.mailbox.is_empty() || self.ir_vm.has_messages()
    }
    
    pub fn send_message(&self, message: Message) -> Result<(), String> {
        self.mailbox_sender.send(message)
            .map_err(|e| format!("Failed to send message: {}", e))
    }
    
    pub fn is_waiting_for_message(&self) -> bool {
        self.waiting_for_message
    }
    
    pub fn set_waiting_for_message(&mut self, waiting: bool) {
        self.waiting_for_message = waiting;
    }
}

impl RunnableProcess for IRProc {
    fn get_id(&self) -> ProcId {
        self.id
    }
    
    fn get_state(&self) -> ProcState {
        self.state
    }
    
    fn set_state(&mut self, state: ProcState) {
        self.state = state;
    }
    
    fn run_until_yield(&mut self) -> VMResult<ProcState> {
        IRProc::run_until_yield(self)
    }
    
    fn has_messages(&self) -> bool {
        IRProc::has_messages(self)
    }
    
    fn send_message(&self, message: Message) -> Result<(), String> {
        IRProc::send_message(self, message)
    }
    
    fn is_waiting_for_message(&self) -> bool {
        self.waiting_for_message
    }
    
    fn set_waiting_for_message(&mut self, waiting: bool) {
        self.waiting_for_message = waiting;
    }
}