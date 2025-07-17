use std::time::{Duration, Instant};
use std::collections::HashMap;
use comfy_table::{Table, Cell, presets::UTF8_FULL, modifiers::UTF8_SOLID_INNER_BORDERS, Color, Attribute};
use colored::*;

use crate::vm::VM;
use crate::ir::{lowering::StackToRegisterLowering, vm::RegisterVM};
use crate::concurrency::pool::SchedulerPool;
use crate::VMConfig;

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub stack_duration: Duration,
    pub ir_duration: Duration,
    pub stack_instructions: usize,
    pub ir_instructions: usize,
    pub stack_memory_peak: usize,
    pub ir_memory_peak: usize,
    pub speedup_factor: f64,
    pub has_concurrency: bool,
}

impl BenchmarkResult {
    pub fn new(name: String, stack_duration: Duration, ir_duration: Duration, 
               stack_instructions: usize, ir_instructions: usize,
               stack_memory_peak: usize, ir_memory_peak: usize,
               has_concurrency: bool) -> Self {
        let speedup_factor = if ir_duration.as_nanos() > 0 {
            stack_duration.as_nanos() as f64 / ir_duration.as_nanos() as f64
        } else {
            1.0
        };
        
        BenchmarkResult {
            name,
            stack_duration,
            ir_duration,
            stack_instructions,
            ir_instructions,
            stack_memory_peak,
            ir_memory_peak,
            speedup_factor,
            has_concurrency,
        }
    }
}

#[derive(Debug)]
pub struct BenchmarkSuite {
    pub results: Vec<BenchmarkResult>,
    pub total_stack_time: Duration,
    pub total_ir_time: Duration,
}

impl BenchmarkSuite {
    pub fn new() -> Self {
        BenchmarkSuite {
            results: Vec::new(),
            total_stack_time: Duration::ZERO,
            total_ir_time: Duration::ZERO,
        }
    }
    
    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.total_stack_time += result.stack_duration;
        self.total_ir_time += result.ir_duration;
        self.results.push(result);
    }
    
    pub fn average_speedup(&self) -> f64 {
        if self.results.is_empty() {
            return 1.0;
        }
        
        let sum: f64 = self.results.iter().map(|r| r.speedup_factor).sum();
        sum / self.results.len() as f64
    }
    
    pub fn overall_speedup(&self) -> f64 {
        if self.total_ir_time.as_nanos() > 0 {
            self.total_stack_time.as_nanos() as f64 / self.total_ir_time.as_nanos() as f64
        } else {
            1.0
        }
    }
}

pub struct PerformanceComparison {
    config: VMConfig,
}

impl PerformanceComparison {
    pub fn new(config: VMConfig) -> Self {
        PerformanceComparison { config }
    }
    
    pub fn benchmark_program(&self, name: &str, program: Vec<crate::vm::OpCode>) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let has_concurrency = self.program_has_concurrency(&program);
        
        println!("Benchmarking {}: {}", name, 
                if has_concurrency { "Concurrent" } else { "Sequential" });
        
        // Benchmark Stack-based execution
        let (stack_duration, stack_instructions, stack_memory_peak) = 
            self.benchmark_stack_execution(&program, has_concurrency)?;
        
        // Benchmark IR-based execution  
        let (ir_duration, ir_instructions, ir_memory_peak) = 
            self.benchmark_ir_execution(&program, has_concurrency)?;
        
        let result = BenchmarkResult::new(
            name.to_string(),
            stack_duration,
            ir_duration,
            stack_instructions,
            ir_instructions,
            stack_memory_peak,
            ir_memory_peak,
            has_concurrency
        );
        
