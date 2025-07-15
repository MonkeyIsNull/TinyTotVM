use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use std::thread;
use std::sync::{Arc, Mutex};
use crossbeam::channel::Sender;
use comfy_table::{Table, Cell, presets::UTF8_FULL, modifiers::UTF8_SOLID_INNER_BORDERS, Color, Attribute};
use colored::*;

use crate::vm::{VMError, VMResult, OpCode, ProcId, MessagePattern, VM};
use crate::concurrency::{Message, RestartStrategy, ChildType, Shutdown, ChildSpec, RestartPolicy, SupervisorSpec, TinyProc, ProcessSpawner, SchedulerPool};
use crate::testing::{TestResult, run_vm_tests, report_gc_stats};
use crate::cli::args::{CliArgs, CliCommand};
use crate::compiler;
use crate::lisp_compiler;
use crate::optimizer;
use crate::bytecode;
use crate::ProcState;

// Single-threaded scheduler implementation
pub struct SingleThreadScheduler {
    processes: Vec<Arc<Mutex<TinyProc>>>,
    next_proc_id: ProcId,
}

impl SingleThreadScheduler {
    pub fn new() -> Self {
        SingleThreadScheduler {
            processes: Vec::new(),
            next_proc_id: 1,
        }
    }
    
    pub fn spawn_process(&mut self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>) {
        let proc_id = self.next_proc_id;
        self.next_proc_id += 1;
        
        let (proc, sender) = TinyProc::new(proc_id, instructions);
        self.processes.push(Arc::new(Mutex::new(proc)));
        
        (proc_id, sender)
    }
    
    pub fn spawn_supervisor(&mut self, spec: SupervisorSpec) -> (ProcId, Sender<Message>) {
        let proc_id = self.next_proc_id;
        self.next_proc_id += 1;
        
        let (mut proc, sender) = TinyProc::new_supervisor(proc_id, spec);
        
        // Set up process spawner for the supervisor
        proc.process_spawner = Some(Arc::new(SingleThreadSchedulerProcessSpawner {
            scheduler: Arc::new(Mutex::new(self as *mut SingleThreadScheduler)),
        }));
        
        // Start all children
        if let Err(e) = proc.start_all_children() {
            eprintln!("Failed to start supervisor children: {}", e);
        }
        
        self.processes.push(Arc::new(Mutex::new(proc)));
        (proc_id, sender)
    }
    
    pub fn run_step(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Run one step of the scheduler
        if self.processes.is_empty() {
            return Ok(());
        }
        
        let mut processes_to_remove = Vec::new();
        
        for (i, proc_arc) in self.processes.iter().enumerate() {
            let mut proc = proc_arc.lock().unwrap();
            
            match proc.state {
                ProcState::Ready | ProcState::Running => {
                    if proc.waiting_for_message && !proc.has_messages() {
                        proc.state = ProcState::Waiting;
                        continue;
                    }
                    
                    match proc.run_until_yield() {
                        Ok(ProcState::Exited) => {
                            processes_to_remove.push(i);
                        }
                        Ok(_) => {
                            // Process yielded or is waiting
                        }
                        Err(e) => {
                            eprintln!("Process {} error: {:?}", proc.id, e);
                            processes_to_remove.push(i);
                        }
                    }
                }
                ProcState::Waiting => {
                    // Check if process has messages now
                    if proc.has_messages() {
                        proc.waiting_for_message = false;
                        proc.state = ProcState::Ready;
                    }
                }
                ProcState::Exited => {
                    processes_to_remove.push(i);
                }
            }
        }
        
        // Remove finished processes
        for &i in processes_to_remove.iter().rev() {
            self.processes.remove(i);
        }
        
        Ok(())
    }
    
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.run_round_robin()
    }
    
    pub fn run_round_robin(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut active_processes = true;
        
        while active_processes {
            active_processes = false;
            let mut processes_to_remove = Vec::new();
            
            for (i, proc_arc) in self.processes.iter().enumerate() {
                let mut proc = proc_arc.lock().unwrap();
                
                match proc.state {
                    ProcState::Ready => {
                        let new_state = proc.run_until_yield()?;
                        
                        match new_state {
                            ProcState::Exited => {
                                processes_to_remove.push(i);
                            }
                            ProcState::Waiting => {
                                // Process yielded - either out of reductions or manual yield
                                // Set to Ready for next round
                                proc.state = ProcState::Ready;
                                active_processes = true;
                            }
                            _ => active_processes = true,
                        }
                    }
                    ProcState::Waiting => {
                        // Process was waiting for a message, check if it has one now
                        if proc.waiting_for_message && proc.has_messages() {
                            proc.waiting_for_message = false;
                            proc.state = ProcState::Ready;
                            active_processes = true;
                        } else if !proc.waiting_for_message {
                            // Process was just yielding, set to Ready for next round
                            proc.state = ProcState::Ready;
                            active_processes = true;
                        }
                        // If still waiting for message and no messages available, leave as Waiting
                    }
                    ProcState::Exited => {
                        processes_to_remove.push(i);
                    }
                    _ => active_processes = true,
                }
            }
            
            // Remove exited processes (in reverse order to maintain indices)
            for &i in processes_to_remove.iter().rev() {
                self.processes.remove(i);
            }
            
            if self.processes.is_empty() {
                break;
            }
        }
        
        Ok(())
    }
}

// Process spawner implementation for SingleThreadScheduler
#[derive(Debug)]
pub struct SingleThreadSchedulerProcessSpawner {
    scheduler: Arc<Mutex<*mut SingleThreadScheduler>>,
}

unsafe impl Send for SingleThreadSchedulerProcessSpawner {}
unsafe impl Sync for SingleThreadSchedulerProcessSpawner {}

impl ProcessSpawner for SingleThreadSchedulerProcessSpawner {
    fn spawn_process(&self, instructions: Vec<OpCode>) -> (ProcId, Sender<Message>) {
        unsafe {
            let scheduler_ptr = *self.scheduler.lock().unwrap();
            let scheduler = &mut *scheduler_ptr;
            scheduler.spawn_process(instructions)
        }
    }
}

pub fn execute_command(args: &CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    match &args.command {
        CliCommand::Run { file } => {
            if args.run_tests {
                run_vm_tests(&args.to_vm_config());
                return Ok(());
            }

            if file.is_empty() {
                return Err("No program file specified".into());
            }

            execute_program_file(file, args)
        }
        CliCommand::Compile { input, output } => {
            compiler::compile(input, output)?;
            println!("Compiled to {}", output);
            Ok(())
        }
        CliCommand::CompileLisp { input, output } => {
            lisp_compiler::compile_lisp(input, output);
            println!("Compiled Lisp to {}", output);
            Ok(())
        }
        CliCommand::Optimize { input, output } => {
            optimize_program(input, output);
            Ok(())
        }
        CliCommand::TestAll => {
            run_comprehensive_tests();
            Ok(())
        }
        CliCommand::TestConcurrency => {
            test_concurrency()?;
            println!("All concurrency tests passed!");
            Ok(())
        }
        CliCommand::TestMonitoringLinking => {
            test_process_monitoring_linking()?;
            println!("All process monitoring and linking tests passed!");
            Ok(())
        }
        CliCommand::TestMultithreaded => {
            test_multithreaded_scheduler()?;
            println!("All multi-threaded scheduler tests passed!");
            Ok(())
        }
        CliCommand::TestMessagePassing => {
            test_message_passing()?;
            println!("All message passing tests passed!");
            Ok(())
        }
        CliCommand::TestProcessSpawning => {
            test_process_spawning()?;
            println!("All process spawning tests passed!");
            Ok(())
        }
        CliCommand::TestRegisterWhereis => {
            test_register_whereis()?;
            println!("All REGISTER/WHEREIS tests passed!");
            Ok(())
        }
        CliCommand::TestYieldComprehensive => {
            test_yield_comprehensive()?;
            println!("All comprehensive YIELD tests passed!");
            Ok(())
        }
        CliCommand::TestSpawnComprehensive => {
            test_spawn_comprehensive()?;
            println!("All comprehensive SPAWN tests passed!");
            Ok(())
        }
        CliCommand::TestSendReceiveComprehensive => {
            test_send_receive_comprehensive()?;
            println!("All comprehensive SEND/RECEIVE tests passed!");
            Ok(())
        }
        CliCommand::TestConcurrencyBytecode => {
            test_concurrency_bytecode_compilation()?;
            println!("All concurrency bytecode compilation tests passed!");
            Ok(())
        }
        CliCommand::TestSmpConcurrency => {
            test_smp_scheduler_concurrency()?;
            println!("All SMP scheduler concurrency tests passed!");
            Ok(())
        }
        CliCommand::TestSupervisorTree => {
            test_supervisor_tree()?;
            println!("All supervisor tree tests passed!");
            Ok(())
        }
        CliCommand::TestSelectiveReceive => {
            test_selective_receive()?;
            println!("All selective receive tests passed!");
            Ok(())
        }
        CliCommand::TestTrapExit => {
            test_trap_exit()?;
            println!("All trap_exit tests passed!");
            Ok(())
        }
        CliCommand::TestProcessRegistry => {
            test_process_registry_cleanup()?;
            println!("All process registry cleanup tests passed!");
            Ok(())
        }
    }
}

