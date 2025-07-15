use comfy_table::{Table, Cell, presets::UTF8_FULL, modifiers::UTF8_SOLID_INNER_BORDERS, Color, Attribute};
use colored::*;
use crate::{VMConfig, OutputMode};
use crate::vm::{OpCode, VM};
use crate::gc::GcStats;

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
}

pub fn run_vm_tests(config: &VMConfig) {
    if !config.run_tests {
        return;
    }

    let mut results = Vec::new();
    
    // Test 1: Basic arithmetic
    let program = vec![
        OpCode::PushInt(5),
        OpCode::PushInt(3),
        OpCode::Add,
        OpCode::Halt,
    ];
    let mut vm = VM::new(program);
    if vm.run().is_ok() && vm.stack.len() == 1 {
        let result = format!("{}", vm.stack[0]);
        results.push(TestResult {
            name: "Basic addition".to_string(),
            expected: "8".to_string(),
            actual: result.clone(),
            passed: result == "8",
        });
    } else {
        results.push(TestResult {
            name: "Basic addition".to_string(),
            expected: "8".to_string(),
            actual: "ERROR".to_string(),
            passed: false,
        });
    }

    // Test 2: String concatenation
    let program = vec![
        OpCode::PushStr("Hello".to_string()),
        OpCode::PushStr(" World".to_string()),
        OpCode::Concat,
        OpCode::Halt,
    ];
    let mut vm = VM::new(program);
    if vm.run().is_ok() && vm.stack.len() == 1 {
        let result = format!("{}", vm.stack[0]);
        results.push(TestResult {
            name: "String concat".to_string(),
            expected: "Hello World".to_string(),
            actual: result.clone(),
            passed: result == "Hello World",
        });
    } else {
        results.push(TestResult {
            name: "String concat".to_string(),
            expected: "Hello World".to_string(),
            actual: "ERROR".to_string(),
            passed: false,
        });
    }

    // Test 3: Variable storage and retrieval
    let program = vec![
        OpCode::PushInt(42),
        OpCode::Store("x".to_string()),
        OpCode::Load("x".to_string()),
        OpCode::Halt,
    ];
    let mut vm = VM::new(program);
    if vm.run().is_ok() && vm.stack.len() == 1 {
        let result = format!("{}", vm.stack[0]);
        results.push(TestResult {
            name: "Variable store/load".to_string(),
            expected: "42".to_string(),
            actual: result.clone(),
            passed: result == "42",
        });
    } else {
        results.push(TestResult {
            name: "Variable store/load".to_string(),
            expected: "42".to_string(),
            actual: "ERROR".to_string(),
            passed: false,
        });
    }

    report_test_results(&results, config);
}

pub fn report_test_results(results: &[TestResult], config: &VMConfig) {
    match config.output_mode {
        OutputMode::PrettyTable => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                 .apply_modifier(UTF8_SOLID_INNER_BORDERS);
            table.set_header(vec!["Test", "Expected", "Actual", "Result"]);

            for r in results {
                let status = if r.passed { "PASS" } else { "FAIL" };
                table.add_row(vec![
                    Cell::new(&r.name),
                    Cell::new(&r.expected),
                    Cell::new(&r.actual),
                    Cell::new(status),
                ]);
            }

            println!("=== Unit Test Results ===");
            println!("{table}");
        }
        OutputMode::Plain => {
            println!("=== Unit Test Results ===");
            for r in results {
                let status = if r.passed { "PASS" } else { "FAIL" };
                println!(
                    "{} | expected: {} | actual: {} | {}",
                    r.name, r.expected, r.actual, status
                );
            }
        }
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let total = results.len();
    println!("Tests passed: {}/{}", passed, total);
}

pub fn report_gc_stats(stats: &GcStats, config: &VMConfig) {
    match config.output_mode {
        OutputMode::PrettyTable => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                 .apply_modifier(UTF8_SOLID_INNER_BORDERS);
            table.set_header(vec![
                Cell::new("GC Metric").add_attribute(Attribute::Bold).fg(Color::Cyan),
                Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::White),
            ]);

            // Color code memory metrics
            let current_color = if stats.current_allocated > 10000 { Color::Red }
                              else if stats.current_allocated > 1000 { Color::Yellow }
                              else { Color::Green };

            table.add_row(vec![
                Cell::new("Total Allocated").fg(Color::White),
                Cell::new(&stats.total_allocated.to_string()).fg(Color::Blue),
            ]);
            table.add_row(vec![
                Cell::new("Total Freed").fg(Color::White),
                Cell::new(&stats.total_freed.to_string()).fg(Color::Green),
            ]);
            table.add_row(vec![
                Cell::new("Currently Allocated").fg(Color::White),
                Cell::new(&stats.current_allocated.to_string()).fg(current_color),
            ]);
            table.add_row(vec![
                Cell::new("Collections Performed").fg(Color::White),
                Cell::new(&stats.collections_performed.to_string()).fg(Color::Magenta),
            ]);

            println!("{}", "═══ GC Statistics ═══".bright_cyan().bold());
            println!("{table}");
        }
        OutputMode::Plain => {
            println!("{}", "═══ GC Statistics ═══".bright_cyan().bold());
            println!("{}: {}", "Total allocated".bright_cyan(), 
                     format!("{}", stats.total_allocated).blue());
            println!("{}: {}", "Total freed".bright_cyan(), 
                     format!("{}", stats.total_freed).green());
            println!("{}: {}", "Currently allocated".bright_cyan(), 
                     format!("{}", stats.current_allocated).yellow());
            println!("{}: {}", "Collections performed".bright_cyan(), 
                     format!("{}", stats.collections_performed).magenta());
        }
    }
}