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
            
            // Boolean constants
            OpCode::True => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::True(dst));
                self.stack.push(dst);
            }
            
            OpCode::False => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::False(dst));
                self.stack.push(dst);
            }
            
            OpCode::Null => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Null(dst));
                self.stack.push(dst);
            }
            
            // Object operations
            OpCode::SetField(field_name) => {
                let value = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("SET_FIELD".to_string()))?;
                let object = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("SET_FIELD".to_string()))?;
                self.block.add_instruction(RegInstr::SetField(object, field_name.clone(), value));
            }
            
            OpCode::GetField(field_name) => {
                let object = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("GET_FIELD".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::GetField(dst, object, field_name.clone()));
                self.stack.push(dst);
            }
            
            OpCode::HasField(field_name) => {
                let object = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("HAS_FIELD".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::HasField(dst, object, field_name.clone()));
                self.stack.push(dst);
            }
            
            OpCode::DeleteField(field_name) => {
                let object = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("DELETE_FIELD".to_string()))?;
                self.block.add_instruction(RegInstr::DeleteField(object, field_name.clone()));
            }
            
            OpCode::Keys => {
                let object = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("KEYS".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Keys(dst, object));
                self.stack.push(dst);
            }
            
            // Variable operations
            OpCode::Store(var_name) => {
                let value = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("STORE".to_string()))?;
                self.block.add_instruction(RegInstr::Store(var_name.clone(), value));
            }
            
            OpCode::Load(var_name) => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Load(dst, var_name.clone()));
                self.stack.push(dst);
            }
            
            OpCode::Delete(var_name) => {
                self.block.add_instruction(RegInstr::Delete(var_name.clone()));
            }
            
            // Function operations
            OpCode::MakeFunction { addr, params } => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::MakeFunction(dst, *addr, params.clone()));
                self.stack.push(dst);
            }
            
            OpCode::CallFunction => {
                let func_reg = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("CALL_FUNCTION".to_string()))?;
                // For now, assume no arguments - this would need more sophisticated handling
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::CallFunction(Some(dst), func_reg, vec![]));
                self.stack.push(dst);
            }
            
            OpCode::MakeLambda { addr, params } => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::MakeLambda(dst, *addr, params.clone()));
                self.stack.push(dst);
            }
            
            OpCode::Capture(var_name) => {
                let value = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("CAPTURE".to_string()))?;
                self.block.add_instruction(RegInstr::Capture(var_name.clone(), value));
            }
            
            // Exception handling
            OpCode::Try { catch_addr } => {
                self.block.add_instruction(RegInstr::Try(*catch_addr));
            }
            
            OpCode::Catch => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Catch(dst));
                self.stack.push(dst);
            }
            
            OpCode::Throw => {
                let exception = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("THROW".to_string()))?;
                self.block.add_instruction(RegInstr::Throw(exception));
            }
            
            OpCode::EndTry => {
                self.block.add_instruction(RegInstr::EndTry);
            }
            
            // Module system
            OpCode::Import(module_path) => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Import(dst, module_path.clone()));
                self.stack.push(dst);
            }
            
            OpCode::Export(name) => {
                let value = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("EXPORT".to_string()))?;
                self.block.add_instruction(RegInstr::Export(name.clone(), value));
            }
            
            // Concurrency operations - the key ones we need to support!
            OpCode::Spawn => {
                let function = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("SPAWN".to_string()))?;
                let dst_pid = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Spawn(dst_pid, function));
                self.stack.push(dst_pid);
            }
            
            OpCode::Receive => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Receive(dst));
                self.stack.push(dst);
            }
            
            OpCode::ReceiveMatch(patterns) => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::ReceiveMatch(dst, patterns.clone()));
                self.stack.push(dst);
            }
            
            OpCode::Yield => {
                self.block.add_instruction(RegInstr::Yield);
            }
            
            OpCode::Send(target_pid) => {
                let message = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("SEND".to_string()))?;
                let pid_reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(pid_reg, RegValue::Const(crate::vm::Value::Int(*target_pid as i64))));
                self.block.add_instruction(RegInstr::Send(pid_reg, message));
            }
            
            OpCode::Monitor(target_pid) => {
                let dst_ref = self.block.alloc_register();
                let pid_reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(pid_reg, RegValue::Const(crate::vm::Value::Int(*target_pid as i64))));
                self.block.add_instruction(RegInstr::Monitor(dst_ref, pid_reg));
                self.stack.push(dst_ref);
            }
            
            OpCode::Demonitor(monitor_ref) => {
                let ref_reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(ref_reg, RegValue::Const(crate::vm::Value::Str(monitor_ref.clone()))));
                self.block.add_instruction(RegInstr::Demonitor(ref_reg));
            }
            
            OpCode::Link(target_pid) => {
                let pid_reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(pid_reg, RegValue::Const(crate::vm::Value::Int(*target_pid as i64))));
                self.block.add_instruction(RegInstr::Link(pid_reg));
            }
            
            OpCode::Unlink(target_pid) => {
                let pid_reg = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Mov(pid_reg, RegValue::Const(crate::vm::Value::Int(*target_pid as i64))));
                self.block.add_instruction(RegInstr::Unlink(pid_reg));
            }
            
            OpCode::TrapExit => {
                let enable_flag = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("TRAP_EXIT".to_string()))?;
                self.block.add_instruction(RegInstr::TrapExit(enable_flag));
            }
            
            // Process registry operations
            OpCode::Register(name) => {
                let pid = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("REGISTER".to_string()))?;
                self.block.add_instruction(RegInstr::Register(name.clone(), pid));
            }
            
            OpCode::Unregister(name) => {
                self.block.add_instruction(RegInstr::Unregister(name.clone()));
            }
            
            OpCode::Whereis(name) => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::Whereis(dst, name.clone()));
                self.stack.push(dst);
            }
            
            OpCode::SendNamed(name) => {
                let message = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("SEND_NAMED".to_string()))?;
                self.block.add_instruction(RegInstr::SendNamed(name.clone(), message));
            }
            
            // Add basic I/O operations
            OpCode::ReadFile => {
                let filename = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("READ_file".to_string()))?;
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::ReadFile(dst, filename));
                self.stack.push(dst);
            }
            
            OpCode::WriteFile => {
                let content = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("write_file".to_string()))?;
                let filename = self.stack.pop().ok_or_else(|| VMError::StackUnderflow("write_file".to_string()))?;
                self.block.add_instruction(RegInstr::WriteFile(filename, content));
            }
            
            OpCode::DumpScope => {
                let dst = self.block.alloc_register();
                self.block.add_instruction(RegInstr::DumpScope(dst));
                self.stack.push(dst);
            }
            
            // For any remaining unsupported instructions, add a NOP 
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