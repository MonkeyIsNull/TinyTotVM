use crate::ir::{RegBlock, RegInstr, RegValue, RegId, StackState};
use crate::vm::{OpCode, Value, VMError, VMResult};
use std::collections::HashMap;

pub struct StackToRegisterLowering {
    block: RegBlock,
    stack: StackState,
    label_map: HashMap<usize, usize>, // bytecode address -> IR instruction index
}

impl StackToRegisterLowering {
    pub fn new() -> Self {
        Self {
            block: RegBlock::new(),
            stack: StackState::new(),
            label_map: HashMap::new(),
        }
    }
    
    pub fn lower(bytecode: &[OpCode]) -> VMResult<RegBlock> {
        let mut lowering = Self::new();
        lowering.process_bytecode(bytecode)?;
        Ok(lowering.block)
    }
    
    fn process_bytecode(&mut self, bytecode: &[OpCode]) -> VMResult<()> {
        // First pass: build address mapping by simulating instruction generation
        let mut temp_instruction_count = 0;
        for (addr, instruction) in bytecode.iter().enumerate() {
            self.label_map.insert(addr, temp_instruction_count);
            
            // Count how many IR instructions this bytecode instruction will generate
            temp_instruction_count += match instruction {
                OpCode::PushInt(_) | OpCode::PushFloat(_) | OpCode::PushStr(_) | 
                OpCode::PushBool(_) => 1,
                
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div |
                OpCode::AddF | OpCode::SubF | OpCode::MulF | OpCode::DivF |
                OpCode::Eq | OpCode::Ne | OpCode::Lt | OpCode::Le | OpCode::Gt | OpCode::Ge |
                OpCode::EqF | OpCode::NeF | OpCode::LtF | OpCode::LeF | OpCode::GtF | OpCode::GeF => 1,
                
                OpCode::Print | OpCode::Dup => 1,
                
                OpCode::Jmp(_) | OpCode::Jz(_) => 1,
                
                OpCode::Halt => 1,
                
                _ => 1, // Conservative estimate for unhandled instructions
            };
        }
        
        // Second pass: translate instructions
        for (addr, instruction) in bytecode.iter().enumerate() {
            self.translate_instruction(addr, instruction)?;
        }
        
        Ok(())
    }
    
    fn translate_instruction(&mut self, addr: usize, instruction: &OpCode) -> VMResult<()> {
        match instruction {
            // Push operations - allocate register and load constant
            OpCode::PushInt(n) => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Int(*n))));
                self.stack.push(reg);
            }
            
            OpCode::PushFloat(f) => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Float(*f))));
                self.stack.push(reg);
            }
            
            OpCode::PushStr(s) => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Str(s.clone()))));
                self.stack.push(reg);
            }
            
            OpCode::PushBool(b) => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Bool(*b))));
                self.stack.push(reg);
            }
            
            OpCode::True => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Bool(true))));
                self.stack.push(reg);
            }
            
            OpCode::False => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Bool(false))));
                self.stack.push(reg);
            }
            
            OpCode::Null => {
                let reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(reg, RegValue::Const(Value::Null)));
                self.stack.push(reg);
            }
            
            // Binary arithmetic operations
            OpCode::Add => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Add(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::AddF => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::AddF(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Sub => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Sub(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::SubF => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::SubF(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Mul => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Mul(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::MulF => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::MulF(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Div => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Div(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::DivF => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::DivF(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Concat => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Concat(dst, src1, src2));
                self.stack.push(dst);
            }
            
            // Comparison operations
            OpCode::Eq => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Eq(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Ne => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Ne(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Lt => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Lt(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Le => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Le(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Gt => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Gt(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Ge => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Ge(dst, src1, src2));
                self.stack.push(dst);
            }
            
            // Logical operations
            OpCode::And => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::And(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Or => {
                let (dst, src1, src2) = self.pop_binary_op()?;
                self.block.add_instruction(RegInstr::Or(dst, src1, src2));
                self.stack.push(dst);
            }
            
            OpCode::Not => {
                let src = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("NOT".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Not(dst, src));
                self.stack.push(dst);
            }
            
            // Variable operations
            OpCode::Store(var_name) => {
                let src = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("STORE".to_string()))?;
                self.block.add_instruction(RegInstr::Store(var_name.clone(), src));
            }
            
            OpCode::Load(var_name) => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Load(dst, var_name.clone()));
                self.stack.push(dst);
            }
            
            // Control flow
            OpCode::Jmp(target) => {
                let ir_target = self.label_map.get(target).copied().unwrap_or(*target);
                self.block.add_instruction(RegInstr::Jmp(ir_target));
            }
            
            OpCode::Jz(target) => {
                let condition = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("JZ".to_string()))?;
                let ir_target = self.label_map.get(target).copied().unwrap_or(*target);
                self.block.add_instruction(RegInstr::Jz(condition, ir_target));
            }
            
            OpCode::Call { addr, params: _ } => {
                // Simplified call - in a full implementation, we'd handle parameters
                let return_reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Call(Some(return_reg), *addr, vec![]));
                self.stack.push(return_reg);
            }
            
            OpCode::Ret => {
                let return_value = self.stack.pop();
                self.block.add_instruction(RegInstr::Ret(return_value));
            }
            
            // I/O operations
            OpCode::Print => {
                let src = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("PRINT".to_string()))?;
                self.block.add_instruction(RegInstr::Print(src));
            }
            
            // Duplication
            OpCode::Dup => {
                let src = self.stack.peek().ok_or_else(|| VMError::StackUnderflow("DUP".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(dst, RegValue::Reg(src)));
                self.stack.push(dst);
            }
            
            // List operations
            OpCode::MakeList(count) => {
                let mut elements = Vec::new();
                for _ in 0..*count {
                    elements.push(self.stack.pop().ok_or_else(|| VMError::StackUnderflow("MAKE_LIST".to_string()))?);
                }
                elements.reverse(); // Stack is LIFO, but we want elements in order
                
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::MakeList(dst, elements));
                self.stack.push(dst);
            }
            
            OpCode::Len => {
                let src = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("LEN".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Len(dst, src));
                self.stack.push(dst);
            }
            
            OpCode::Index => {
                let index = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("INDEX".to_string()))?;
                let container = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("INDEX".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Index(dst, container, index));
                self.stack.push(dst);
            }
            
            // Object operations
            OpCode::MakeObject => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::MakeObject(dst));
                self.stack.push(dst);
            }
            
            // Terminal instruction
            OpCode::Halt => {
                self.block.add_instruction(RegInstr::Halt);
            }
            
            // For unsupported instructions, add a NOP for now
            _ => {
                self.block.add_instruction(RegInstr::Nop);
            }
        }
        
        Ok(())
    }
    
    fn pop_binary_op(&mut self) -> VMResult<(RegId, RegId, RegId)> {
        let src2 = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("binary operation".to_string()))?;
        let src1 = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("binary operation".to_string()))?;
        let dst = self.block.alloc_register();
        Ok((dst, src1, src2))
    }
}