use crate::bytecode::{build_bundle, build_native, is_dgb_path, run_bundle_with_io};
use crate::checker::TypeChecker;
use crate::config::{DogConfig, Personality};
use crate::diagnostics::{achievement_for_runs, welcome_banner};
use crate::doctor::run_doctor;
use crate::editor::launch_editor;
use crate::formatter::format_source;
use crate::hub::{handle_add, handle_info, handle_list, handle_remove, handle_search};
use crate::interpreter::{check_source, run_source_with_io, Interpreter};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::prompt::{handle_completions, handle_prompt, render_live_prompt};
use crate::sdu::handle_sdu;
use crate::stdlib::{StdIo, StdIoWithArgs};
use crate::studio::launch_studio;
use crate::sys::{
    handle_clean, handle_docs, handle_env, handle_forge, handle_inspect, handle_logs,
    handle_permissions, handle_process, handle_profile, handle_server, handle_status,
    handle_system, handle_update,
};
use crate::tester::run_tests;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut config = DogConfig::load();

    if args.is_empty() {
        print_help();
        return;
    }

    append_cli_log(&args[0]);

    match args[0].as_str() {
        "--help" | "-h" | "help" => print_help(),
        "--version" | "-V" | "version" => {
            println!("dog {VERSION}");
        }
        "run" => {
            let Some(path) = args.get(1) else {
                eprintln!("Usage: dog run <file.dg | file.dgb> [-- <script-args...>]");
                process::exit(2);
            };
            let script_args: Vec<String> = if let Some(pos) = args.iter().position(|a| a == "--") {
                args[pos + 1..].to_vec()
            } else if args.len() > 2 {
                args[2..].to_vec()
            } else {
                Vec::new()
            };
            config.record_run();
            if is_dgb_path(path) {
                let mut stdio = StdIoWithArgs { args: script_args };
                if let Err(err) = run_bundle_with_io(path, &mut stdio) {
                    eprintln!("{err}");
                    process::exit(1);
                }
            } else {
                let source = read_file_or_exit(path);
                let mut stdio = StdIoWithArgs { args: script_args };
                if let Err(diag) = run_source_with_io(path, &source, &mut stdio) {
                    eprintln!("{}", diag.format_with(config.personality, Some(&source)));
                    process::exit(1);
                }
            }
        }
        "build" => {
            let native = args.iter().any(|a| a == "--native");
            let mut input_path: Option<&str> = None;
            let mut out_path: Option<&str> = None;
            let mut i = 1usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--native" => {}
                    "--out" | "-o" => {
                        if let Some(v) = args.get(i + 1) {
                            out_path = Some(v.as_str());
                            i += 1;
                        } else {
                            eprintln!("Missing output path after --out");
                            process::exit(2);
                        }
                    }
                    a if !a.starts_with('-') && input_path.is_none() => {
                        input_path = Some(a);
                    }
                    other => {
                        eprintln!("Unexpected build argument '{other}'.");
                        process::exit(2);
                    }
                }
                i += 1;
            }
            let Some(input) = input_path else {
                eprintln!("Usage: dog build <file.dg> [--out <path>] [--native]");
                process::exit(2);
            };
            if native {
                match build_native(input, out_path) {
                    Ok(out) => println!("Built native binary: {}", out.display()),
                    Err(e) => {
                        eprintln!("{e}");
                        process::exit(1);
                    }
                }
            } else {
                match build_bundle(input, out_path) {
                    Ok((out, bundle)) => println!(
                        "Built DOG bundle: {} (sha256:{})",
                        out.display(),
                        &bundle.sha256[..16]
                    ),
                    Err(e) => {
                        eprintln!("{e}");
                        process::exit(1);
                    }
                }
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
        "format" | "fmt" => {
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
        "studio" => {
            let target = args.get(1).map(String::as_str);
            if let Err(err) = launch_studio(target, config.personality) {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        "config" => {
            if args.get(1).map(String::as_str) == Some("personality") {
                if let Some(mode_raw) = args.get(2) {
                    if let Some(mode) = Personality::parse(mode_raw) {
                        config.personality = mode;
                        if let Err(err) = config.save() {
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
                eprintln!(
                    "Usage: dog config personality [normal|sarcastic|chaotic|professional|silent]"
                );
                process::exit(2);
            }
        }
        "repl" => {
            config.record_run();
            run_repl(&config);
        }
        "inspect" => {
            let Some(path) = args.get(1) else {
                eprintln!("Usage: dog inspect <file.dg>");
                process::exit(2);
            };
            match handle_inspect(path) {
                Ok(out) => println!("{out}"),
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            }
        }
        "profile" => {
            let Some(path) = args.get(1) else {
                eprintln!("Usage: dog profile <file.dg>");
                process::exit(2);
            };
            match handle_profile(path) {
                Ok(out) => println!("{out}"),
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            }
        }
        "docs" => {
            println!("{}", handle_docs(args.get(1).map(String::as_str)));
        }
        "forge" => match handle_forge(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "new" => {
            let mut forge_args = vec!["new".to_string()];
            forge_args.extend(args[1..].iter().cloned());
            match handle_forge(&forge_args) {
                Ok(out) => println!("{out}"),
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            }
        }
        "init" => {
            let mut forge_args = vec!["new".to_string(), ".".to_string()];
            forge_args.extend(args[1..].iter().cloned());
            match handle_forge(&forge_args) {
                Ok(out) => println!("{out}"),
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            }
        }
        "prompt" => match handle_prompt(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "completions" => match handle_completions(args.get(1).map(String::as_str)) {
            Ok(out) => print!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "server" => {
            if let Err(e) = handle_server(&args[1..]) {
                eprintln!("{e}");
                process::exit(1);
            }
        }
        "system" => println!("{}", handle_system()),
        "status" => println!("{}", handle_status()),
        "env" => print!("{}", handle_env()),
        "process" => println!("{}", handle_process()),
        "permissions" => println!("{}", handle_permissions()),
        "update" | "upgrade" => println!("{}", handle_update()),
        "clean" => println!("{}", handle_clean()),
        "logs" => match handle_logs(&args[1..]) {
            Ok(out) => print!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "add" => match handle_add(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "remove" => match handle_remove(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "search" => println!("{}", handle_search(&args[1..])),
        "list" => match handle_list() {
            Ok(out) => print!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "info" => match handle_info(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "sdu" => match handle_sdu(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        unknown => {
            eprintln!("Unknown command '{unknown}'. Run `dog --help` to see working commands.");
            process::exit(2);
        }
    }
}

fn append_cli_log(cmd: &str) {
    let log_file = DogConfig::home_dir().join("logs").join("dog.log");
    if let Some(parent) = log_file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_file) {
        let _ = writeln!(f, "[{ts}] cmd={cmd}");
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

fn run_repl(config: &DogConfig) {
    println!("DOG {VERSION} interactive REPL (type 'exit' to quit)");
    if let Some(banner) = welcome_banner(config.personality, config.runs_completed) {
        println!("{banner}");
    }
    if let Some(ach) = achievement_for_runs(config.runs_completed) {
        println!("{ach}");
    }

    let stdin = io::stdin();
    let mut checker = TypeChecker::new();
    let mut stdio = StdIo;
    let mut interp = Interpreter::new(&mut stdio);
    let mut buffer = String::new();
    let mut block_depth: usize = 0;

    loop {
        if block_depth == 0 {
            print!("{}", render_live_prompt());
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
                if matches!(
                    lead,
                    "if" | "match" | "try" | "while" | "repeat" | "for" | "fn" | "struct" | "enum"
                ) {
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
                    eprintln!("{}", diag.format_with(config.personality, Some(&snippet)));
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

Core Language & Tooling:
  dog run <file.dg | file.dgb>      Execute a .dg program or compiled .dgb bundle
  dog build <file.dg> [--native]    Compile to .dgb bundle (or native binary via rustc)
  dog check <file.dg>               Type-check a .dg program without running it
  dog format [--check] <file.dg>    Format a .dg file with 4-space indentation (alias: fmt)
  dog test [path]                   Run *_test.dg test files
  dog inspect <file.dg>             Print tokens, AST outline, and checked symbols
  dog profile <file.dg>             Measure lex, parse, check, and runtime timings

Interactive, Prompt & Workspace IDEs:
  dog repl                          Stateful interactive DOG terminal
  dog edit [file.dg]                Polished 256-color DOG code editor
  dog studio [dir]                  Split-pane DOG Studio terminal IDE
  dog prompt <install|status|...>   Add/remove Kali-style DOG shell prompt & completions
  dog completions [bash|zsh|fish]   Generate shell tab-completion script
  dog forge new <name> [--web|--bot] Scaffold a new project (alias: dog new / dog init)

Packages (DOG Hub & dog.pack):
  dog add <path | pkg>              Add local package or cached dependency
  dog remove <pkg>                  Remove dependency from dog.pack
  dog search [query]                Search local ~/.dog/cache package index
  dog list                          List project dependencies
  dog info [pkg]                    Inspect project or cached package metadata

System, Server, Docs & SDU:
  dog docs [topic]                  Offline language/stdlib/network/CLI docs
  dog server [dir] [port] [--host]  Multi-threaded static HTTP file server
  dog doctor                        Inspect environment, Termux, rustc, and config
  dog system | status | env         Inspect host, project state, and masked env
  dog process | permissions         Inspect PID/memory and filesystem privileges
  dog logs [sdu] | clean | update   View logs, remove .dgb artifacts, check version
  dog config personality <mode>     Set normal|sarcastic|chaotic|professional|silent
  dog sdu <subcommand>              Super Dog User status, scoped auth, and audit logs
  dog version | help                Print version or this help message"
    );
}
