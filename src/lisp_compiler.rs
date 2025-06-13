// lisp_compiler.rs
use std::fs;
use std::io::Write;

#[derive(Debug, Clone)]
enum Expr {
    Int(i64),
    Str(String),
    Bool(bool),
    Symbol(String),
    List(Vec<Expr>),
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for line in input.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let mut in_string = false;
        let mut current = String::new();

        for c in line.chars() {
            match c {
                '"' => {
                    current.push(c);
                    in_string = !in_string;
                }
                '(' | ')' if !in_string => {
                    if !current.trim().is_empty() {
                        tokens.push(current.trim().to_string());
                        current.clear();
                    }
                    tokens.push(c.to_string());
                }
                ' ' if !in_string => {
                    if !current.trim().is_empty() {
                        tokens.push(current.trim().to_string());
                        current.clear();
                    }
                }
                _ => current.push(c),
            }
        }

        if !current.trim().is_empty() {
            tokens.push(current.trim().to_string());
        }
    }

    tokens
}

fn parse(tokens: &mut Vec<String>) -> Expr {
    if tokens.is_empty() {
        panic!("Unexpected EOF while reading");
    }
    let token = tokens.remove(0);
    match token.as_str() {
        "(" => {
            let mut list = Vec::new();
            while tokens[0] != ")" {
                list.push(parse(tokens));
            }
            tokens.remove(0); // remove ')'
            Expr::List(list)
        }
        ")" => panic!("Unexpected ')'"),
        _ => atom(&token),
    }
}

fn atom(token: &str) -> Expr {
    if let Ok(n) = token.parse::<i64>() {
        Expr::Int(n)
    } else if token == "#t" {
        Expr::Bool(true)
    } else if token == "#f" {
        Expr::Bool(false)
    } else if token.starts_with('"') && token.ends_with('"') {
        Expr::Str(token.trim_matches('"').to_string())
    } else {
        Expr::Symbol(token.to_string())
    }
}

fn compile_expr(expr: &Expr, output: &mut dyn Write) {
    match expr {
        Expr::Int(n) => writeln!(output, "PUSH_INT {}", n).unwrap(),
        Expr::Str(s) => writeln!(output, "PUSH_STR \"{}\"", s).unwrap(),
        Expr::Bool(true) => writeln!(output, "TRUE").unwrap(),
        Expr::Bool(false) => writeln!(output, "FALSE").unwrap(),
        Expr::Symbol(s) => writeln!(output, "LOAD {}", s).unwrap(),
        Expr::List(list) => {
            if list.is_empty() {
                return;
            }
            match &list[0] {
                Expr::Symbol(s) => match s.as_str() {
                    "+" => binary_op(&list[1..], "ADD", output),
                    "-" => binary_op(&list[1..], "SUB", output),
                    "=" => binary_op(&list[1..], "EQ", output),
                    ">" => binary_op(&list[1..], "GT", output),
                    "<" => binary_op(&list[1..], "LT", output),
                    ">=" => binary_op(&list[1..], "GE", output),
                    "<=" => binary_op(&list[1..], "LE", output),
                    "print" => {
                        compile_expr(&list[1], output);
                        writeln!(output, "PRINT").unwrap();
                    }
                    "define" => match &list[1] {
                        Expr::Symbol(name) => {
                            compile_expr(&list[2], output);
                            writeln!(output, "STORE {}", name).unwrap();
                        }
                        _ => panic!("Invalid define syntax"),
                    },
                    "set!" => match &list[1] {
                        Expr::Symbol(name) => {
                            compile_expr(&list[2], output);
                            writeln!(output, "STORE {}", name).unwrap();
                        }
                        _ => panic!("Invalid set! syntax"),
                    },
                    "if" => {
                        let else_label = fresh_label("else");
                        let end_label = fresh_label("end_if");
                        compile_expr(&list[1], output);
                        writeln!(output, "JZ {}", else_label).unwrap();
                        compile_expr(&list[2], output);
                        writeln!(output, "JMP {}", end_label).unwrap();
                        writeln!(output, "LABEL {}", else_label).unwrap();
                        compile_expr(&list[3], output);
                        writeln!(output, "LABEL {}", end_label).unwrap();
                    }
                    _ => panic!("Unknown operation: {}", s),
                },
                _ => panic!("First element in list must be a symbol"),
            }
        }
    }
}

fn binary_op(args: &[Expr], op: &str, output: &mut dyn Write) {
    compile_expr(&args[0], output);
    compile_expr(&args[1], output);
    writeln!(output, "{}", op).unwrap();
}

fn fresh_label(base: &str) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}_{}", base, id)
}

pub fn compile_lisp(input: &str, output_path: &str) {
    let raw = fs::read_to_string(input).expect("Failed to read Lisp file");
    let mut tokens = tokenize(&raw);
    let mut out = fs::File::create(output_path).expect("Failed to create output file");

    while !tokens.is_empty() {
        let expr = parse(&mut tokens);
        compile_expr(&expr, &mut out);
    }
    writeln!(out, "HALT").unwrap();
}