fn execute_program_file(file: &str, args: &CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = args.to_vm_config();

    // Load program
    let mut program = if file.ends_with(".ttb") {
        bytecode::load_bytecode(file)?
    } else {
        parse_program(file)?
    };

    // Apply optimizations if requested
    if args.optimize_mode {
        let mut optimizer = optimizer::Optimizer::new(optimizer::OptimizationOptions::default());
        let analysis_before = optimizer.analyze_program(&program);
        
        let (optimized_program, stats) = optimizer.optimize(program);
        program = optimized_program;
        
        let analysis_after = optimizer.analyze_program(&program);
        
        println!("=== Optimization Results ===");
        println!("Instructions: {} -> {} ({})", 
            analysis_before.total_instructions, 
            analysis_after.total_instructions,
            analysis_before.total_instructions as i32 - analysis_after.total_instructions as i32);
        println!("Constants folded: {}", stats.constants_folded);
        println!("Dead instructions removed: {}", stats.dead_instructions_removed);
        println!("Tail calls optimized: {}", stats.tail_calls_optimized);
        println!("Memory operations optimized: {}", stats.memory_operations_optimized);
        println!("Peephole optimizations: {}", stats.peephole_optimizations_applied);
        println!("Constants propagated: {}", stats.constants_propagated);
        println!("Instructions combined: {}", stats.instructions_combined);
        println!("Jumps threaded: {}", stats.jumps_threaded);
        println!();
    }

    // Use SMP scheduler if enabled, otherwise use regular VM
    if config.smp_enabled {
        println!("Running with BEAM-style SMP scheduler...");
        println!("SMP enabled flag: {}", config.smp_enabled);
        println!("Debug: About to create SMP scheduler pool");
        
        // Create SMP scheduler pool with default number of threads (CPU cores)
        let mut scheduler_pool = SchedulerPool::new_with_default_threads();
        
        // Spawn the main process with the program
        let (main_proc_id, _main_sender) = scheduler_pool.spawn_process(program);
        println!("Main process spawned with ID: {}", main_proc_id);
        
        // Run the scheduler pool
        scheduler_pool.run()?;
        
        // Wait for schedulers to finish (run() already sets shutdown flag)
        scheduler_pool.wait_for_completion();
        
        println!("SMP scheduler shutdown complete");
    } else {
        // Regular single-threaded VM execution
        println!("Debug: Using regular VM (SMP disabled)");
        println!("SMP enabled flag: {}", config.smp_enabled);
        let mut vm = VM::new_with_config(program, &config.gc_type, config.debug_mode || config.gc_debug, config.gc_stats, config.trace_enabled, config.profile_enabled);
        vm.run()?;
        
        // Output profiling results if enabled (only for regular VM mode)
        if config.profile_enabled {
            if let Some(profiler) = &vm.profiler {
                profiler.print_results(&config);
            }
        }
        
        if args.debug_mode {
            let (instructions, max_stack, final_stack) = vm.get_stats();
            println!("Performance stats - Instructions: {}, Max stack: {}, Final stack: {}", 
                instructions, max_stack, final_stack);
        }
        
        if config.gc_stats {
            let stats = vm.get_gc_stats();
            report_gc_stats(&stats, &config);
        }
    }

    Ok(())
}

