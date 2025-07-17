use std::env;
use tiny_tot_vm::{OutputMode, VMConfig};

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub debug_mode: bool,
    pub optimize_mode: bool,
    pub gc_type: String,
    pub gc_debug: bool,
    pub gc_stats: bool,
    pub run_tests: bool,
    pub no_table: bool,
    pub trace_enabled: bool,
    pub profile_enabled: bool,
    pub smp_enabled: bool,
    pub trace_procs: bool,
    pub profile_procs: bool,
    pub use_ir: bool,
    pub command: CliCommand,
}

#[derive(Debug, Clone)]
pub enum CliCommand {
    Run { file: String },
    Compile { input: String, output: String },
    CompileLisp { input: String, output: String },
    Optimize { input: String, output: String },
    TestAll,
    TestConcurrency,
    TestMonitoringLinking,
    TestMultithreaded,
    TestMessagePassing,
    TestProcessSpawning,
    TestRegisterWhereis,
    TestYieldComprehensive,
    TestSpawnComprehensive,
    TestSendReceiveComprehensive,
    TestConcurrencyBytecode,
    TestSmpConcurrency,
    TestSupervisorTree,
    TestSelectiveReceive,
    TestTrapExit,
    TestProcessRegistry,
}

impl CliArgs {
    pub fn parse() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();

        if args.len() < 2 {
            return Err(Self::usage_string());
        }

        let mut debug_mode = false;
        let mut optimize_mode = false;
        let mut gc_type = "mark-sweep".to_string();
        let mut gc_debug = false;
        let mut gc_stats = false;
        let mut run_tests = false;
        let mut no_table = false;
        let mut trace_enabled = false;
        let mut profile_enabled = false;
        let mut smp_enabled = true;  // SMP is now the default
        let mut trace_procs = false;
        let mut profile_procs = false;
        let mut use_ir = false;
        let mut file_index = 1;

        // Parse flags
        while file_index < args.len() && args[file_index].starts_with("--") {
            match args[file_index].as_str() {
                "--debug" => {
                    debug_mode = true;
                    file_index += 1;
                }
                "--optimize" => {
                    optimize_mode = true;
                    file_index += 1;
                }
                "--gc" => {
                    if file_index + 1 >= args.len() {
                        return Err("--gc flag requires a garbage collector type".to_string());
                    }
                    gc_type = args[file_index + 1].clone();
                    if gc_type != "mark-sweep" && gc_type != "no-gc" {
                        return Err(format!("Unknown GC type: {}. Valid options: mark-sweep, no-gc", gc_type));
                    }
                    file_index += 2;
                }
                "--gc-debug" => {
                    gc_debug = true;
                    file_index += 1;
                }
                "--gc-stats" => {
                    gc_stats = true;
                    file_index += 1;
                }
                "--run-tests" => {
                    run_tests = true;
                    file_index += 1;
                }
                "--no-table" => {
                    no_table = true;
                    file_index += 1;
                }
                "--trace" => {
                    trace_enabled = true;
                    file_index += 1;
                }
                "--profile" => {
                    profile_enabled = true;
                    file_index += 1;
                }
                "--no-smp" => {
                    smp_enabled = false;
                    file_index += 1;
                }
                "--trace-procs" => {
                    trace_procs = true;
                    file_index += 1;
                }
                "--profile-procs" => {
                    profile_procs = true;
                    file_index += 1;
                }
                "--use-ir" => {
                    use_ir = true;
                    file_index += 1;
                }
                _ => {
                    return Err(format!("Unknown flag: {}", args[file_index]));
                }
            }
        }

        // Parse command
        let command = if file_index < args.len() {
            match args[file_index].as_str() {
                "compile" => {
                    if args.len() != file_index + 3 {
                        return Err("Usage: tinytotvm compile <input.ttvm> <output.ttb>".to_string());
                    }
                    CliCommand::Compile {
                        input: args[file_index + 1].clone(),
                        output: args[file_index + 2].clone(),
                    }
                }
                "optimize" => {
                    if args.len() != file_index + 3 {
                        return Err("Usage: tinytotvm optimize <input.ttvm> <output.ttvm>".to_string());
                    }
                    CliCommand::Optimize {
                        input: args[file_index + 1].clone(),
                        output: args[file_index + 2].clone(),
                    }
                }
                "compile-lisp" => {
                    if args.len() != file_index + 3 {
                        return Err("Usage: tinytotvm compile-lisp <input.lisp> <output.ttvm>".to_string());
                    }
                    CliCommand::CompileLisp {
                        input: args[file_index + 1].clone(),
                        output: args[file_index + 2].clone(),
                    }
                }
                "test-all" => CliCommand::TestAll,
                "test-concurrency" => CliCommand::TestConcurrency,
                "test-monitoring-linking" => CliCommand::TestMonitoringLinking,
                "test-multithreaded" => CliCommand::TestMultithreaded,
                "test-message-passing" => CliCommand::TestMessagePassing,
                "test-process-spawning" => CliCommand::TestProcessSpawning,
                "test-register-whereis" => CliCommand::TestRegisterWhereis,
                "test-yield-comprehensive" => CliCommand::TestYieldComprehensive,
                "test-spawn-comprehensive" => CliCommand::TestSpawnComprehensive,
                "test-send-receive-comprehensive" => CliCommand::TestSendReceiveComprehensive,
                "test-concurrency-bytecode" => CliCommand::TestConcurrencyBytecode,
                "test-smp-concurrency" => CliCommand::TestSmpConcurrency,
                "test-supervisor-tree" => CliCommand::TestSupervisorTree,
                "test-selective-receive" => CliCommand::TestSelectiveReceive,
                "test-trap-exit" => CliCommand::TestTrapExit,
                "test-process-registry" => CliCommand::TestProcessRegistry,
                _ => {
                    // Assume it's a file to run
                    CliCommand::Run { file: args[file_index].clone() }
                }
            }
        } else if run_tests {
            // Special case: if --run-tests is specified without a file, allow it
            CliCommand::Run { file: String::new() }
        } else {
            return Err("No program file specified".to_string());
        };

