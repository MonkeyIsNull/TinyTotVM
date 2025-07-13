use crate::OpCode;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct OptimizationOptions {
    pub dead_code_elimination: bool,
    pub constant_folding: bool,
    pub tail_call_optimization: bool,
    pub memory_layout_optimization: bool,
    pub peephole_optimizations: bool,
    pub constant_propagation: bool,
    pub instruction_combining: bool,
    pub jump_threading: bool,
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        Self {
            dead_code_elimination: true,
            constant_folding: true,
            tail_call_optimization: true,
            memory_layout_optimization: true,
            peephole_optimizations: true,
            constant_propagation: true,
            instruction_combining: true,
            jump_threading: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationStats {
    pub dead_instructions_removed: usize,
    pub constants_folded: usize,
    pub tail_calls_optimized: usize,
    pub memory_operations_optimized: usize,
    pub peephole_optimizations_applied: usize,
    pub constants_propagated: usize,
    pub instructions_combined: usize,
    pub jumps_threaded: usize,
}

impl Default for OptimizationStats {
    fn default() -> Self {
        Self {
            dead_instructions_removed: 0,
            constants_folded: 0,
            tail_calls_optimized: 0,
            memory_operations_optimized: 0,
            peephole_optimizations_applied: 0,
            constants_propagated: 0,
            instructions_combined: 0,
            jumps_threaded: 0,
        }
    }
}

pub struct Optimizer {
    options: OptimizationOptions,
    stats: OptimizationStats,
}

impl Optimizer {
    pub fn new(options: OptimizationOptions) -> Self {
        Self {
            options,
            stats: OptimizationStats::default(),
        }
    }

    pub fn optimize(&mut self, instructions: Vec<OpCode>) -> (Vec<OpCode>, OptimizationStats) {
        let mut optimized = instructions;
        self.stats = OptimizationStats::default();

        // Apply optimization passes in order
        if self.options.constant_folding {
            optimized = self.constant_folding_pass(optimized);
        }

        if self.options.constant_propagation {
            optimized = self.constant_propagation_pass(optimized);
        }

        if self.options.instruction_combining {
            optimized = self.instruction_combining_pass(optimized);
        }

        if self.options.peephole_optimizations {
            optimized = self.peephole_optimization_pass(optimized);
        }

        if self.options.jump_threading {
            optimized = self.jump_threading_pass(optimized);
        }

        if self.options.dead_code_elimination {
            optimized = self.dead_code_elimination_pass(optimized);
        }

        if self.options.tail_call_optimization {
            optimized = self.tail_call_optimization_pass(optimized);
        }

        if self.options.memory_layout_optimization {
            optimized = self.memory_layout_optimization_pass(optimized);
        }

        (optimized, self.stats.clone())
    }

    fn constant_folding_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        let mut optimized = Vec::new();
        let mut i = 0;

        while i < instructions.len() {
            match self.try_fold_constants(&instructions, i) {
                Some((folded_instruction, consumed)) => {
                    optimized.push(folded_instruction);
                    self.stats.constants_folded += consumed - 1;
                    i += consumed;
                }
                None => {
                    optimized.push(instructions[i].clone());
                    i += 1;
                }
            }
        }

        optimized
    }

    fn try_fold_constants(&self, instructions: &[OpCode], start: usize) -> Option<(OpCode, usize)> {
        // Check for 2-instruction patterns first
        if start + 1 < instructions.len() {
            match (&instructions[start], &instructions[start + 1]) {
                (OpCode::True, OpCode::Not) => {
                    return Some((OpCode::False, 2));
                }
                (OpCode::False, OpCode::Not) => {
                    return Some((OpCode::True, 2));
                }
                _ => {}
            }
        }

        // Check for 3-instruction patterns
        if start + 2 >= instructions.len() {
            return None;
        }

        // Look for patterns like: PUSH_INT x, PUSH_INT y, ADD
        match (&instructions[start], &instructions[start + 1], &instructions[start + 2]) {
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Add) => {
                let result = a + b;
                Some((OpCode::PushInt(result), 3))
            }
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Sub) => {
                let result = a - b;
                Some((OpCode::PushInt(result), 3))
            }
            (OpCode::PushFloat(a), OpCode::PushFloat(b), OpCode::AddF) => {
                let result = a + b;
                Some((OpCode::PushFloat(result), 3))
            }
            (OpCode::PushFloat(a), OpCode::PushFloat(b), OpCode::SubF) => {
                let result = a - b;
                Some((OpCode::PushFloat(result), 3))
            }
            (OpCode::PushFloat(a), OpCode::PushFloat(b), OpCode::MulF) => {
                let result = a * b;
                Some((OpCode::PushFloat(result), 3))
            }
            (OpCode::PushFloat(a), OpCode::PushFloat(b), OpCode::DivF) if *b != 0.0 => {
                let result = a / b;
                Some((OpCode::PushFloat(result), 3))
            }
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Eq) => {
                let result = a == b;
                Some((if result { OpCode::True } else { OpCode::False }, 3))
            }
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Ne) => {
                let result = a != b;
                Some((if result { OpCode::True } else { OpCode::False }, 3))
            }
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Lt) => {
                let result = a < b;
                Some((if result { OpCode::True } else { OpCode::False }, 3))
            }
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Gt) => {
                let result = a > b;
                Some((if result { OpCode::True } else { OpCode::False }, 3))
            }
            _ => None,
        }
    }

    fn dead_code_elimination_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        let reachable = self.find_reachable_instructions(&instructions);
        let original_len = instructions.len();
        
        let optimized: Vec<OpCode> = instructions
            .into_iter()
            .enumerate()
            .filter(|(i, _)| reachable.contains(i))
            .map(|(_, instr)| instr)
            .collect();

        self.stats.dead_instructions_removed = original_len - optimized.len();
        optimized
    }

    fn find_reachable_instructions(&self, instructions: &[OpCode]) -> HashSet<usize> {
        let mut reachable = HashSet::new();
        let mut worklist = vec![0]; // Start from instruction 0

        while let Some(pc) = worklist.pop() {
            if pc >= instructions.len() || reachable.contains(&pc) {
                continue;
            }

            reachable.insert(pc);

            match &instructions[pc] {
                OpCode::Jmp(target) => {
                    worklist.push(*target);
                }
                OpCode::Jz(target) => {
                    worklist.push(*target);
                    worklist.push(pc + 1); // Fall through
                }
                OpCode::Call { addr, .. } => {
                    worklist.push(*addr);
                    worklist.push(pc + 1); // Continue after call
                }
                OpCode::Try { catch_addr } => {
                    worklist.push(*catch_addr);
                    worklist.push(pc + 1); // Continue in try block
                }
                OpCode::Ret | OpCode::Halt => {
                    // No fall through
                }
                _ => {
                    worklist.push(pc + 1); // Fall through to next instruction
                }
            }
        }

        reachable
    }

    fn tail_call_optimization_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        let mut optimized = Vec::new();
        let mut i = 0;

        while i < instructions.len() {
            if let Some(optimized_tail_call) = self.try_optimize_tail_call(&instructions, i) {
                optimized.extend(optimized_tail_call);
                self.stats.tail_calls_optimized += 1;
                
                // Skip the original call and ret instructions
                while i < instructions.len() && !matches!(instructions[i], OpCode::Ret) {
                    i += 1;
                }
                i += 1; // Skip the RET
            } else {
                optimized.push(instructions[i].clone());
                i += 1;
            }
        }

        optimized
    }

    fn try_optimize_tail_call(&self, instructions: &[OpCode], start: usize) -> Option<Vec<OpCode>> {
        // Look for pattern: CALL followed immediately by RET
        if start + 1 >= instructions.len() {
            return None;
        }

        match (&instructions[start], &instructions[start + 1]) {
            (OpCode::Call { addr, .. }, OpCode::Ret) => {
                // Convert tail call to jump
                Some(vec![OpCode::Jmp(*addr)])
            }
            (OpCode::CallFunction, OpCode::Ret) => {
                // For function pointer tail calls, we still need to handle the stack
                // This is a simplified optimization - in practice would need more analysis
                Some(vec![OpCode::CallFunction])
            }
            _ => None,
        }
    }

    fn memory_layout_optimization_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        let mut optimized = Vec::new();
        let mut i = 0;

        while i < instructions.len() {
            match self.try_optimize_memory_operations(&instructions, i) {
                Some((optimized_ops, consumed)) => {
                    let original_len = optimized_ops.len();
                    optimized.extend(optimized_ops);
                    self.stats.memory_operations_optimized += consumed - original_len;
                    i += consumed;
                }
                None => {
                    optimized.push(instructions[i].clone());
                    i += 1;
                }
            }
        }

        optimized
    }

    fn try_optimize_memory_operations(&self, instructions: &[OpCode], start: usize) -> Option<(Vec<OpCode>, usize)> {
        if start + 1 >= instructions.len() {
            return None;
        }

        // Optimize redundant load/store patterns
        match (&instructions[start], &instructions[start + 1]) {
            // STORE x followed by LOAD x -> STORE x, DUP
            // Note: This optimization assumes there's a value on the stack to store
            (OpCode::Store(var1), OpCode::Load(var2)) if var1 == var2 => {
                // Only optimize if we're sure there's a value to duplicate
                // For now, let's be conservative and skip this optimization
                None
            }
            _ => {
                // Look for longer patterns
                self.try_optimize_longer_memory_patterns(instructions, start)
            }
        }
    }

    fn try_optimize_longer_memory_patterns(&self, instructions: &[OpCode], start: usize) -> Option<(Vec<OpCode>, usize)> {
        if start + 2 >= instructions.len() {
            return None;
        }

        // Pattern: LOAD x, LOAD x -> LOAD x, DUP
        // Disabled for now to avoid stack issues
        None
    }

    // Peephole optimization pass - optimizes small instruction sequences
    fn peephole_optimization_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        let mut optimized = Vec::new();
        let mut i = 0;

        while i < instructions.len() {
            match self.try_peephole_optimization(&instructions, i) {
                Some((optimized_sequence, consumed)) => {
                    optimized.extend(optimized_sequence);
                    self.stats.peephole_optimizations_applied += 1;
                    i += consumed;
                }
                None => {
                    optimized.push(instructions[i].clone());
                    i += 1;
                }
            }
        }

        optimized
    }

    fn try_peephole_optimization(&self, instructions: &[OpCode], start: usize) -> Option<(Vec<OpCode>, usize)> {
        if start + 1 >= instructions.len() {
            return None;
        }

        // Optimize common patterns
        match (&instructions[start], &instructions[start + 1]) {
            // PUSH x, POP -> nothing (remove both)
            (OpCode::PushInt(_), OpCode::Dup) if start + 2 < instructions.len() => {
                // Look for patterns like PUSH, DUP, STORE where we can optimize
                None // Conservative for now
            }
            // JMP addr1, JMP addr2 -> JMP addr2 (dead jump elimination)
            (OpCode::Jmp(_), OpCode::Jmp(addr2)) => {
                Some((vec![OpCode::Jmp(*addr2)], 2))
            }
            // NOT NOT -> nothing (double negation elimination)
            (OpCode::Not, OpCode::Not) => {
                Some((vec![], 2))
            }
            _ => {
                // Check for 3-instruction patterns
                self.try_peephole_three_instruction(instructions, start)
            }
        }
    }

    fn try_peephole_three_instruction(&self, instructions: &[OpCode], start: usize) -> Option<(Vec<OpCode>, usize)> {
        if start + 2 >= instructions.len() {
            return None;
        }

        match (&instructions[start], &instructions[start + 1], &instructions[start + 2]) {
            // PUSH 0, <anything>, ADD -> <anything> (identity operation)
            (OpCode::PushInt(0), middle, OpCode::Add) => {
                Some((vec![middle.clone()], 3))
            }
            // PUSH 1, <anything>, MUL_F -> <anything> (identity operation for multiplication)
            (OpCode::PushFloat(f), middle, OpCode::MulF) if *f == 1.0 => {
                Some((vec![middle.clone()], 3))
            }
            // PUSH x, PUSH x, EQ -> TRUE (x == x is always true)
            (OpCode::PushInt(a), OpCode::PushInt(b), OpCode::Eq) if a == b => {
                Some((vec![OpCode::True], 3))
            }
            _ => None,
        }
    }

    // Constant propagation pass - tracks variable values and replaces loads with constants
    fn constant_propagation_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        use std::collections::HashMap;
        
        let mut optimized = Vec::new();
        let mut constants: HashMap<String, OpCode> = HashMap::new();

        for instruction in instructions {
            match instruction {
                // Track constant assignments
                OpCode::Store(ref var) => {
                    // If the last instruction was a constant push, remember it
                    if let Some(last) = optimized.last() {
                        match last {
                            OpCode::PushInt(_) | OpCode::PushFloat(_) | OpCode::PushStr(_) | 
                            OpCode::True | OpCode::False | OpCode::Null => {
                                constants.insert(var.clone(), last.clone());
                            }
                            _ => {
                                constants.remove(var);
                            }
                        }
                    }
                    optimized.push(instruction);
                }
                // Replace loads of constants with the constant value
                OpCode::Load(ref var) => {
                    if let Some(constant) = constants.get(var) {
                        optimized.push(constant.clone());
                        self.stats.constants_propagated += 1;
                    } else {
                        optimized.push(instruction);
                    }
                }
                // Clear constants on variables that might be modified
                _ => {
                    optimized.push(instruction);
                }
            }
        }

        optimized
    }

    // Instruction combining pass - combines multiple instructions into more efficient forms
    fn instruction_combining_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        let mut optimized = Vec::new();
        let mut i = 0;

        while i < instructions.len() {
            match self.try_combine_instructions(&instructions, i) {
                Some((combined_sequence, consumed)) => {
                    let original_len = combined_sequence.len();
                    optimized.extend(combined_sequence);
                    self.stats.instructions_combined += consumed - original_len;
                    i += consumed;
                }
                None => {
                    optimized.push(instructions[i].clone());
                    i += 1;
                }
            }
        }

        optimized
    }

    fn try_combine_instructions(&self, instructions: &[OpCode], start: usize) -> Option<(Vec<OpCode>, usize)> {
        if start + 1 >= instructions.len() {
            return None;
        }

        match (&instructions[start], &instructions[start + 1]) {
            // Combine duplicate detection: DUP followed by EQ -> TRUE (x == x)
            (OpCode::Dup, OpCode::Eq) => {
                Some((vec![OpCode::True], 2))
            }
            // Combine stack manipulation: PUSH 0, JZ addr -> JMP addr (always jumps)
            (OpCode::PushInt(0), OpCode::Jz(addr)) => {
                Some((vec![OpCode::Jmp(*addr)], 2))
            }
            (OpCode::False, OpCode::Jz(addr)) => {
                Some((vec![OpCode::Jmp(*addr)], 2))
            }
            // Combine: TRUE, JZ addr -> nothing (never jumps)
            (OpCode::True, OpCode::Jz(_)) => {
                Some((vec![], 2))
            }
            _ => {
                self.try_combine_three_instructions(instructions, start)
            }
        }
    }

    fn try_combine_three_instructions(&self, instructions: &[OpCode], start: usize) -> Option<(Vec<OpCode>, usize)> {
        if start + 2 >= instructions.len() {
            return None;
        }

        match (&instructions[start], &instructions[start + 1], &instructions[start + 2]) {
            // PUSH x, STORE var, LOAD var -> PUSH x, DUP, STORE var
            (push_op @ (OpCode::PushInt(_) | OpCode::PushFloat(_) | OpCode::PushStr(_)), 
             OpCode::Store(var1), OpCode::Load(var2)) if var1 == var2 => {
                Some((vec![push_op.clone(), OpCode::Dup, OpCode::Store(var1.clone())], 3))
            }
            _ => None,
        }
    }

    // Jump threading pass - optimizes jump chains
    fn jump_threading_pass(&mut self, instructions: Vec<OpCode>) -> Vec<OpCode> {
        use std::collections::HashMap;
        
        // Build jump target map
        let mut jump_targets: HashMap<usize, usize> = HashMap::new();
        
        // Find all direct jumps and their ultimate targets
        for (i, instruction) in instructions.iter().enumerate() {
            if let OpCode::Jmp(target) = instruction {
                jump_targets.insert(i, *target);
            }
        }

        // Thread jumps - if we jump to another jump, jump directly to final target
        let mut optimized = Vec::new();
        for (_i, instruction) in instructions.iter().enumerate() {
            match instruction {
                OpCode::Jmp(target) => {
                    let final_target = self.find_ultimate_jump_target(*target, &instructions);
                    if final_target != *target {
                        optimized.push(OpCode::Jmp(final_target));
                        self.stats.jumps_threaded += 1;
                    } else {
                        optimized.push(instruction.clone());
                    }
                }
                OpCode::Jz(target) => {
                    let final_target = self.find_ultimate_jump_target(*target, &instructions);
                    if final_target != *target {
                        optimized.push(OpCode::Jz(final_target));
                        self.stats.jumps_threaded += 1;
                    } else {
                        optimized.push(instruction.clone());
                    }
                }
                _ => {
                    optimized.push(instruction.clone());
                }
            }
        }

        optimized
    }

    fn find_ultimate_jump_target(&self, mut target: usize, instructions: &[OpCode]) -> usize {
        let mut visited = HashSet::new();
        
        while target < instructions.len() && !visited.contains(&target) {
            visited.insert(target);
            
            if let OpCode::Jmp(new_target) = &instructions[target] {
                target = *new_target;
            } else {
                break;
            }
        }
        
        target
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }
}

