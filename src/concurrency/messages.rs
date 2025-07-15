use std::time::Instant;
use crate::vm::{Value, ProcId};

#[derive(Debug, Clone)]
pub enum Message {
    Value(Value),
    Signal(String),
    Exit(ProcId),
    Monitor(ProcId, String), // Monitor request: (target_pid, monitor_ref)
    Down(ProcId, String, String), // Down message: (monitored_pid, monitor_ref, reason)
    Link(ProcId), // Link request
    Unlink(ProcId), // Unlink request
    TrapExit(bool), // Set trap_exit flag
}

#[derive(Debug, Clone)]
pub struct OrderedMessage {
    pub message: Message,
    pub from_pid: ProcId,
    pub to_pid: ProcId,
    pub sequence_number: u64,
    pub timestamp: Instant,
}