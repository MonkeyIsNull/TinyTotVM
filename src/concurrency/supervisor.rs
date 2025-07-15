use std::time::Duration;
use crate::vm::{OpCode, ProcId};

// Supervision system enums and structs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RestartStrategy {
    OneForOne,     // Restart only the failed child
    OneForAll,     // Restart all children when one fails
    RestForOne,    // Restart failed child and all started after it
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildType {
    Worker,
    Supervisor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shutdown {
    Brutal,           // Kill immediately
    Timeout(Duration), // Give time to shutdown gracefully
    Infinity,         // Wait indefinitely
}

#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub id: String,
    pub instructions: Vec<OpCode>,
    pub restart: RestartPolicy,
    pub shutdown: Shutdown,
    pub child_type: ChildType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RestartPolicy {
    Permanent,   // Always restart
    Temporary,   // Never restart
    Transient,   // Restart only if abnormal exit
}

#[derive(Debug, Clone)]
pub struct SupervisorSpec {
    pub strategy: RestartStrategy,
    pub intensity: usize,        // max restarts
    pub period: Duration,        // time window for intensity
    pub children: Vec<ChildSpec>,
}

#[derive(Debug, Clone)]
pub struct ChildState {
    pub pid: ProcId,
    pub spec: ChildSpec,
    pub restart_count: usize,
    pub last_restart: std::time::Instant,
}