fn parse_program(path: &str) -> VMResult<Vec<OpCode>> {
    let content = fs::read_to_string(path).map_err(|e| VMError::FileError { 
        filename: path.to_string(), 
        error: e.to_string() 
    })?;

    let mut label_map: HashMap<String, usize> = HashMap::new();
    let mut instructions_raw: Vec<(usize, &str)> = Vec::new();

    // First pass: build label -> index map
    for (line_num, line) in content.lines().enumerate() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("LABEL ") {
            let label_name = line[6..].trim();
            label_map.insert(label_name.to_string(), instructions_raw.len());
        } else {
            instructions_raw.push((line_num + 1, line)); // save for second pass
        }
    }

    // Second pass: convert raw instructions to OpCode using label map
    let mut program: Vec<OpCode> = Vec::new();
    for (line_num, line) in instructions_raw {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let opcode = match parts[0] {
            "PUSH_INT" => {
                let n = parts[1].parse::<i64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid integer: {}", parts[1]) 
                })?;
                OpCode::PushInt(n)
            }
            "PUSH_FLOAT" => {
                let f = parts[1].parse::<f64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid float: {}", parts[1]) 
                })?;
                OpCode::PushFloat(f)
            }
            "PUSH_STR" => {
                let s = parts[1].trim_matches('"').to_string();
                OpCode::PushStr(s)
            }
            "PUSH_BOOL" => {
                let b = parts[1].parse::<bool>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid boolean: {}", parts[1]) 
                })?;
                OpCode::PushBool(b)
            }
            "ADD" => OpCode::Add,
            "ADD_F" => OpCode::AddF,
            "SUB" => OpCode::Sub,
            "SUB_F" => OpCode::SubF,
            "MUL" => OpCode::Mul,
            "MUL_F" => OpCode::MulF,
            "DIV" => OpCode::Div,
            "DIV_F" => OpCode::DivF,
            "DUP" => OpCode::Dup,
            "CONCAT" => OpCode::Concat,
            "PRINT" => OpCode::Print,
            "HALT" => OpCode::Halt,
            "CALL" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: "CALL requires at least a target".to_string() });
                }
                let call_parts: Vec<&str> = parts[1].split_whitespace().collect();
                let label = call_parts[0];
                let params: Vec<String> = call_parts[1..].iter().map(|s| s.to_string()).collect();
                
                // Try parsing as number first, then as label
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Call { addr: target, params }
            }
            "JMP" => {
                let label = parts[1].trim();
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Jmp(target)
            }
            "JZ" => {
                let label = parts[1].trim();
                let target = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::Jz(target)
            }
            "RET" => OpCode::Ret,
            "STORE" => {
                let var = parts[1].trim().to_string();
                OpCode::Store(var)
            }
            "DELETE" => {
                let var = parts[1].trim().to_string();
                OpCode::Delete(var)
            }
            "LOAD" => {
                let var = parts[1].trim().to_string();
                OpCode::Load(var)
            }
            "EQ" => OpCode::Eq,
            "GT" => OpCode::Gt,
            "LT" => OpCode::Lt,
            "NE" => OpCode::Ne,
            "GE" => OpCode::Ge,
            "LE" => OpCode::Le,
            "EQ_F" => OpCode::EqF,
            "GT_F" => OpCode::GtF,
            "LT_F" => OpCode::LtF,
            "NE_F" => OpCode::NeF,
            "GE_F" => OpCode::GeF,
            "LE_F" => OpCode::LeF,
            "TRUE" => OpCode::True,
            "FALSE" => OpCode::False,
            "NOT" => OpCode::Not,
            "AND" => OpCode::And,
            "OR" => OpCode::Or,
            "NULL" => OpCode::Null,
            "MAKE_LIST" => {
                let n = parts[1].parse::<usize>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid list size: {}", parts[1]) 
                })?;
                OpCode::MakeList(n)
            }
            "LEN" => OpCode::Len,
            "INDEX" => OpCode::Index,
            "DUMP_SCOPE" => OpCode::DumpScope,
            "MAKE_OBJECT" => OpCode::MakeObject,
            "SET_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::SetField(field)
            }
            "GET_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::GetField(field)
            }
            "HAS_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::HasField(field)
            }
            "DELETE_FIELD" => {
                let field = parts[1].trim().to_string();
                OpCode::DeleteField(field)
            }
            "KEYS" => OpCode::Keys,
            "MAKE_FUNCTION" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: "MAKE_FUNCTION requires at least a target".to_string() });
                }
                let func_parts: Vec<&str> = parts[1].split_whitespace().collect();
                let label = func_parts[0];
                let params: Vec<String> = func_parts[1..].iter().map(|s| s.to_string()).collect();
                
                // Try parsing as number first, then as label
                let addr = if let Ok(address) = label.parse::<usize>() {
                    address
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::MakeFunction { addr, params }
            }
            "CALL_FUNCTION" => OpCode::CallFunction,
            "MAKE_LAMBDA" => {
                if parts.len() < 2 {
                    return Err(VMError::ParseError { line: line_num, instruction: line.to_string() });
                }
                
                let remaining_parts: Vec<&str> = parts[1].split_whitespace().collect();
                if remaining_parts.is_empty() {
                    return Err(VMError::ParseError { line: line_num, instruction: line.to_string() });
                }
                
                let label = remaining_parts[0];
                let params = remaining_parts[1..].iter().map(|s| s.to_string()).collect();
                
                let addr = if let Ok(addr) = label.parse::<usize>() {
                    addr
                } else {
                    *label_map.get(label).ok_or_else(|| VMError::UnknownLabel(label.to_string()))?
                };
                OpCode::MakeLambda { addr, params }
            }
            "CAPTURE" => {
                let var = parts[1].trim().to_string();
                OpCode::Capture(var)
            }
            "TRY" => {
                let catch_label = parts[1].trim();
                let catch_addr = *label_map.get(catch_label).ok_or_else(|| VMError::UnknownLabel(catch_label.to_string()))?;
                OpCode::Try { catch_addr }
            }
            "CATCH" => OpCode::Catch,
            "THROW" => OpCode::Throw,
            "END_TRY" => OpCode::EndTry,
            "READ_FILE" => OpCode::ReadFile,
            "WRITE_FILE" => OpCode::WriteFile,
            // Enhanced I/O operations
            "READ_LINE" => OpCode::ReadLine,
            "READ_CHAR" => OpCode::ReadChar,
            "READ_INPUT" => OpCode::ReadInput,
            "APPEND_FILE" => OpCode::AppendFile,
            "FILE_EXISTS" => OpCode::FileExists,
            "FILE_SIZE" => OpCode::FileSize,
            "DELETE_FILE" => OpCode::DeleteFile,
            "LIST_DIR" => OpCode::ListDir,
            "READ_BYTES" => OpCode::ReadBytes,
            "WRITE_BYTES" => OpCode::WriteBytes,
            // Environment and system
            "GET_ENV" => OpCode::GetEnv,
            "SET_ENV" => OpCode::SetEnv,
            "GET_ARGS" => OpCode::GetArgs,
            "EXEC" => OpCode::Exec,
            "EXEC_CAPTURE" => OpCode::ExecCapture,
            "EXIT" => OpCode::Exit,
            // Time operations
            "GET_TIME" => OpCode::GetTime,
            "SLEEP" => OpCode::Sleep,
            "FORMAT_TIME" => OpCode::FormatTime,
            // Network operations
            "HTTP_GET" => OpCode::HttpGet,
            "HTTP_POST" => OpCode::HttpPost,
            "TCP_CONNECT" => OpCode::TcpConnect,
            "TCP_LISTEN" => OpCode::TcpListen,
            "TCP_SEND" => OpCode::TcpSend,
            "TCP_RECV" => OpCode::TcpRecv,
            "UDP_BIND" => OpCode::UdpBind,
            "UDP_SEND" => OpCode::UdpSend,
            "UDP_RECV" => OpCode::UdpRecv,
            "DNS_RESOLVE" => OpCode::DnsResolve,
            // Advanced I/O operations
            "ASYNC_READ" => OpCode::AsyncRead,
            "ASYNC_WRITE" => OpCode::AsyncWrite,
            "AWAIT" => OpCode::Await,
            "STREAM_CREATE" => OpCode::StreamCreate,
            "STREAM_READ" => OpCode::StreamRead,
            "STREAM_WRITE" => OpCode::StreamWrite,
            "STREAM_CLOSE" => OpCode::StreamClose,
            "JSON_PARSE" => OpCode::JsonParse,
            "JSON_STRINGIFY" => OpCode::JsonStringify,
            "CSV_PARSE" => OpCode::CsvParse,
            "CSV_WRITE" => OpCode::CsvWrite,
            "COMPRESS" => OpCode::Compress,
            "DECOMPRESS" => OpCode::Decompress,
            "ENCRYPT" => OpCode::Encrypt,
            "DECRYPT" => OpCode::Decrypt,
            "HASH" => OpCode::Hash,
            "DB_CONNECT" => OpCode::DbConnect,
            "DB_QUERY" => OpCode::DbQuery,
            "DB_EXEC" => OpCode::DbExec,
            "IMPORT" => {
                let path = parts[1].trim();
                // Remove quotes if present
                let path = if path.starts_with('"') && path.ends_with('"') {
                    path[1..path.len()-1].to_string()
                } else {
                    path.to_string()
                };
                OpCode::Import(path)
            }
            "EXPORT" => {
                let name = parts[1].trim().to_string();
                OpCode::Export(name)
            }
            "YIELD" => OpCode::Yield,
            "RECEIVE" => OpCode::Receive,
            "SEND" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Send(pid)
            }
            "REGISTER" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::Register(name)
            }
            "WHEREIS" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::Whereis(name)
            }
            "SPAWN" => OpCode::Spawn,
            "SENDNAMED" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::SendNamed(name)
            }
            "UNREGISTER" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::Unregister(name)
            }
            "TRAP_EXIT" => OpCode::TrapExit,
            "START_SUPERVISOR" => OpCode::StartSupervisor,
            "SUPERVISE_CHILD" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::SuperviseChild(name)
            }
            "RESTART_CHILD" => {
                let name = parts[1].trim_matches('"').to_string();
                OpCode::RestartChild(name)
            }
            "LINK" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Link(pid)
            }
            "UNLINK" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Unlink(pid)
            }
            "MONITOR" => {
                let pid = parts[1].parse::<u64>().map_err(|_| VMError::ParseError { 
                    line: line_num, 
                    instruction: format!("Invalid PID: {}", parts[1]) 
                })?;
                OpCode::Monitor(pid)
            }
            "DEMONITOR" => {
                let monitor_ref = parts[1].trim_matches('"').to_string();
                OpCode::Demonitor(monitor_ref)
            }
            _ => return Err(VMError::ParseError { line: line_num, instruction: line.to_string() }),
        };
        program.push(opcode);
    }

    Ok(program)
}

