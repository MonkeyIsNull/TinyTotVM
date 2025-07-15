use std::collections::HashMap;
use std::time::{Duration, Instant};
use comfy_table::{Table, Cell, presets::UTF8_FULL, modifiers::UTF8_SOLID_INNER_BORDERS, Color, Attribute};
use colored::*;

#[derive(Debug, Clone)]
pub struct FunctionProfiler {
    pub start_time: Instant,
    pub instruction_count: usize,
}

impl FunctionProfiler {
    pub fn new() -> Self {
        FunctionProfiler {
            start_time: Instant::now(),
            instruction_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Profiler {
    pub function_timings: HashMap<String, Duration>,
    pub instruction_counts: HashMap<String, usize>,
    pub call_counts: HashMap<String, usize>,
    pub peak_stack_depth: usize,
    pub total_allocations: usize,
    pub peak_heap_size: usize,
    pub current_function_stack: Vec<(String, FunctionProfiler)>,
    pub call_depth: usize,
}

impl Profiler {
    pub fn new() -> Self {
        Profiler {
            function_timings: HashMap::new(),
            instruction_counts: HashMap::new(),
            call_counts: HashMap::new(),
            peak_stack_depth: 0,
            total_allocations: 0,
            peak_heap_size: 0,
            current_function_stack: Vec::new(),
            call_depth: 0,
        }
    }

    pub fn start_function(&mut self, function_name: String) {
        self.call_depth += 1;
        let profiler = FunctionProfiler::new();
        self.current_function_stack.push((function_name.clone(), profiler));
        
        // Update call count
        *self.call_counts.entry(function_name).or_insert(0) += 1;
    }

    pub fn end_function(&mut self) -> Option<String> {
        if let Some((function_name, profiler)) = self.current_function_stack.pop() {
            self.call_depth = self.call_depth.saturating_sub(1);
            
            let elapsed = profiler.start_time.elapsed();
            
            // Add to total timing for this function
            *self.function_timings.entry(function_name.clone()).or_insert(Duration::ZERO) += elapsed;
            
            // Add to total instruction count for this function
            *self.instruction_counts.entry(function_name.clone()).or_insert(0) += profiler.instruction_count;
            
            Some(function_name)
        } else {
            None
        }
    }

    pub fn record_instruction(&mut self) {
        // Increment instruction count for current function
        if let Some((_, profiler)) = self.current_function_stack.last_mut() {
            profiler.instruction_count += 1;
        }
    }

    pub fn update_stack_depth(&mut self, depth: usize) {
        if depth > self.peak_stack_depth {
            self.peak_stack_depth = depth;
        }
    }

    pub fn record_allocation(&mut self, size: usize) {
        self.total_allocations += 1;
        if size > self.peak_heap_size {
            self.peak_heap_size = size;
        }
    }

    pub fn print_results(&self, config: &crate::VMConfig) {
        if self.function_timings.is_empty() && self.call_counts.is_empty() {
            return;
        }

        println!("\n{}", "═══ Profiling Results ═══".bright_cyan().bold());
        
        match config.output_mode {
            crate::OutputMode::PrettyTable => {
                // Function Summary Table
                let mut table = Table::new();
                table.load_preset(UTF8_FULL)
                     .apply_modifier(UTF8_SOLID_INNER_BORDERS);
                
                // Set colored headers
                table.set_header(vec![
                    Cell::new("Function").add_attribute(Attribute::Bold).fg(Color::Cyan),
                    Cell::new("Calls").add_attribute(Attribute::Bold).fg(Color::Green),
                    Cell::new("Time (ms)").add_attribute(Attribute::Bold).fg(Color::Yellow),
                    Cell::new("Instructions").add_attribute(Attribute::Bold).fg(Color::Blue),
                    Cell::new("Avg Time/Call (μs)").add_attribute(Attribute::Bold).fg(Color::Magenta),
                ]);
                
                let mut functions: Vec<_> = self.function_timings.keys().collect();
                functions.sort();
                
                for function_name in functions {
                    let timing = self.function_timings.get(function_name).unwrap();
                    let instructions = self.instruction_counts.get(function_name).unwrap_or(&0);
                    let calls = self.call_counts.get(function_name).unwrap_or(&0);
                    let avg_time_us = if *calls > 0 {
                        timing.as_micros() as f64 / *calls as f64
                    } else {
                        0.0
                    };
                    
                    // Color code performance metrics
                    let time_ms = timing.as_secs_f64() * 1000.0;
                    let time_color = if time_ms > 10.0 { Color::Red } 
                                   else if time_ms > 1.0 { Color::Yellow } 
                                   else { Color::Green };
                    
                    let calls_color = if *calls > 100 { Color::Red }
                                    else if *calls > 10 { Color::Yellow }
                                    else { Color::Green };
                    
                    table.add_row(vec![
                        Cell::new(function_name).fg(Color::White),
                        Cell::new(calls.to_string()).fg(calls_color),
                        Cell::new(format!("{:.3}", time_ms)).fg(time_color),
                        Cell::new(instructions.to_string()).fg(Color::Blue),
                        Cell::new(format!("{:.1}", avg_time_us)).fg(Color::Magenta),
                    ]);
                }
                
                println!("{}", table);
                
                // Performance Summary Table
                let mut summary_table = Table::new();
                summary_table.load_preset(UTF8_FULL)
                             .apply_modifier(UTF8_SOLID_INNER_BORDERS);
                summary_table.set_header(vec![
                    Cell::new("Performance Metric").add_attribute(Attribute::Bold).fg(Color::Cyan),
                    Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::White),
                ]);
                
                // Color code memory metrics
                let stack_color = if self.peak_stack_depth > 50 { Color::Red }
                                else if self.peak_stack_depth > 20 { Color::Yellow }
                                else { Color::Green };
                
                let heap_color = if self.peak_heap_size > 10000 { Color::Red }
                               else if self.peak_heap_size > 1000 { Color::Yellow }
                               else { Color::Green };
                
                summary_table.add_row(vec![
                    Cell::new("Peak Stack Depth").fg(Color::White),
                    Cell::new(format!("{} frames", self.peak_stack_depth)).fg(stack_color),
                ]);
                summary_table.add_row(vec![
                    Cell::new("Total Allocations").fg(Color::White),
                    Cell::new(self.total_allocations.to_string()).fg(Color::Blue),
                ]);
                summary_table.add_row(vec![
                    Cell::new("Peak Heap Size").fg(Color::White),
                    Cell::new(format!("{} bytes", self.peak_heap_size)).fg(heap_color),
                ]);
                
                println!("{}", summary_table);
            }
            crate::OutputMode::Plain => {
                println!("{}", "Function Summary:".bright_cyan().bold());
                let mut functions: Vec<_> = self.function_timings.keys().collect();
                functions.sort();
                
                for function_name in functions {
                    let timing = self.function_timings.get(function_name).unwrap();
                    let instructions = self.instruction_counts.get(function_name).unwrap_or(&0);
                    let calls = self.call_counts.get(function_name).unwrap_or(&0);
                    let time_ms = timing.as_secs_f64() * 1000.0;
                    
                    println!("  {} - {} calls - {} ms - {} instructions", 
                        function_name.white(),
                        format!("{}", calls).green(),
                        format!("{:.3}", time_ms).yellow(),
                        format!("{}", instructions).blue());
                }
                
                println!("\n{}: {} frames", "Peak Stack Depth".bright_cyan(), 
                         format!("{}", self.peak_stack_depth).green());
                println!("{}: {}", "Total Allocations".bright_cyan(), 
                         format!("{}", self.total_allocations).blue());
                println!("{}: {} bytes", "Peak Heap Size".bright_cyan(), 
                         format!("{}", self.peak_heap_size).green());
            }
        }
    }
}