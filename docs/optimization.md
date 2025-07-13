# Optimization Guide

TinyTotVM features a sophisticated **8-pass optimization engine** that provides significant performance improvements through various optimization techniques.

## Using Optimizations

### Command Line Usage
```bash
# Run with optimizations enabled
ttvm --optimize examples/program.ttvm

# Optimize and save program
ttvm optimize input.ttvm optimized.ttvm

# Combined with other flags
ttvm --optimize --debug --gc-stats examples/program.ttvm
```

## Optimization Passes

TinyTotVM includes 8 distinct optimization passes that work together to improve program performance:

1. **Constant Folding** - Evaluates expressions at compile time
2. **Constant Propagation** - Replaces variable loads with known values  
3. **Dead Code Elimination** - Removes unreachable code
4. **Peephole Optimizations** - Optimizes small instruction sequences
5. **Instruction Combining** - Merges instructions for efficiency
6. **Jump Threading** - Optimizes jump chains
7. **Tail Call Optimization** - Converts recursion to loops
8. **Memory Layout Optimization** - Optimizes memory access patterns

### Performance Results

| Test Case | Instruction Reduction |
|-----------|---------------------|
| **Dead Code Test** | 71% |
| **Comprehensive Test** | 46% |
| **Constant Folding** | 37% |
| **Math Operations** | 37% |

## Optimization Test Programs

```bash
# Test individual optimizations
ttvm --optimize examples/constant_folding_test.ttvm
ttvm --optimize examples/dead_code_test.ttvm
ttvm --optimize examples/tail_call_test.ttvm

# Comprehensive optimization showcase
ttvm --optimize examples/complete_optimization_showcase.ttvm
```