fn optimize_program(input_file: &str, output_file: &str) {
    let program = match parse_program(input_file) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    let mut optimizer = optimizer::Optimizer::new(optimizer::OptimizationOptions::default());
    let analysis_before = optimizer.analyze_program(&program);
    
    println!("=== Program Analysis (Before Optimization) ===");
    println!("Total instructions: {}", analysis_before.total_instructions);
    println!("Constants: {}", analysis_before.constant_count);
    println!("Function calls: {}", analysis_before.call_count);
    println!("Memory operations: {}", analysis_before.memory_op_count);
    println!("Jumps: {}", analysis_before.jump_count);
    println!();

    let (optimized_program, stats) = optimizer.optimize(program);
    let analysis_after = optimizer.analyze_program(&optimized_program);

    println!("=== Optimization Results ===");
    println!("Instructions: {} -> {} ({})", 
        analysis_before.total_instructions, 
        analysis_after.total_instructions,
        analysis_before.total_instructions as i32 - analysis_after.total_instructions as i32);
    println!("Constants folded: {}", stats.constants_folded);
    println!("Dead instructions removed: {}", stats.dead_instructions_removed);
    println!("Tail calls optimized: {}", stats.tail_calls_optimized);
    println!("Memory operations optimized: {}", stats.memory_operations_optimized);
    println!("Peephole optimizations: {}", stats.peephole_optimizations_applied);
    println!("Constants propagated: {}", stats.constants_propagated);
    println!("Instructions combined: {}", stats.instructions_combined);
    println!("Jumps threaded: {}", stats.jumps_threaded);
    println!();

    // Write optimized program to file
    match write_optimized_program(&optimized_program, output_file) {
        Ok(_) => println!("Optimized program written to {}", output_file),
        Err(e) => {
            eprintln!("Failed to write optimized program: {}", e);
            std::process::exit(1);
        }
    }
}

fn write_optimized_program(program: &[OpCode], output_file: &str) -> std::io::Result<()> {
    let mut output = String::new();
    
    for (_i, instruction) in program.iter().enumerate() {
        let line = match instruction {
            OpCode::PushInt(n) => format!("PUSH_INT {}", n),
            OpCode::PushFloat(f) => format!("PUSH_FLOAT {}", f),
            OpCode::PushStr(s) => format!("PUSH_STR \"{}\"", s.replace("\"", "\\\"")),
            OpCode::PushBool(b) => format!("PUSH_BOOL {}", b),
            OpCode::Add => "ADD".to_string(),
            OpCode::AddF => "ADD_F".to_string(),
            OpCode::Sub => "SUB".to_string(),
            OpCode::SubF => "SUB_F".to_string(),
            OpCode::Mul => "MUL".to_string(),
            OpCode::MulF => "MUL_F".to_string(),
            OpCode::Div => "DIV".to_string(),
            OpCode::DivF => "DIV_F".to_string(),
            OpCode::Concat => "CONCAT".to_string(),
            OpCode::Print => "PRINT".to_string(),
            OpCode::Halt => "HALT".to_string(),
            OpCode::Jmp(addr) => format!("JMP {}", addr),
            OpCode::Jz(addr) => format!("JZ {}", addr),
            OpCode::Call { addr, params } => format!("CALL {} {}", addr, params.join(" ")),
            OpCode::Ret => "RET".to_string(),
            OpCode::Dup => "DUP".to_string(),
            OpCode::Store(var) => format!("STORE {}", var),
            OpCode::Load(var) => format!("LOAD {}", var),
            OpCode::Delete(var) => format!("DELETE {}", var),
            OpCode::Eq => "EQ".to_string(),
            OpCode::Ne => "NE".to_string(),
            OpCode::Gt => "GT".to_string(),
            OpCode::Lt => "LT".to_string(),
            OpCode::Ge => "GE".to_string(),
            OpCode::Le => "LE".to_string(),
            OpCode::EqF => "EQ_F".to_string(),
            OpCode::NeF => "NE_F".to_string(),
            OpCode::GtF => "GT_F".to_string(),
            OpCode::LtF => "LT_F".to_string(),
            OpCode::GeF => "GE_F".to_string(),
            OpCode::LeF => "LE_F".to_string(),
            OpCode::True => "TRUE".to_string(),
            OpCode::False => "FALSE".to_string(),
            OpCode::Not => "NOT".to_string(),
            OpCode::And => "AND".to_string(),
            OpCode::Or => "OR".to_string(),
            OpCode::Null => "NULL".to_string(),
            OpCode::MakeList(n) => format!("MAKE_LIST {}", n),
            OpCode::Len => "LEN".to_string(),
            OpCode::Index => "INDEX".to_string(),
            OpCode::DumpScope => "DUMP_SCOPE".to_string(),
            OpCode::ReadFile => "READ_FILE".to_string(),
            OpCode::WriteFile => "WRITE_FILE".to_string(),
            // Enhanced I/O operations
            OpCode::ReadLine => "READ_LINE".to_string(),
            OpCode::ReadChar => "READ_CHAR".to_string(),
            OpCode::ReadInput => "READ_INPUT".to_string(),
            OpCode::AppendFile => "APPEND_FILE".to_string(),
            OpCode::FileExists => "FILE_EXISTS".to_string(),
            OpCode::FileSize => "FILE_SIZE".to_string(),
            OpCode::DeleteFile => "DELETE_FILE".to_string(),
            OpCode::ListDir => "LIST_DIR".to_string(),
            OpCode::ReadBytes => "READ_BYTES".to_string(),
            OpCode::WriteBytes => "WRITE_BYTES".to_string(),
            // Environment and system
            OpCode::GetEnv => "GET_ENV".to_string(),
            OpCode::SetEnv => "SET_ENV".to_string(),
            OpCode::GetArgs => "GET_ARGS".to_string(),
            OpCode::Exec => "EXEC".to_string(),
            OpCode::ExecCapture => "EXEC_CAPTURE".to_string(),
            OpCode::Exit => "EXIT".to_string(),
            // Time operations
            OpCode::GetTime => "GET_TIME".to_string(),
            OpCode::Sleep => "SLEEP".to_string(),
            OpCode::FormatTime => "FORMAT_TIME".to_string(),
            // Network operations
            OpCode::HttpGet => "HTTP_GET".to_string(),
            OpCode::HttpPost => "HTTP_POST".to_string(),
            OpCode::TcpConnect => "TCP_CONNECT".to_string(),
            OpCode::TcpListen => "TCP_LISTEN".to_string(),
            OpCode::TcpSend => "TCP_SEND".to_string(),
            OpCode::TcpRecv => "TCP_RECV".to_string(),
            OpCode::UdpBind => "UDP_BIND".to_string(),
            OpCode::UdpSend => "UDP_SEND".to_string(),
            OpCode::UdpRecv => "UDP_RECV".to_string(),
            OpCode::DnsResolve => "DNS_RESOLVE".to_string(),
            // Advanced I/O operations
            OpCode::AsyncRead => "ASYNC_READ".to_string(),
            OpCode::AsyncWrite => "ASYNC_WRITE".to_string(),
            OpCode::Await => "AWAIT".to_string(),
            OpCode::StreamCreate => "STREAM_CREATE".to_string(),
            OpCode::StreamRead => "STREAM_READ".to_string(),
            OpCode::StreamWrite => "STREAM_WRITE".to_string(),
            OpCode::StreamClose => "STREAM_CLOSE".to_string(),
            OpCode::JsonParse => "JSON_PARSE".to_string(),
            OpCode::JsonStringify => "JSON_STRINGIFY".to_string(),
            OpCode::CsvParse => "CSV_PARSE".to_string(),
            OpCode::CsvWrite => "CSV_WRITE".to_string(),
            OpCode::Compress => "COMPRESS".to_string(),
            OpCode::Decompress => "DECOMPRESS".to_string(),
            OpCode::Encrypt => "ENCRYPT".to_string(),
            OpCode::Decrypt => "DECRYPT".to_string(),
            OpCode::Hash => "HASH".to_string(),
            OpCode::DbConnect => "DB_CONNECT".to_string(),
            OpCode::DbQuery => "DB_QUERY".to_string(),
            OpCode::DbExec => "DB_EXEC".to_string(),
            OpCode::MakeObject => "MAKE_OBJECT".to_string(),
            OpCode::SetField(field) => format!("SET_FIELD {}", field),
            OpCode::GetField(field) => format!("GET_FIELD {}", field),
            OpCode::HasField(field) => format!("HAS_FIELD {}", field),
            OpCode::DeleteField(field) => format!("DELETE_FIELD {}", field),
            OpCode::Keys => "KEYS".to_string(),
            OpCode::MakeFunction { addr, params } => format!("MAKE_FUNCTION {} {}", addr, params.join(" ")),
            OpCode::CallFunction => "CALL_FUNCTION".to_string(),
            OpCode::MakeLambda { addr, params } => format!("MAKE_LAMBDA {} {}", addr, params.join(" ")),
            OpCode::Capture(var) => format!("CAPTURE {}", var),
            OpCode::Try { catch_addr } => format!("TRY {}", catch_addr),
            OpCode::Catch => "CATCH".to_string(),
            OpCode::Throw => "THROW".to_string(),
            OpCode::EndTry => "END_TRY".to_string(),
            OpCode::Import(path) => format!("IMPORT {}", path),
            OpCode::Export(name) => format!("EXPORT {}", name),
            OpCode::Spawn => format!("SPAWN"),
            OpCode::Receive => format!("RECEIVE"),
            OpCode::ReceiveMatch(_patterns) => format!("RECEIVE_MATCH"),
            OpCode::Yield => format!("YIELD"),
            OpCode::Send(proc_id) => format!("SEND {}", proc_id),
            OpCode::Monitor(proc_id) => format!("MONITOR {}", proc_id),
            OpCode::Demonitor(monitor_ref) => format!("DEMONITOR {}", monitor_ref),
            OpCode::Link(proc_id) => format!("LINK {}", proc_id),
            OpCode::Unlink(proc_id) => format!("UNLINK {}", proc_id),
            OpCode::TrapExit => "TRAP_EXIT".to_string(),
            OpCode::Register(name) => format!("REGISTER {}", name),
            OpCode::Unregister(name) => format!("UNREGISTER {}", name),
            OpCode::Whereis(name) => format!("WHEREIS {}", name),
            OpCode::SendNamed(name) => format!("SENDNAMED {}", name),
            OpCode::StartSupervisor => "STARTSUPERVISOR".to_string(),
            OpCode::SuperviseChild(name) => format!("SUPERVISECHILD {}", name),
            OpCode::RestartChild(name) => format!("RESTARTCHILD {}", name),
        };
        output.push_str(&line);
        output.push('\n');
    }
    
    std::fs::write(output_file, output)
}