        Ok(result)
    }
    
    fn benchmark_stack_execution(&self, program: &[crate::vm::OpCode], has_concurrency: bool) 
        -> Result<(Duration, usize, usize), Box<dyn std::error::Error>> {
        
        let start_time = Instant::now();
        let mut instruction_count = 0;
        let mut memory_peak = 0;
        
        if has_concurrency {
            // Use SMP scheduler for concurrent programs
            let mut scheduler_pool = SchedulerPool::new_with_default_threads();
            let (_main_proc_id, _main_sender) = scheduler_pool.spawn_process(program.to_vec());
            scheduler_pool.run()?;
            scheduler_pool.wait_for_completion();
            
            // Note: Instruction counting for SMP scheduler is complex, 
            // using program length as approximation
            instruction_count = program.len();
            memory_peak = program.len() * 8; // Rough memory estimate
        } else {
            // Use regular VM for sequential programs
            let mut vm = VM::new(program.to_vec());
            vm.run()?;
            
            // Get metrics from VM
            instruction_count = vm.instruction_count;
            memory_peak = vm.stack.len() * 8; // Rough memory estimate
        }
        
        let duration = start_time.elapsed();
        Ok((duration, instruction_count, memory_peak))
    }
    
    fn benchmark_ir_execution(&self, program: &[crate::vm::OpCode], has_concurrency: bool) 
        -> Result<(Duration, usize, usize), Box<dyn std::error::Error>> {
        
        let start_time = Instant::now();
        let mut instruction_count = 0;
        let mut memory_peak = 0;
        
        // Convert to IR first
        let ir_block = StackToRegisterLowering::lower(program)?;
        
        if has_concurrency {
            // Use SMP scheduler with IR translation (hybrid approach)
            let mut scheduler_pool = SchedulerPool::new_with_default_threads();
            let (_main_proc_id, _main_sender) = scheduler_pool.spawn_process(program.to_vec());
            scheduler_pool.run()?;
            scheduler_pool.wait_for_completion();
            
            instruction_count = ir_block.instructions.len();
            memory_peak = ir_block.register_count as usize * 8; // Register memory
        } else {
            // Use direct IR execution for sequential programs
            let mut ir_vm = RegisterVM::new(ir_block.clone());
            let _result = ir_vm.run()?;
            
            instruction_count = ir_block.instructions.len();
            memory_peak = ir_vm.registers.len() * 8; // Register memory
        }
        
        let duration = start_time.elapsed();
        Ok((duration, instruction_count, memory_peak))
    }
    
    fn program_has_concurrency(&self, program: &[crate::vm::OpCode]) -> bool {
        program.iter().any(|op| matches!(op, 
            crate::vm::OpCode::Spawn | crate::vm::OpCode::Receive | crate::vm::OpCode::ReceiveMatch(_) | 
            crate::vm::OpCode::Yield | crate::vm::OpCode::Send(_) | crate::vm::OpCode::Monitor(_) | 
            crate::vm::OpCode::Demonitor(_) | crate::vm::OpCode::Link(_) | crate::vm::OpCode::Unlink(_) | 
            crate::vm::OpCode::TrapExit | crate::vm::OpCode::Register(_) | crate::vm::OpCode::Unregister(_) | 
            crate::vm::OpCode::Whereis(_) | crate::vm::OpCode::SendNamed(_) | crate::vm::OpCode::StartSupervisor | 
            crate::vm::OpCode::SuperviseChild(_) | crate::vm::OpCode::RestartChild(_)
        ))
    }
    
    pub fn run_benchmark_suite(&self, benchmark_programs: &Vec<(String, Vec<crate::vm::OpCode>)>) 
        -> Result<BenchmarkSuite, Box<dyn std::error::Error>> {
        
        let mut suite = BenchmarkSuite::new();
        
        println!("{}", "═══ IR vs Stack Performance Comparison ═══".bright_cyan().bold());
        println!("Running {} benchmark programs...\n", benchmark_programs.len());
        
        for (name, program) in benchmark_programs {
            match self.benchmark_program(name, program.clone()) {
                Ok(result) => {
                    println!("{} completed", name.green());
                    suite.add_result(result);
                }
                Err(e) => {
                    println!("{} failed: {}", name.red(), e);
                }
            }
        }
        
        Ok(suite)
    }
    
    pub fn print_results(&self, suite: &BenchmarkSuite) {
        if suite.results.is_empty() {
            println!("No benchmark results to display.");
            return;
        }
        
        println!("\n{}", "═══ Performance Comparison Results ═══".bright_cyan().bold());
        
        match self.config.output_mode {
            crate::OutputMode::PrettyTable => {
                self.print_detailed_table(suite);
                self.print_summary_table(suite);
            }
            crate::OutputMode::Plain => {
                self.print_plain_results(suite);
            }
        }
    }
    
    fn print_detailed_table(&self, suite: &BenchmarkSuite) {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL)
             .apply_modifier(UTF8_SOLID_INNER_BORDERS);
        
        table.set_header(vec![
            Cell::new("Benchmark").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Type").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Stack Time (μs)").add_attribute(Attribute::Bold).fg(Color::Blue),
            Cell::new("IR Time (μs)").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Speedup").add_attribute(Attribute::Bold).fg(Color::Yellow),
            Cell::new("Stack Inst.").add_attribute(Attribute::Bold).fg(Color::Magenta),
            Cell::new("IR Inst.").add_attribute(Attribute::Bold).fg(Color::Magenta),
        ]);
        
        for result in &suite.results {
            let benchmark_type = if result.has_concurrency { "Concurrent" } else { "Sequential" };
            let type_color = if result.has_concurrency { Color::Red } else { Color::Blue };
            
            let speedup_color = if result.speedup_factor > 1.2 { Color::Green }
                              else if result.speedup_factor > 0.8 { Color::Yellow }
                              else { Color::Red };
            
            table.add_row(vec![
                Cell::new(&result.name).fg(Color::White),
                Cell::new(benchmark_type).fg(type_color),
                Cell::new(format!("{:.1}", result.stack_duration.as_micros())).fg(Color::Blue),
                Cell::new(format!("{:.1}", result.ir_duration.as_micros())).fg(Color::Green),
                Cell::new(format!("{:.2}x", result.speedup_factor)).fg(speedup_color),
                Cell::new(result.stack_instructions.to_string()).fg(Color::Magenta),
                Cell::new(result.ir_instructions.to_string()).fg(Color::Magenta),
            ]);
        }
        
        println!("{}", table);
    }
    
    fn print_summary_table(&self, suite: &BenchmarkSuite) {
        let mut summary_table = Table::new();
        summary_table.load_preset(UTF8_FULL)
                     .apply_modifier(UTF8_SOLID_INNER_BORDERS);
        summary_table.set_header(vec![
            Cell::new("Performance Summary").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::White),
        ]);
        
        let overall_speedup = suite.overall_speedup();
        let average_speedup = suite.average_speedup();
        
        let overall_color = if overall_speedup > 1.2 { Color::Green }
                          else if overall_speedup > 0.8 { Color::Yellow }
                          else { Color::Red };
        
        let average_color = if average_speedup > 1.2 { Color::Green }
                          else if average_speedup > 0.8 { Color::Yellow }
                          else { Color::Red };
        
        summary_table.add_row(vec![
            Cell::new("Total Stack Time").fg(Color::White),
            Cell::new(format!("{:.2} ms", suite.total_stack_time.as_secs_f64() * 1000.0)).fg(Color::Blue),
        ]);
        summary_table.add_row(vec![
            Cell::new("Total IR Time").fg(Color::White),
            Cell::new(format!("{:.2} ms", suite.total_ir_time.as_secs_f64() * 1000.0)).fg(Color::Green),
        ]);
        summary_table.add_row(vec![
            Cell::new("Overall Speedup").fg(Color::White),
            Cell::new(format!("{:.2}x", overall_speedup)).fg(overall_color),
        ]);
        summary_table.add_row(vec![
            Cell::new("Average Speedup").fg(Color::White),
            Cell::new(format!("{:.2}x", average_speedup)).fg(average_color),
        ]);
        summary_table.add_row(vec![
            Cell::new("Benchmarks Run").fg(Color::White),
            Cell::new(suite.results.len().to_string()).fg(Color::Cyan),
        ]);
        
        let concurrent_count = suite.results.iter().filter(|r| r.has_concurrency).count();
        let sequential_count = suite.results.len() - concurrent_count;
        
        summary_table.add_row(vec![
            Cell::new("Sequential Programs").fg(Color::White),
            Cell::new(sequential_count.to_string()).fg(Color::Blue),
        ]);
        summary_table.add_row(vec![
            Cell::new("Concurrent Programs").fg(Color::White),
            Cell::new(concurrent_count.to_string()).fg(Color::Red),
        ]);
        
        println!("{}", summary_table);
    }
    
    fn print_plain_results(&self, suite: &BenchmarkSuite) {
        println!("{}", "Individual Results:".bright_cyan().bold());
        
        for result in &suite.results {
            let benchmark_type = if result.has_concurrency { "Concurrent" } else { "Sequential" };
            println!("  {} ({}):", result.name.white(), benchmark_type.yellow());
            println!("    Stack: {:.1} μs ({} instructions)", 
                     result.stack_duration.as_micros(), result.stack_instructions);
            println!("    IR:    {:.1} μs ({} instructions)", 
                     result.ir_duration.as_micros(), result.ir_instructions);
            println!("    Speedup: {:.2}x", result.speedup_factor);
            println!();
        }
        
        println!("{}", "Summary:".bright_cyan().bold());
        println!("  Total Stack Time: {:.2} ms", suite.total_stack_time.as_secs_f64() * 1000.0);
        println!("  Total IR Time: {:.2} ms", suite.total_ir_time.as_secs_f64() * 1000.0);
        println!("  Overall Speedup: {:.2}x", suite.overall_speedup());
        println!("  Average Speedup: {:.2}x", suite.average_speedup());
        println!("  Benchmarks: {} total ({} sequential, {} concurrent)",
                 suite.results.len(),
                 suite.results.iter().filter(|r| !r.has_concurrency).count(),
                 suite.results.iter().filter(|r| r.has_concurrency).count());
    }
}