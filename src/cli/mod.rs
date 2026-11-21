use crate::checker::TypeChecker;
use crate::config::{DogConfig, Personality};
use crate::doctor::run_doctor;
use crate::editor::launch_editor;
use crate::formatter::format_source;
use crate::interpreter::{check_source, run_source, Interpreter};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::stdlib::StdIo;
use crate::tester::run_tests;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = DogConfig::load();

    if args.is_empty() {
        print_help();
        return;
    }

    match args[0].as_str() {
        "--help" | "-h" | "help" => print_help(),
        "--version" | "-V" | "version" => {
            println!("dog {VERSION}");
        }
        "run" => {
            let Some(path) = args.get(1) else {
                eprintln!("Usage: dog run <file.dg>");
                process::exit(2);
            };
            let source = read_file_or_exit(path);
            if let Err(diag) = run_source(path, &source) {
                eprintln!("{}", diag.format_with(config.personality, Some(&source)));
                process::exit(1);
            }
        }
        "check" => {
            let Some(path) = args.get(1) else {
                eprintln!("Usage: dog check <file.dg>");
                process::exit(2);
            };
            let source = read_file_or_exit(path);
            match check_source(path, &source) {
                Ok(_) => println!("OK: {path} passed syntax and strict type checks."),
                Err(diag) => {
                    eprintln!("{}", diag.format_with(config.personality, Some(&source)));
                    process::exit(1);
                }
            }
        }
        "format" => {
            let check_only = args.iter().any(|a| a == "--check");
            let path = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with('-'))
                .map(String::as_str);
            let Some(path) = path else {
                eprintln!("Usage: dog format [--check] <file.dg>");
                process::exit(2);
            };
            let source = read_file_or_exit(path);
            let formatted = format_source(&source);
            if check_only {
                if source == formatted {
                    println!("OK: {path} is already formatted.");
                } else {
                    eprintln!("Needs formatting: {path}");
                    process::exit(1);
                }
            } else if let Err(err) = fs::write(path, &formatted) {
                eprintln!("Failed to write '{path}': {err}");
                process::exit(1);
            } else {
                println!("Formatted {path}");
            }
        }
        "test" => {
            let target = args.get(1).map(String::as_str);
            match run_tests(target, config.personality) {
                Ok(report) => {
                    if report.failed > 0 {
                        process::exit(1);
                    }
                }
                Err(err) => {
                    eprintln!("{err}");
                    process::exit(1);
                }
            }
        }
        "doctor" => run_doctor(),
        "edit" | "editor" => {
            let target = args.get(1).map(String::as_str);
            if let Err(err) = launch_editor(target, config.personality) {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        "config" => {
            if args.get(1).map(String::as_str) == Some("personality") {
                if let Some(mode_raw) = args.get(2) {
                    if let Some(mode) = Personality::parse(mode_raw) {
                        let next = DogConfig { personality: mode };
                        if let Err(err) = next.save() {
                            eprintln!("{err}");
                            process::exit(1);
                        }
                        println!("DOG personality set to '{}'.", mode.as_str());
                    } else {
                        eprintln!(
                            "Invalid personality '{mode_raw}'. Valid modes: normal, sarcastic, chaotic, professional, silent."
                        );
                        process::exit(2);
                    }
                } else {
                    println!("Current personality: {}", config.personality.as_str());
                }
            } else {
                eprintln!("Usage: dog config personality [normal|sarcastic|chaotic|professional|silent]");
                process::exit(2);
            }
        }
        "repl" => run_repl(config.personality),
        unknown => {
            eprintln!("Unknown command '{unknown}'. Run `dog --help` to see working commands.");
            process::exit(2);
        }
    }
}

fn read_file_or_exit(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Failed to read '{path}': {err}");
            process::exit(1);
        }
    }
}

fn detect_prompt_identity() -> (String, char) {
    let user = env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .unwrap_or_else(|_| {
            if env::var("PREFIX").is_ok_and(|p| p.contains("com.termux")) {
                "shell".to_string()
            } else {
                "nonroot".to_string()
            }
        });

    if detect_actual_root() {
        ("root".to_string(), '#')
    } else {
        (user, '$')
    }
}

fn detect_actual_root() -> bool {
    #[cfg(unix)]
    {
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("Uid:") {
                    if rest.split_whitespace().nth(1) == Some("0") {
                        return true;
                    }
                }
            }
        }
        false
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn format_kali_prompt() -> String {
    let (user, sigil) = detect_prompt_identity();
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());
    let display_dir = if let Ok(home) = env::var("HOME") {
        if let Some(stripped) = cwd.strip_prefix(&home) {
            if stripped.is_empty() {
                "~".to_string()
            } else {
                format!("~{stripped}")
            }
        } else {
            cwd
        }
    } else {
        cwd
    };

    let blue = "\x1b[1;34m";
    let red = "\x1b[1;31m";
    let green = "\x1b[1;32m";
    let reset = "\x1b[0m";

    format!(
        "{blue}┌──({red}{user}⩫dog{blue})-[{green}{display_dir}{blue}]\n└─{red}{sigil}{reset} "
    )
}

fn run_repl(personality: Personality) {
    println!("DOG {VERSION} interactive REPL (type 'exit' to quit)");
    let stdin = io::stdin();
    let mut checker = TypeChecker::new();
    let mut stdio = StdIo;
    let mut interp = Interpreter::new(&mut stdio);
    let mut buffer = String::new();
    let mut block_depth: usize = 0;

    loop {
        if block_depth == 0 {
            print!("{}", format_kali_prompt());
        } else {
            print!("\x1b[1;34m... \x1b[0m");
        }
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if block_depth == 0 && (trimmed == "exit" || trimmed == "quit") {
                    break;
                }
                if block_depth == 0 && trimmed.is_empty() {
                    continue;
                }

                let lead = trimmed
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if matches!(lead, "if" | "while" | "repeat" | "for" | "fn") {
                    block_depth += 1;
                } else if lead == "end" {
                    block_depth = block_depth.saturating_sub(1);
                }

                buffer.push_str(&line);
                if block_depth > 0 {
                    continue;
                }

                let snippet = std::mem::take(&mut buffer);
                let res = Lexer::new("<repl>", &snippet)
                    .tokenize()
                    .and_then(|tokens| Parser::new(tokens).parse())
                    .and_then(|program| {
                        checker.check(&program)?;
                        interp.eval_program(&program)
                    });

                if let Err(diag) = res {
                    eprintln!("{}", diag.format_with(personality, Some(&snippet)));
                }
            }
            Err(err) => {
                eprintln!("REPL input error: {err}");
                break;
            }
        }
    }
}

fn print_help() {
    println!(
        "DOG Programming Language CLI ({VERSION})

Usage:
  dog run <file.dg>                 Execute a .dg program
  dog check <file.dg>               Type-check a .dg program without running it
  dog format [--check] <file.dg>    Format a .dg file with 4-space indentation
  dog test [path]                   Run *_test.dg test files
  dog doctor                        Inspect environment, Termux, and DOG config
  dog edit [file.dg]                Open the terminal DOG EDITOR
  dog editor [file.dg]              Alias for `dog edit`
  dog repl                          Start stateful interactive DOG terminal
  dog config personality <mode>     Set personality (normal|sarcastic|chaotic|professional|silent)
  dog version | --version           Print DOG CLI version
  dog help | --help                 Show this help message"
    );
}
