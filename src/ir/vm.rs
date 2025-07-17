use crate::ir::{RegBlock, RegInstr, RegValue, RegId};
use crate::vm::{Value, VMError, VMResult};
use std::collections::HashMap;

pub struct RegisterVM {
    pub registers: Vec<Value>,
    pub variables: HashMap<String, Value>,
    pub ip: usize,
    pub block: RegBlock,
    pub halted: bool,
}

impl RegisterVM {
    pub fn new(block: RegBlock) -> Self {
        let register_count = block.register_count as usize;
        Self {
            registers: vec![Value::Null; register_count],
            variables: HashMap::new(),
            ip: block.entry,
            block,
            halted: false,
        }
    }
    
    pub fn run(&mut self) -> VMResult<Option<Value>> {
        while !self.halted && self.ip < self.block.instructions.len() {
            self.execute_instruction()?;
        }
        
        // Return the value in register 0 if it exists
        Ok(self.registers.get(0).cloned())
    }
    
    fn execute_instruction(&mut self) -> VMResult<()> {
        let instruction = &self.block.instructions[self.ip].clone();
        
        match instruction {
            RegInstr::Mov(dst, src) => {
                let value = self.resolve_reg_value(src)?;
                self.set_register(*dst, value)?;
                self.ip += 1;
            }
            
            RegInstr::Add(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.add_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::AddF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.add_float_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Sub(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.sub_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::SubF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.sub_float_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Mul(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.mul_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::MulF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.mul_float_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Div(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.div_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::DivF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.div_float_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Concat(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.concat_values(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Eq(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = Value::Bool(self.values_equal(&val1, &val2));
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Ne(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = Value::Bool(!self.values_equal(&val1, &val2));
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Lt(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.compare_values(&val1, &val2, |a, b| a < b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Le(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.compare_values(&val1, &val2, |a, b| a <= b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Gt(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.compare_values(&val1, &val2, |a, b| a > b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Ge(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.compare_values(&val1, &val2, |a, b| a >= b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::And(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.logical_and(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Or(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.logical_or(&val1, &val2)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Not(dst, src) => {
                let val = self.get_register(*src)?;
                let result = self.logical_not(&val)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::Load(dst, var_name) => {
                let value = self.variables.get(var_name)
                    .cloned()
                    .unwrap_or(Value::Null);
                self.set_register(*dst, value)?;
                self.ip += 1;
            }
            
            RegInstr::Store(var_name, src) => {
                let value = self.get_register(*src)?.clone();
                self.variables.insert(var_name.clone(), value);
                self.ip += 1;
            }
            
            RegInstr::Jmp(target) => {
                self.ip = *target;
            }
            
            RegInstr::Jz(condition, target) => {
                let val = self.get_register(*condition)?;
                if self.is_falsy(&val) {
                    self.ip = *target;
                } else {
                    self.ip += 1;
                }
            }
            
            RegInstr::Print(src) => {
                let value = self.get_register(*src)?;
                println!("{}", self.format_value(&value));
                self.ip += 1;
            }
            
            RegInstr::MakeList(dst, elements) => {
                let mut list = Vec::new();
                for &reg in elements {
                    list.push(self.get_register(reg)?.clone());
                }
                self.set_register(*dst, Value::List(list))?;
                self.ip += 1;
            }
            
            RegInstr::Len(dst, src) => {
                let val = self.get_register(*src)?;
                let length = match val {
                    Value::List(ref list) => list.len() as i64,
                    Value::Str(ref s) => s.len() as i64,
                    _ => return Err(VMError::TypeMismatch {
                        expected: "List or String".to_string(),
                        got: format!("{:?}", val),
                        operation: "LEN".to_string(),
                    }),
                };
                self.set_register(*dst, Value::Int(length))?;
                self.ip += 1;
            }
            
            RegInstr::Index(dst, container, index) => {
                let container_val = self.get_register(*container)?;
                let index_val = self.get_register(*index)?;
                let result = self.index_value(&container_val, &index_val)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::MakeObject(dst) => {
                self.set_register(*dst, Value::Object(HashMap::new()))?;
                self.ip += 1;
            }
            
            RegInstr::Halt => {
                self.halted = true;
            }
            
            RegInstr::Nop => {
                self.ip += 1;
            }
            
            // Simplified implementations for now
            RegInstr::Call(return_reg, _fn_id, _args) => {
                if let Some(reg) = return_reg {
                    self.set_register(*reg, Value::Null)?;
                }
                self.ip += 1;
            }
            
            RegInstr::Ret(_return_val) => {
                self.halted = true;
            }
            
            // Float comparison operations
            RegInstr::EqF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.float_compare_values(&val1, &val2, |a, b| (a - b).abs() < f64::EPSILON)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::NeF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.float_compare_values(&val1, &val2, |a, b| (a - b).abs() >= f64::EPSILON)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::LtF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.float_compare_values(&val1, &val2, |a, b| a < b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::LeF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.float_compare_values(&val1, &val2, |a, b| a <= b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::GtF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.float_compare_values(&val1, &val2, |a, b| a > b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            RegInstr::GeF(dst, src1, src2) => {
                let val1 = self.get_register(*src1)?;
                let val2 = self.get_register(*src2)?;
                let result = self.float_compare_values(&val1, &val2, |a, b| a >= b)?;
                self.set_register(*dst, result)?;
                self.ip += 1;
            }
            
            // Handle all the new instruction types we added
            RegInstr::True(dst) => {
                self.set_register(*dst, Value::Bool(true))?;
                self.ip += 1;
            }
            
            RegInstr::False(dst) => {
                self.set_register(*dst, Value::Bool(false))?;
                self.ip += 1;
            }
            
            RegInstr::Null(dst) => {
                self.set_register(*dst, Value::Null)?;
                self.ip += 1;
            }
            
            RegInstr::Store(var_name, src) => {
                let value = self.get_register(*src)?.clone();
                self.variables.insert(var_name.clone(), value);
                self.ip += 1;
            }
            
            RegInstr::Load(dst, var_name) => {
                let value = self.variables.get(var_name)
                    .ok_or_else(|| VMError::UndefinedVariable(var_name.clone()))?
                    .clone();
                self.set_register(*dst, value)?;
                self.ip += 1;
            }
            
            RegInstr::Delete(var_name) => {
                self.variables.remove(var_name);
                self.ip += 1;
            }
            
            // For concurrency operations that need TinyProc integration, 
            // return a specific error that indicates they need scheduler support
            RegInstr::Spawn(_, _) | RegInstr::Receive(_) | RegInstr::Send(_, _) | 
            RegInstr::Yield | RegInstr::Monitor(_, _) | RegInstr::Link(_) | 
            RegInstr::Unlink(_) | RegInstr::Register(_, _) | RegInstr::Whereis(_, _) => {
                return Err(VMError::UnsupportedOperation(
                    "Concurrency operations require TinyProc scheduler integration. Use regular VM with --no-smp or SMP scheduler.".to_string()
                ));
            }
            
            // For other complex operations not yet implemented, skip with NOP behavior
            _ => {
                // Skip unimplemented instructions for now
                self.ip += 1;
            }
        }
        
        Ok(())
    }
    
    fn resolve_reg_value(&self, reg_value: &RegValue) -> VMResult<Value> {
        match reg_value {
            RegValue::Const(value) => Ok(value.clone()),
            RegValue::Reg(reg_id) => self.get_register(*reg_id).map(|v| v.clone()),
        }
    }
    
    fn get_register(&self, reg_id: RegId) -> VMResult<&Value> {
        self.registers.get(reg_id as usize)
            .ok_or_else(|| VMError::ParseError {
                line: 0,
                instruction: format!("Invalid register: r{}", reg_id),
            })
    }
    
    fn set_register(&mut self, reg_id: RegId, value: Value) -> VMResult<()> {
        if reg_id as usize >= self.registers.len() {
            return Err(VMError::ParseError {
                line: 0,
                instruction: format!("Invalid register: r{}", reg_id),
            });
        }
        self.registers[reg_id as usize] = value;
        Ok(())
    }
    
    // Helper methods for arithmetic operations
    fn add_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
            _ => Err(VMError::TypeMismatch {
                expected: "Int".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "ADD".to_string(),
            }),
        }
    }
    
    fn add_float_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
            _ => Err(VMError::TypeMismatch {
                expected: "Float".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "ADD_F".to_string(),
            }),
        }
    }
    
    fn sub_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
            _ => Err(VMError::TypeMismatch {
                expected: "Int".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "SUB".to_string(),
            }),
        }
    }
    
    fn sub_float_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
            _ => Err(VMError::TypeMismatch {
                expected: "Float".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "SUB_F".to_string(),
            }),
        }
    }
    
    fn mul_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
            _ => Err(VMError::TypeMismatch {
                expected: "Int".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "MUL".to_string(),
            }),
        }
    }
    
    fn mul_float_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
            _ => Err(VMError::TypeMismatch {
                expected: "Float".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "MUL_F".to_string(),
            }),
        }
    }
    
    fn div_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                if *y == 0 {
                    Err(VMError::RuntimeError("Division by zero".to_string()))
                } else {
                    Ok(Value::Int(x / y))
                }
            }
            _ => Err(VMError::TypeMismatch {
                expected: "Int".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "DIV".to_string(),
            }),
        }
    }
    
    fn div_float_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Float(x), Value::Float(y)) => {
                if *y == 0.0 {
                    Err(VMError::RuntimeError("Division by zero".to_string()))
                } else {
                    Ok(Value::Float(x / y))
                }
            }
            _ => Err(VMError::TypeMismatch {
                expected: "Float".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "DIV_F".to_string(),
            }),
        }
    }
    
    fn concat_values(&self, a: &Value, b: &Value) -> VMResult<Value> {
        match (a, b) {
            (Value::Str(x), Value::Str(y)) => Ok(Value::Str(format!("{}{}", x, y))),
            _ => Err(VMError::TypeMismatch {
                expected: "String".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "CONCAT".to_string(),
            }),
        }
    }
    
    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Float(x), Value::Float(y)) => (x - y).abs() < f64::EPSILON,
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
    
    fn compare_values<F>(&self, a: &Value, b: &Value, op: F) -> VMResult<Value>
    where
        F: Fn(i64, i64) -> bool,
    {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(op(*x, *y))),
            _ => Err(VMError::TypeMismatch {
                expected: "Int".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "comparison".to_string(),
            }),
        }
    }
    
    fn float_compare_values<F>(&self, a: &Value, b: &Value, op: F) -> VMResult<Value>
    where
        F: Fn(f64, f64) -> bool,
    {
        match (a, b) {
            (Value::Float(x), Value::Float(y)) => Ok(Value::Bool(op(*x, *y))),
            _ => Err(VMError::TypeMismatch {
                expected: "Float".to_string(),
                got: format!("{:?} and {:?}", a, b),
                operation: "float comparison".to_string(),
            }),
        }
    }
    
    fn logical_and(&self, a: &Value, b: &Value) -> VMResult<Value> {
        let a_truthy = !self.is_falsy(a);
        let b_truthy = !self.is_falsy(b);
        Ok(Value::Bool(a_truthy && b_truthy))
    }
    
    fn logical_or(&self, a: &Value, b: &Value) -> VMResult<Value> {
        let a_truthy = !self.is_falsy(a);
        let b_truthy = !self.is_falsy(b);
        Ok(Value::Bool(a_truthy || b_truthy))
    }
    
    fn logical_not(&self, a: &Value) -> VMResult<Value> {
        Ok(Value::Bool(self.is_falsy(a)))
    }
    
    fn is_falsy(&self, value: &Value) -> bool {
        match value {
            Value::Bool(false) => true,
            Value::Null => true,
            Value::Int(0) => true,
            Value::Float(f) => *f == 0.0,
            Value::Str(s) => s.is_empty(),
            _ => false,
        }
    }
    
    fn index_value(&self, container: &Value, index: &Value) -> VMResult<Value> {
        match (container, index) {
            (Value::List(list), Value::Int(idx)) => {
                let idx = *idx as usize;
                list.get(idx)
                    .cloned()
                    .ok_or_else(|| VMError::IndexOutOfBounds {
                        index: idx,
                        length: list.len(),
                    })
            }
            _ => Err(VMError::TypeMismatch {
                expected: "List and Int".to_string(),
                got: format!("{:?} and {:?}", container, index),
                operation: "INDEX".to_string(),
            }),
        }
    }
    
    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::List(list) => {
                let elements: Vec<String> = list.iter().map(|v| self.format_value(v)).collect();
                format!("[{}]", elements.join(", "))
            }
            Value::Object(_) => "object".to_string(),
            _ => format!("{:?}", value),
        }
    }
}