// Analysis utilities
impl Optimizer {
    pub fn analyze_program(&self, instructions: &[OpCode]) -> ProgramAnalysis {
        let mut analysis = ProgramAnalysis::new();
        
        for (_i, instr) in instructions.iter().enumerate() {
            match instr {
                OpCode::PushInt(_) | OpCode::PushFloat(_) | OpCode::PushStr(_) | OpCode::True | OpCode::False | OpCode::Null => {
                    analysis.constant_count += 1;
                }
                OpCode::Call { .. } | OpCode::CallFunction => {
                    analysis.call_count += 1;
                }
                OpCode::Load(_) | OpCode::Store(_) => {
                    analysis.memory_op_count += 1;
                }
                OpCode::Jmp(_) | OpCode::Jz(_) => {
                    analysis.jump_count += 1;
                }
                _ => {}
            }
        }

        analysis.total_instructions = instructions.len();
        analysis
    }
}

#[derive(Debug)]
pub struct ProgramAnalysis {
    pub total_instructions: usize,
    pub constant_count: usize,
    pub call_count: usize,
    pub memory_op_count: usize,
    pub jump_count: usize,
}

impl ProgramAnalysis {
    fn new() -> Self {
        Self {
            total_instructions: 0,
            constant_count: 0,
            call_count: 0,
            memory_op_count: 0,
            jump_count: 0,
        }
    }
}