use std::fs;
use std::path::Path;
use std::process::Command;

// Test that IR execution works for simple programs
#[test]
fn test_ir_equivalence_with_stack_vm() {
    // Test with simple examples that don't use concurrency features
    let simple_examples = ["countdown.ttvm", "bool_test.ttvm", "comparison.ttvm"];
    
    for example in &simple_examples {
        let file_path = format!("examples/{}", example);
        
        // Skip if file doesn't exist
        if !Path::new(&file_path).exists() {
            continue;
        }
        
        // Run with regular stack VM
        let stack_output = Command::new("cargo")
            .args(&["run", "--quiet", "--", "--no-smp", &file_path])
            .output()
            .expect("Failed to run stack VM");

        // Only proceed if stack VM succeeds (some examples need concurrency)
        if !stack_output.status.success() {
            continue;
        }

        // Run with IR VM
        let ir_output = Command::new("cargo")
            .args(&["run", "--quiet", "--", "--use-ir", &file_path])
            .output()
            .expect("Failed to run IR VM");

        assert!(
            ir_output.status.success(),
            "IR VM failed for {}: {}",
            example,
            String::from_utf8_lossy(&ir_output.stderr)
        );

        // Outputs should be equivalent (ignoring VM-specific debug messages)
        let stack_stdout = filter_vm_output(&stack_output.stdout);
        let ir_stdout = filter_vm_output(&ir_output.stdout);
        
        assert_eq!(
            stack_stdout, ir_stdout,
            "Output mismatch for {}:\nStack VM: {}\nIR VM: {}",
            example, stack_stdout, ir_stdout
        );
    }
}

// Helper function to filter out VM-specific debug messages
fn filter_vm_output(output: &[u8]) -> String {
    let output_str = String::from_utf8_lossy(output);
    output_str
        .lines()
        .filter(|line| {
            !line.contains("Debug: Using regular VM") &&
            !line.contains("SMP enabled flag:") &&
            !line.contains("Running with IR") &&
            !line.contains("IR execution completed") &&
            !line.contains("Running with BEAM-style")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[test]
fn test_ir_basic_arithmetic() {
    // Test basic arithmetic operations with countdown example
    let arithmetic_output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--use-ir", "examples/countdown.ttvm"])
        .output()
        .expect("Failed to run IR arithmetic test");

    assert!(
        arithmetic_output.status.success(),
        "IR arithmetic test failed: {}",
        String::from_utf8_lossy(&arithmetic_output.stderr)
    );
}

#[test]
fn test_ir_control_flow() {
    // Test control flow operations with if-else example
    let control_output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--use-ir", "examples/if_else.ttvm"])
        .output()
        .expect("Failed to run IR control flow test");

    assert!(
        control_output.status.success(),
        "IR control flow test failed: {}",
        String::from_utf8_lossy(&control_output.stderr)
    );
}

#[test]
fn test_ir_boolean_operations() {
    // Test boolean operations
    let bool_output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--use-ir", "examples/bool_test.ttvm"])
        .output()
        .expect("Failed to run IR boolean test");

    assert!(
        bool_output.status.success(),
        "IR boolean test failed: {}",
        String::from_utf8_lossy(&bool_output.stderr)
    );
}