        Ok(CliArgs {
            debug_mode,
            optimize_mode,
            gc_type,
            gc_debug,
            gc_stats,
            run_tests,
            no_table,
            trace_enabled,
            profile_enabled,
            smp_enabled,
            trace_procs,
            profile_procs,
            use_ir,
            command,
        })
    }

    pub fn to_vm_config(&self) -> VMConfig {
        let output_mode = if self.no_table {
            OutputMode::Plain
        } else {
            OutputMode::PrettyTable
        };

        VMConfig {
            output_mode,
            run_tests: self.run_tests,
            gc_debug: self.gc_debug,
            gc_stats: self.gc_stats,
            debug_mode: self.debug_mode,
            optimize_mode: self.optimize_mode,
            gc_type: self.gc_type.clone(),
            trace_enabled: self.trace_enabled,
            profile_enabled: self.profile_enabled,
            smp_enabled: self.smp_enabled,
            trace_procs: self.trace_procs,
            profile_procs: self.profile_procs,
            use_ir: self.use_ir,
        }
    }

    fn usage_string() -> String {
        format!(
            "Usage: ttvm [--debug] [--optimize] [--gc <type>] [--gc-debug] [--gc-stats] [--run-tests] [--no-table] [--trace] [--profile] [--no-smp] [--trace-procs] [--profile-procs] [--use-ir] <program.ttvm|program.ttb>\n\
             \x20      ttvm compile <input.ttvm> <output.ttb>\n\
             \x20      ttvm compile-lisp <input.lisp> <output.ttvm>\n\
             \x20      ttvm optimize <input.ttvm> <output.ttvm>\n\
             \x20      ttvm test-all                                    # Run all examples and tests\n\
             \x20      ttvm test-concurrency                           # Run concurrency tests\n\
             \x20      ttvm test-multithreaded                         # Run multi-threaded scheduler tests\n\
             \x20      ttvm test-message-passing                       # Run message passing tests\n\
             \x20      ttvm test-process-spawning                      # Run process spawning tests\n\
             \x20      ttvm test-register-whereis                      # Run REGISTER/WHEREIS tests\n\
             \x20      ttvm test-yield-comprehensive                   # Run comprehensive YIELD tests\n\
             \x20      ttvm test-spawn-comprehensive                   # Run comprehensive SPAWN tests\n\
             \x20      ttvm test-send-receive-comprehensive            # Run comprehensive SEND/RECEIVE tests\n\
             \x20      ttvm test-concurrency-bytecode                  # Run concurrency bytecode compilation tests\n\
             \x20      ttvm test-smp-concurrency                       # Run SMP scheduler concurrency tests\n\
             \x20      ttvm test-coffee-shop                           # Run coffee shop message passing demo\n\
             \x20      ttvm test-supervisor-tree                       # Run supervisor tree tests\n\
             \x20      ttvm test-selective-receive                     # Run selective receive tests\n\
             \x20      ttvm test-trap-exit                             # Run trap_exit tests\n\
             \x20      ttvm test-process-registry                      # Run process registry cleanup tests\n\
             \n\
             GC Types: mark-sweep (default), no-gc\n\
             SMP Scheduler: Enabled by default with all CPU cores. Use --no-smp for single-threaded mode.\n\
             Debug Output: --run-tests enables unit test tables, --gc-debug enables GC debug tables\n\
             Table Control: --no-table disables formatted output in favor of plain text\n\
             Performance: --trace enables instruction tracing, --profile enables function profiling\n\
             Concurrency: Multi-core execution enabled by default, --trace-procs enables process tracing, --profile-procs enables process profiling\n\
             Execution Modes: --use-ir enables experimental register-based IR execution (basic programs only)"
        )
    }
}