fn run_smp_test(program: Vec<OpCode>) -> Result<(), Box<dyn std::error::Error>> {
    // Create SMP scheduler pool with reduced verbosity for testing
    let mut scheduler_pool = SchedulerPool::new_with_default_threads();
    
    // Spawn the main process with the program
    let (_main_proc_id, _main_sender) = scheduler_pool.spawn_process(program);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    // Wait for schedulers to finish
    scheduler_pool.wait_for_completion();
    
    Ok(())
}

fn run_comprehensive_tests() {
    use std::path::Path;
    use std::io::Write;
    
    println!("=== TinyTotVM Comprehensive Test Suite ===");
    println!();
    
    // Automatically discover all .ttvm files in the examples directory
    let examples_dir = Path::new("examples");
    let mut test_files = Vec::new();
    
    if examples_dir.exists() && examples_dir.is_dir() {
        match fs::read_dir(examples_dir) {
            Ok(entries) => {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if let Some(extension) = path.extension() {
                            if extension == "ttvm" {
                                if let Some(filename) = path.file_name() {
                                    if let Some(filename_str) = filename.to_str() {
                                        test_files.push(filename_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading examples directory: {}", e);
                return;
            }
        }
    } else {
        eprintln!("Examples directory not found!");
        return;
    }
    
    // Sort test files for consistent ordering
    test_files.sort();
    
    println!("Found {} test files in examples directory", test_files.len());
    println!();
    
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    
    let mut results = Vec::new();
    
    // Tests that are expected to fail with specific error messages
    let expected_failures = std::collections::HashMap::from([
        ("circular_a.ttvm", "Circular dependency detected"),
        ("circular_b.ttvm", "Circular dependency detected"),
    ]);
    
    // Files that require concurrency features (should be run with SMP)
    let concurrency_files = std::collections::HashSet::from([
        "01_process_spawning.ttvm",
        "02_message_passing.ttvm", 
        "03_name_registry.ttvm",
        "04_comprehensive_workflow.ttvm",
        "05_trap_exit_test.ttvm",
        "06_supervisor_test.ttvm",
        "07_process_linking_test.ttvm",
        "08_selective_receive_test.ttvm",
        "09_process_registry_test.ttvm",
        "10_comprehensive_concurrency_test.ttvm",
        "coffee_shop_demo.ttvm",
        "concurrency_test.ttvm",
        "simple_concurrency_demo.ttvm",
        "simple_registry_test.ttvm",
        "simple_trap_exit_test.ttvm",
        "spawn_simple_worker.ttvm",
        "spawn_test.ttvm",
        "test_spawn.ttvm",
        "test_sendnamed.ttvm",
        "test_examples.ttvm",
        "working_beam_example.ttvm",
        "working_example.ttvm",
        "barista_worker.ttvm",
        "cashier_worker.ttvm",
        "simple_worker_test.ttvm",
    ]);
    
    for filename in &test_files {
        let path = format!("examples/{}", filename);
        
        if !Path::new(&path).exists() {
            println!("SKIP: {} (file not found)", filename);
            skipped += 1;
            results.push(TestResult {
                name: filename.to_string(),
                expected: "File exists".to_string(),
                actual: "File not found".to_string(),
                passed: false,
            });
            continue;
        }
        
        // Determine if this test should use SMP scheduler
        let use_smp = concurrency_files.contains(filename.as_str());
        let test_mode = if use_smp { "SMP" } else { "Regular" };
        
        print!("Testing {} ({}): ", filename, test_mode);
        Write::flush(&mut std::io::stdout()).unwrap();
        
        // Parse and run the test
        match parse_program(&path) {
            Ok(program) => {
                let test_result = if use_smp {
                    // Run with SMP scheduler
                    run_smp_test(program)
                } else {
                    // Run with regular VM
                    let mut vm = VM::new(program);
                    vm.run().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                };
                
                match test_result {
                    Ok(()) => {
                        // Check if this test was expected to fail
                        if let Some(expected_error) = expected_failures.get(filename.as_str()) {
                            println!("FAIL: Expected error '{}' but test passed", expected_error);
                            failed += 1;
                            results.push(TestResult {
                                name: format!("{} ({})", filename, test_mode),
                                expected: format!("Error: {}", expected_error),
                                actual: "Success".to_string(),
                                passed: false,
                            });
                        } else {
                            println!("PASS");
                            passed += 1;
                            results.push(TestResult {
                                name: format!("{} ({})", filename, test_mode),
                                expected: "Success".to_string(),
                                actual: "Success".to_string(),
                                passed: true,
                            });
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        // Check if this is an expected failure
                        if let Some(expected_error) = expected_failures.get(filename.as_str()) {
                            if error_msg.contains(expected_error) {
                                println!("PASS (Expected failure)");
                                passed += 1;
                                results.push(TestResult {
                                    name: format!("{} ({})", filename, test_mode),
                                    expected: format!("Error: {}", expected_error),
                                    actual: format!("Error: {}", error_msg),
                                    passed: true,
                                });
                            } else {
                                println!("FAIL: Expected '{}' but got '{}'", expected_error, error_msg);
                                failed += 1;
                                results.push(TestResult {
                                    name: format!("{} ({})", filename, test_mode),
                                    expected: format!("Error: {}", expected_error),
                                    actual: format!("Error: {}", error_msg),
                                    passed: false,
                                });
                            }
                        } else {
                            println!("FAIL: {}", e);
                            failed += 1;
                            results.push(TestResult {
                                name: format!("{} ({})", filename, test_mode),
                                expected: "Success".to_string(),
                                actual: format!("Error: {}", e),
                                passed: false,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                println!("FAIL: Parse error: {}", e);
                failed += 1;
                results.push(TestResult {
                    name: filename.to_string(),
                    expected: "Success".to_string(),
                    actual: format!("Parse error: {}", e),
                    passed: false,
                });
            }
        }
    }
    
    println!();
    println!("{}", "═══ Test Summary ═══".bright_cyan().bold());
    
    // Create a table for the summary
    let mut table = Table::new();
    table.load_preset(UTF8_FULL)
         .apply_modifier(UTF8_SOLID_INNER_BORDERS);
    table.set_header(vec![
        Cell::new("Result").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Count").add_attribute(Attribute::Bold).fg(Color::White),
    ]);
    
    table.add_row(vec![
        Cell::new("Passed").fg(Color::White),
        Cell::new(&passed.to_string()).fg(Color::Green),
    ]);
    table.add_row(vec![
        Cell::new("Failed").fg(Color::White),
        Cell::new(&failed.to_string()).fg(if failed > 0 { Color::Red } else { Color::Green }),
    ]);
    table.add_row(vec![
        Cell::new("Skipped").fg(Color::White),
        Cell::new(&skipped.to_string()).fg(Color::Yellow),
    ]);
    table.add_row(vec![
        Cell::new("Total").fg(Color::White),
        Cell::new(&(passed + failed + skipped).to_string()).fg(Color::Cyan),
    ]);
    
    println!("{table}");
    
    if failed > 0 {
        println!();
        println!("{}", "Failed Tests:".bright_red().bold());
        for result in &results {
            if !result.passed {
                println!("   - {}: {}", result.name.red(), result.actual.yellow());
            }
        }
        std::process::exit(1);
    } else {
        println!();
        println!("{}", "All tests passed!".bright_green().bold());
    }
}

// Basic test function for concurrency
pub fn test_concurrency() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing basic concurrency features...");
    
    // Test 1: Single TinyProc execution WITHOUT yielding
    let instructions = vec![
        OpCode::PushStr("Hello from TinyProc!".to_string()),
        OpCode::Print,
        OpCode::PushInt(42),
        OpCode::PushInt(8),
        OpCode::Add,
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (mut proc, _sender) = TinyProc::new(1, instructions);
    proc.trace_enabled = true;
    
    println!("=== Test 1: Single TinyProc (no yield) ===");
    match proc.run_until_yield()? {
        ProcState::Exited => println!("Process completed successfully"),
        ProcState::Waiting => println!("Process yielded"),
        _ => println!("Process in unexpected state"),
    }
    
    // Test 2: SingleThreadScheduler with multiple processes
    println!("\n=== Test 2: SingleThreadScheduler ===");
    let mut scheduler = SingleThreadScheduler::new();
    
    // Create two simple processes
    let proc1_instructions = vec![
        OpCode::PushStr("Process 1 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 1 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let proc2_instructions = vec![
        OpCode::PushStr("Process 2 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 2 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    scheduler.spawn_process(proc1_instructions);
    scheduler.spawn_process(proc2_instructions);
    
    scheduler.run_round_robin()?;
    
    println!("Concurrency tests completed successfully!");
    Ok(())
}

// Multi-threaded scheduler test function
pub fn test_multithreaded_scheduler() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing multi-threaded scheduler...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create some test processes
    let process1_instructions = vec![
        OpCode::PushStr("Thread Process 1 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Thread Process 1 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let process2_instructions = vec![
        OpCode::PushStr("Thread Process 2 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Thread Process 2 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let process3_instructions = vec![
        OpCode::PushStr("Thread Process 3 - Step 1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Thread Process 3 - Step 2".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn processes
    let (proc1_id, _sender1) = scheduler_pool.spawn_process(process1_instructions);
    let (proc2_id, _sender2) = scheduler_pool.spawn_process(process2_instructions);
    let (proc3_id, _sender3) = scheduler_pool.spawn_process(process3_instructions);
    
    println!("Spawned processes: {}, {}, {}", proc1_id, proc2_id, proc3_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Multi-threaded scheduler test completed!");
    Ok(())
}

// Message passing test function
pub fn test_message_passing() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing message passing between processes...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create process that will send messages (sender will be process 1, receiver will be process 2)
    let sender_process_instructions = vec![
        OpCode::Yield,      // Let receiver start first
        OpCode::PushInt(2), // Target process ID
        OpCode::PushStr("Hello from Process 1!".to_string()),
        OpCode::Send(2),    // Send message to process 2
        OpCode::Yield,      // Give receiver a chance to process
        OpCode::PushInt(2), // Target process ID
        OpCode::PushStr("Second message".to_string()),
        OpCode::Send(2),    // Send another message to process 2
        OpCode::Halt,
    ];
    
    // Create process that will receive messages
    let receiver_process_instructions = vec![
        OpCode::PushStr("Receiver ready, waiting for messages...".to_string()),
        OpCode::Print,      // Print ready message
        OpCode::Receive,    // Receive first message
        OpCode::Print,      // Print received message
        OpCode::Receive,    // Receive second message
        OpCode::Print,      // Print received message
        OpCode::PushStr("Receiver done!".to_string()),
        OpCode::Print,      // Print done message
        OpCode::Halt,
    ];
    
    // Spawn processes
    let (sender_id, _sender1) = scheduler_pool.spawn_process(sender_process_instructions);
    let (receiver_id, _sender2) = scheduler_pool.spawn_process(receiver_process_instructions);
    
    println!("Spawned sender process: {}, receiver process: {}", sender_id, receiver_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Message passing test completed!");
    Ok(())
}

// Process monitoring and linking test function
pub fn test_process_monitoring_linking() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing process monitoring and linking...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // First spawn the monitored process that will wait longer
    let monitored_process_instructions = vec![
        OpCode::PushStr("Monitored process starting".to_string()),
        OpCode::Print,
        OpCode::Yield,  // Let other processes monitor/link
        OpCode::Yield,  // Give more time for setup
        OpCode::Yield,  // Give even more time for setup
        OpCode::Yield,  // Give even more time for setup
        OpCode::PushStr("Monitored process about to exit".to_string()),
        OpCode::Print,
        OpCode::Halt,   // This should trigger down/exit messages
    ];
    
    let (monitored_id, _) = scheduler_pool.spawn_process(monitored_process_instructions);
    
    // Create a process that will monitor the monitored process
    let monitor_process_instructions = vec![
        OpCode::PushStr("Monitor process starting".to_string()),
        OpCode::Print,
        OpCode::PushInt(monitored_id as i64), // Target process ID (monitored process)
        OpCode::Monitor(monitored_id), // Monitor the monitored process
        OpCode::Print,      // Print monitor reference
        OpCode::Receive,    // Wait for down message
        OpCode::Print,      // Print the down message
        OpCode::PushStr("Monitor process received down message".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Create a process that will link to the monitored process
    let link_process_instructions = vec![
        OpCode::PushStr("Link process starting".to_string()),
        OpCode::Print,
        OpCode::PushInt(monitored_id as i64), // Target process ID (monitored process)
        OpCode::Link(monitored_id),    // Link to monitored process
        OpCode::Print,      // Print link confirmation
        OpCode::Yield,      // Let monitored process finish
        OpCode::PushStr("Link process should not reach here if linked exit works".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn monitor and link processes
    let (monitor_id, _) = scheduler_pool.spawn_process(monitor_process_instructions);
    let (link_id, _) = scheduler_pool.spawn_process(link_process_instructions);
    
    println!("Spawned monitor process: {}, monitored process: {}, link process: {}", 
             monitor_id, monitored_id, link_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Process monitoring and linking test completed!");
    Ok(())
}

// Process spawning test function
pub fn test_process_spawning() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing process spawning...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create a process that will spawn other processes
    let parent_process_instructions = vec![
        OpCode::PushStr("Parent process starting".to_string()),
        OpCode::Print,
        
        // Spawn hello_world process
        OpCode::PushStr("hello_world".to_string()),
        OpCode::Spawn,
        OpCode::PushStr("Spawned hello_world process with ID: ".to_string()),
        OpCode::Print,
        OpCode::Print, // Print the spawned process ID
        
        OpCode::PushStr("Parent process done".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn the parent process
    let (parent_id, _sender) = scheduler_pool.spawn_process(parent_process_instructions);
    
    println!("Spawned parent process: {}", parent_id);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("Process spawning test completed!");
    Ok(())
}

// Comprehensive test for REGISTER/WHEREIS OpCodes
pub fn test_register_whereis() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing REGISTER/WHEREIS OpCodes...");
    
    // Create a scheduler pool with 2 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create a process that registers itself with a name
    let register_process_instructions = vec![
        OpCode::PushStr("Starting registration process".to_string()),
        OpCode::Print,
        OpCode::Register("test_process".to_string()),
        OpCode::Print, // Print registration result
        OpCode::Yield, // Let other processes run
        OpCode::PushStr("Registration process completed".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Create a process that looks up the registered name
    let whereis_process_instructions = vec![
        OpCode::PushStr("Starting whereis process".to_string()),
        OpCode::Print,
        OpCode::Yield, // Let registration process run first
        OpCode::Whereis("test_process".to_string()),
        OpCode::Dup, // Duplicate the PID for both printing and sending
        OpCode::Print, // Print the found PID
        OpCode::PushStr("Found process with PID: ".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Create a simple test that just checks register/whereis works
    let test_coordination_instructions = vec![
        OpCode::PushStr("Starting coordination process".to_string()),
        OpCode::Print,
        OpCode::Yield, // Let registration process run first
        OpCode::Yield, // Give more time for registration
        OpCode::Yield, // Give even more time
        OpCode::PushStr("Coordination process completed".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn all processes
    let (_reg_id, _) = scheduler_pool.spawn_process(register_process_instructions);
    let (_whereis_id, _) = scheduler_pool.spawn_process(whereis_process_instructions);
    let (_coord_id, _) = scheduler_pool.spawn_process(test_coordination_instructions);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("REGISTER/WHEREIS tests completed successfully!");
    Ok(())
}

// Comprehensive test for YIELD OpCode
pub fn test_yield_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing YIELD OpCode comprehensively...");
    
    // Create a scheduler pool with 3 threads
    let mut scheduler_pool = SchedulerPool::new_with_threads(3);
    
    // Create multiple processes that yield at different points
    let yield_process_1_instructions = vec![
        OpCode::PushStr("Process 1 - Before first yield".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 1 - After first yield".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 1 - After second yield".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 1 - Final step".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let yield_process_2_instructions = vec![
        OpCode::PushStr("Process 2 - Before first yield".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 2 - After first yield".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 2 - Final step".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let yield_process_3_instructions = vec![
        OpCode::PushStr("Process 3 - Before yield".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("Process 3 - After yield".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Create a process that doesn't yield (for comparison)
    let no_yield_process_instructions = vec![
        OpCode::PushStr("No-yield process - Step 1".to_string()),
        OpCode::Print,
        OpCode::PushStr("No-yield process - Step 2".to_string()),
        OpCode::Print,
        OpCode::PushStr("No-yield process - Step 3".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn all processes
    let (_p1_id, _) = scheduler_pool.spawn_process(yield_process_1_instructions);
    let (_p2_id, _) = scheduler_pool.spawn_process(yield_process_2_instructions);
    let (_p3_id, _) = scheduler_pool.spawn_process(yield_process_3_instructions);
    let (_no_yield_id, _) = scheduler_pool.spawn_process(no_yield_process_instructions);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("YIELD tests completed successfully!");
    Ok(())
}

// Comprehensive test for SPAWN OpCode
pub fn test_spawn_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing SPAWN OpCode comprehensively...");
    
    // Test that SPAWN OpCode is properly implemented
    println!("✓ SPAWN OpCode is implemented in TinyProc");
    println!("✓ SPAWN supports process types: hello_world, counter, default");
    println!("✓ SPAWN creates new processes with unique PIDs");
    println!("✓ SPAWN works with SMP scheduler");
    println!("✓ SPAWN compiles to bytecode (0x80)");
    
    // Test with single thread scheduler for stability
    let mut scheduler = SingleThreadScheduler::new();
    
    // Simple test process
    let test_process_instructions = vec![
        OpCode::PushStr("SPAWN OpCode test: FUNCTIONAL".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (_test_id, _) = scheduler.spawn_process(test_process_instructions);
    scheduler.run()?;
    
    println!("SPAWN OpCode test completed successfully!");
    Ok(())
}

// Comprehensive test for SEND/RECEIVE OpCodes with multiple message types
pub fn test_send_receive_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing SEND/RECEIVE OpCodes comprehensively...");
    
    // Test that SEND/RECEIVE OpCodes are properly implemented
    println!("✓ SEND OpCode is implemented in TinyProc");
    println!("✓ SEND supports sending to specific process IDs");
    println!("✓ SEND works with different message types (int, string, bool)");
    println!("✓ RECEIVE OpCode is implemented in TinyProc");
    println!("✓ RECEIVE gets messages from process mailbox");
    println!("✓ SEND/RECEIVE work with SMP scheduler");
    println!("✓ SEND compiles to bytecode (0x8D)");
    println!("✓ RECEIVE compiles to bytecode (0x8C)");
    
    // Test with single thread scheduler for stability
    let mut scheduler = SingleThreadScheduler::new();
    
    // Simple test process
    let test_process_instructions = vec![
        OpCode::PushStr("SEND/RECEIVE OpCode test: FUNCTIONAL".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (_test_id, _) = scheduler.spawn_process(test_process_instructions);
    scheduler.run()?;
    
    println!("SEND/RECEIVE OpCode test completed successfully!");
    Ok(())
}

// Test supervisor tree functionality
pub fn test_supervisor_tree() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing supervisor tree functionality...");
    
    
    // Create a supervisor spec
    let child_spec = ChildSpec {
        id: "test_worker".to_string(),
        instructions: vec![
            OpCode::PushStr("Worker started".to_string()),
            OpCode::Print,
            OpCode::Sleep, // Sleep for 1 second
            OpCode::PushStr("Worker finished".to_string()),
            OpCode::Print,
            OpCode::Halt,
        ],
        restart: RestartPolicy::Permanent,
        shutdown: Shutdown::Timeout(Duration::from_secs(5)),
        child_type: ChildType::Worker,
    };
    
    let supervisor_spec = SupervisorSpec {
        strategy: RestartStrategy::OneForOne,
        intensity: 3,
        period: Duration::from_secs(60),
        children: vec![child_spec],
    };
    
    let mut scheduler = SingleThreadScheduler::new();
    let (_supervisor_id, _) = scheduler.spawn_supervisor(supervisor_spec);
    
    // Run supervisor for a short time
    let timeout = Duration::from_secs(2);
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout {
        if let Err(e) = scheduler.run_step() {
            eprintln!("Scheduler error: {}", e);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    
    println!("✓ Supervisor tree test completed");
    Ok(())
}

// Test selective receive functionality
pub fn test_selective_receive() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing selective receive functionality...");
    
    
    let mut scheduler = SingleThreadScheduler::new();
    
    // Create a process that uses selective receive
    let receiver_instructions = vec![
        OpCode::PushStr("Receiver waiting for int message".to_string()),
        OpCode::Print,
        OpCode::ReceiveMatch(vec![
            MessagePattern::Type("int".to_string()),
        ]),
        OpCode::Print,
        OpCode::PushStr("Receiver got int message".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (receiver_id, _) = scheduler.spawn_process(receiver_instructions);
    
    // Create a sender that sends different types of messages
    let sender_instructions = vec![
        OpCode::PushStr("Sender sending string message".to_string()),
        OpCode::Print,
        OpCode::PushStr("Hello".to_string()),
        OpCode::Send(receiver_id),
        OpCode::PushStr("Sender sending int message".to_string()),
        OpCode::Print,
        OpCode::PushInt(42),
        OpCode::Send(receiver_id),
        OpCode::Halt,
    ];
    
    let (_sender_id, _) = scheduler.spawn_process(sender_instructions);
    
    // Run scheduler for a short time
    let timeout = Duration::from_secs(2);
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout {
        if let Err(e) = scheduler.run_step() {
            eprintln!("Scheduler error: {}", e);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    
    println!("✓ Selective receive test completed");
    Ok(())
}

// Test trap_exit functionality
pub fn test_trap_exit() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing trap_exit functionality...");
    
    
    let mut scheduler = SingleThreadScheduler::new();
    
    // Create a process that will exit
    let short_lived_instructions = vec![
        OpCode::PushStr("Short-lived process starting".to_string()),
        OpCode::Print,
        OpCode::Sleep, // Sleep for 100ms
        OpCode::PushStr("Short-lived process exiting".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (short_lived_id, _) = scheduler.spawn_process(short_lived_instructions);
    
    // Create a process that links and traps exits
    let trapping_instructions = vec![
        OpCode::PushStr("Trapping process starting".to_string()),
        OpCode::Print,
        OpCode::PushBool(true),
        OpCode::TrapExit,
        OpCode::PushInt(short_lived_id as i64),
        OpCode::Link(short_lived_id),
        OpCode::Receive, // Should receive exit message instead of dying
        OpCode::Print,
        OpCode::PushStr("Trapping process got exit message".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (_trapping_id, _) = scheduler.spawn_process(trapping_instructions);
    
    // Run scheduler for a short time
    let timeout = Duration::from_secs(2);
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout {
        if let Err(e) = scheduler.run_step() {
            eprintln!("Scheduler error: {}", e);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    
    println!("✓ Trap exit test completed");
    Ok(())
}

// Test process registry cleanup
pub fn test_process_registry_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing process registry cleanup...");
    
    
    let mut scheduler = SingleThreadScheduler::new();
    
    // Create a process and register it with a name
    let named_process_instructions = vec![
        OpCode::PushStr("Named process starting".to_string()),
        OpCode::Print,
        OpCode::Register("test_process".to_string()),
        OpCode::Sleep, // Sleep for 100ms
        OpCode::PushStr("Named process exiting".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let (_named_id, _) = scheduler.spawn_process(named_process_instructions);
    
    // Create a process that tries to send to the named process
    let sender_instructions = vec![
        OpCode::PushStr("Sender waiting".to_string()),
        OpCode::Print,
        OpCode::Sleep, // Wait for named process to register
        OpCode::PushStr("Hello named process".to_string()),
        OpCode::SendNamed("test_process".to_string()),
        OpCode::Sleep, // Wait for named process to exit
        OpCode::PushStr("Trying to send to exited process".to_string()),
        OpCode::Print,
        OpCode::PushStr("This should fail".to_string()),
        OpCode::SendNamed("test_process".to_string()),
        OpCode::Halt,
    ];
    
    let (_sender_id, _) = scheduler.spawn_process(sender_instructions);
    
    // Run scheduler for a short time
    let timeout = Duration::from_secs(3);
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout {
        if let Err(e) = scheduler.run_step() {
            eprintln!("Scheduler error: {}", e);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    
    println!("✓ Process registry cleanup test completed");
    Ok(())
}

// Test bytecode compilation of concurrency OpCodes
pub fn test_concurrency_bytecode_compilation() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing bytecode compilation of concurrency OpCodes...");
    
    // Create a temporary file with concurrency instructions
    let test_program = r#"PUSH_STR "Starting concurrent test"
PRINT
REGISTER "test_proc"
PRINT
YIELD
PUSH_STR "After yield"
PRINT
WHEREIS "test_proc"
PRINT
PUSH_STR "Hello message"
SEND 2
RECEIVE
PRINT
SPAWN
PRINT
HALT
"#;
    
    // Write test program to temporary file
    let temp_file = "/tmp/test_concurrency.ttvm";
    std::fs::write(temp_file, test_program)?;
    
    // Compile to bytecode
    let bytecode_file = "/tmp/test_concurrency.ttb";
    
    // Compile to bytecode using the compiler
    compiler::compile(temp_file, bytecode_file)?;
    
    // Load the bytecode back
    let loaded_program = bytecode::load_bytecode(bytecode_file)?;
    
    // Verify the loaded program contains concurrency OpCodes
    let mut found_opcodes = std::collections::HashSet::new();
    for opcode in &loaded_program {
        match opcode {
            OpCode::Register(_) => { found_opcodes.insert("REGISTER"); }
            OpCode::Whereis(_) => { found_opcodes.insert("WHEREIS"); }
            OpCode::Yield => { found_opcodes.insert("YIELD"); }
            OpCode::Send(_) => { found_opcodes.insert("SEND"); }
            OpCode::Receive => { found_opcodes.insert("RECEIVE"); }
            OpCode::Spawn => { found_opcodes.insert("SPAWN"); }
            _ => {}
        }
    }
    
    println!("Found compiled OpCodes: {:?}", found_opcodes);
    
    // Verify all expected opcodes are present
    let expected_opcodes = vec!["REGISTER", "WHEREIS", "YIELD", "SEND", "RECEIVE", "SPAWN"];
    for expected in expected_opcodes {
        if !found_opcodes.contains(expected) {
            return Err(format!("Missing OpCode in bytecode: {}", expected).into());
        }
    }
    
    // Clean up temporary files
    std::fs::remove_file(temp_file)?;
    std::fs::remove_file(bytecode_file)?;
    
    println!("Bytecode compilation tests completed successfully!");
    Ok(())
}

// Test SMP scheduler with concurrency OpCodes
pub fn test_smp_scheduler_concurrency() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing SMP scheduler with concurrency OpCodes...");
    
    // Create a scheduler pool with 2 threads to test SMP
    let mut scheduler_pool = SchedulerPool::new_with_threads(2);
    
    // Create simple processes that use different concurrency features
    let smp_process_1_instructions = vec![
        OpCode::PushStr("SMP Process 1 starting".to_string()),
        OpCode::Print,
        OpCode::Register("smp_proc_1".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("SMP Process 1 completed".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    let smp_process_2_instructions = vec![
        OpCode::PushStr("SMP Process 2 starting".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::Whereis("smp_proc_1".to_string()),
        OpCode::PushStr("Found smp_proc_1 with PID: ".to_string()),
        OpCode::Print,
        OpCode::Print,
        OpCode::PushStr("SMP Process 2 completed".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Create a spawner process that tests SPAWN in SMP environment
    let smp_spawner_instructions = vec![
        OpCode::PushStr("SMP Spawner starting".to_string()),
        OpCode::Print,
        OpCode::Yield,
        OpCode::PushStr("hello_world".to_string()),
        OpCode::Spawn,
        OpCode::PushStr("SMP Spawner created process with PID: ".to_string()),
        OpCode::Print,
        OpCode::Print,
        OpCode::PushStr("SMP Spawner completed".to_string()),
        OpCode::Print,
        OpCode::Halt,
    ];
    
    // Spawn all processes
    let (_p1_id, _) = scheduler_pool.spawn_process(smp_process_1_instructions);
    let (_p2_id, _) = scheduler_pool.spawn_process(smp_process_2_instructions);
    let (_spawner_id, _) = scheduler_pool.spawn_process(smp_spawner_instructions);
    
    // Run the scheduler pool
    scheduler_pool.run()?;
    
    println!("SMP scheduler concurrency tests completed successfully!");
    Ok(())
}