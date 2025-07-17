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