use crate::bytecode::{build_bundle, build_native, is_dgb_path, run_bundle_with_io};
use crate::web::build_web_bundle_to_disk;
use crate::checker::TypeChecker;
use crate::config::{parse_bool_flag, DogConfig, Personality};
use crate::diagnostics::{achievement_for_runs, suggest_similar, welcome_banner};
use crate::doctor::run_doctor;
use crate::editor::launch_editor;
use crate::formatter::format_source;
use crate::hub::{
    handle_info, handle_list, handle_search, install_package_live, remove_package_live, DogPack,
};
use crate::interpreter::{check_source, run_source_with_io, Interpreter};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::prompt::{handle_completions, handle_prompt, render_live_prompt};
use crate::sdu::handle_sdu;
use crate::stdlib::{StdIo, StdIoWithArgs};
use crate::studio::launch_studio;
use crate::sys::{
    handle_bench, handle_clean, handle_debug, handle_docs, handle_env, handle_eval_with_io,
    handle_explain, handle_forge, handle_graph, handle_inspect, handle_learn, handle_lint,
    handle_logs, handle_permissions, handle_process, handle_profile, handle_server,
    handle_status, handle_system, handle_update_exec, handle_watch, handle_web,
};
use crate::term::{InstallStyle, TermTheme};
use crate::tester::{run_coverage, run_tests};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut config = DogConfig::load();
    let messenger = config.messenger();

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
        "eval" | "e" => {
            if args.len() < 2 {
                eprintln!("Usage: dog eval \"<code-or-expression>\"");
                process::exit(2);
            }
            let snippet = args[1..].join(" ");
            config.record_run();
            let mut stdio = StdIo;
            if let Err(err) = handle_eval_with_io(&snippet, config.personality, &mut stdio) {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        "build" => {
            let native = args.iter().any(|a| a == "--native");
            let web = args.iter().any(|a| a == "--web");
            let mut input_path: Option<&str> = None;
            let mut out_path: Option<&str> = None;
            let mut i = 1usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--native" | "--web" => {}
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
                eprintln!("Usage: dog build <file.dg> [--out <path>] [--native | --web]");
                process::exit(2);
            };
            if native && web {
                messenger.error("Choose either `--native` or `--web` for `dog build`.");
                process::exit(2);
            }
            if web {
                match build_web_bundle_to_disk(input, out_path) {
                    Ok(art) => messenger.success(&format!(
                        "Built DOG web bundle: {} (+ {}, {})",
                        art.index_html_path.display(),
                        art.css_path.display(),
                        art.js_path.display()
                    )),
                    Err(e) => {
                        messenger.error(&e);
                        process::exit(1);
                    }
                }
            } else if native {
                match build_native(input, out_path) {
                    Ok(out) => messenger.success(&format!("Built native binary: {}", out.display())),
                    Err(e) => {
                        messenger.error(&e);
                        process::exit(1);
                    }
                }
            } else {
                match build_bundle(input, out_path) {
                    Ok((out, bundle)) => messenger.success(&format!(
                        "Built DOG bundle: {} (sha256:{})",
                        out.display(),
                        &bundle.sha256[..16]
                    )),
                    Err(e) => {
                        messenger.error(&e);
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
                Ok(_) => messenger.success(&format!("{path} passed syntax and strict type checks.")),
                Err(diag) => {
                    eprintln!("{}", diag.format_with(config.personality, Some(&source)));
                    process::exit(1);
                }
            }
        }
        "lint" => {
            let strict = args.iter().any(|a| a == "--strict");
            let path = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with('-'))
                .map(String::as_str);
            let Some(path) = path else {
                eprintln!("Usage: dog lint [--strict] <file.dg>");
                process::exit(2);
            };
            match handle_lint(path, strict) {
                Ok((report, warnings)) => {
                    println!("{report}");
                    if strict && warnings > 0 {
                        process::exit(1);
                    }
                }
                Err(err) => {
                    eprintln!("{err}");
                    process::exit(1);
                }
            }
        }
        "bench" | "benchmark" => {
            let mut target: Option<&str> = None;
            let mut runs: usize = 100;
            for arg in &args[1..] {
                if let Ok(n) = arg.parse::<usize>() {
                    if target.is_some() {
                        runs = n;
                    }
                } else if target.is_none() && !arg.starts_with('-') {
                    target = Some(arg.as_str());
                } else if let Some(val) = arg.strip_prefix("--runs=") {
                    if let Ok(n) = val.parse::<usize>() {
                        runs = n;
                    }
                }
            }
            let Some(path) = target else {
                eprintln!("Usage: dog bench <file.dg> [runs]");
                process::exit(2);
            };
            match handle_bench(path, runs, &messenger, true) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    messenger.error(&err);
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
                    messenger.success(&format!("{path} is already formatted."));
                } else {
                    messenger.warn(&format!("Needs formatting: {path}"));
                    process::exit(1);
                }
            } else if let Err(err) = fs::write(path, &formatted) {
                messenger.error(&format!("Failed to write '{path}': {err}"));
                process::exit(1);
            } else {
                messenger.success(&format!("Formatted {path}"));
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
        "coverage" => {
            let target = args.get(1).map(String::as_str);
            match run_coverage(target, config.personality) {
                Ok(report) => {
                    println!("{}", report.report_text);
                    if report.tests_failed > 0 {
                        process::exit(1);
                    }
                }
                Err(err) => {
                    messenger.error(&err);
                    process::exit(1);
                }
            }
        }
        "debug" => {
            let Some(path) = args.get(1) else {
                eprintln!("Usage: dog debug <file.dg>");
                process::exit(2);
            };
            if let Err(err) = handle_debug(path, config.personality) {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        "watch" => {
            if let Err(err) = handle_watch(&args[1..], config.personality) {
                messenger.error(&err);
                process::exit(1);
            }
        }
        "graph" => {
            let target = args.get(1).map(String::as_str);
            match handle_graph(target) {
                Ok(out) => println!("{out}"),
                Err(err) => {
                    messenger.error(&err);
                    process::exit(1);
                }
            }
        }
        "explain" => {
            let Some(target) = args.get(1) else {
                eprintln!("Usage: dog explain <E001-E041 | file.dg>");
                process::exit(2);
            };
            match handle_explain(target) {
                Ok(out) => println!("{out}"),
                Err(err) => {
                    messenger.error(&err);
                    process::exit(1);
                }
            }
        }
        "package" => {
            let mut input_path: Option<String> = None;
            let mut out_path: Option<String> = None;
            let mut i = 1usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--out" | "-o" => {
                        if let Some(v) = args.get(i + 1) {
                            out_path = Some(v.clone());
                            i += 1;
                        } else {
                            eprintln!("Missing output path after --out");
                            process::exit(2);
                        }
                    }
                    a if !a.starts_with('-') => {
                        if input_path.is_none() && a.ends_with(".dg") {
                            input_path = Some(a.to_string());
                        } else if out_path.is_none() {
                            out_path = Some(a.to_string());
                        } else {
                            eprintln!("Unexpected package argument '{a}'.");
                            process::exit(2);
                        }
                    }
                    other => {
                        eprintln!("Unknown option '{other}'. Usage: dog package [file.dg] [out.dgb]");
                        process::exit(2);
                    }
                }
                i += 1;
            }
            if input_path.is_none() && std::path::Path::new("dog.pack").is_file() {
                if let Ok(pack) = DogPack::load(std::path::Path::new("dog.pack")) {
                    input_path = Some(pack.entry.clone());
                    if out_path.is_none() {
                        out_path = Some(format!("{}-{}.dgb", pack.name, pack.version));
                    }
                }
            }
            let Some(entry) = input_path else {
                eprintln!("Usage: dog package [file.dg] [out.dgb] (or run inside a dog.pack project)");
                process::exit(2);
            };
            match build_bundle(&entry, out_path.as_deref()) {
                Ok((out, bundle)) => messenger.success(&format!(
                    "Packaged DOG archive: {} (sha256:{})",
                    out.display(),
                    &bundle.sha256[..16]
                )),
                Err(e) => {
                    messenger.error(&e);
                    process::exit(1);
                }
            }
        }
        "playground" => {
            let target = args.get(1).map(String::as_str);
            if launch_editor(target, config.personality).is_err() {
                config.record_run();
                run_repl(&config);
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
        "config" => handle_config_cmd(&args[1..], &mut config),
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
                    messenger.error(&e);
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
                    messenger.error(&e);
                    process::exit(1);
                }
            }
        }
        "docs" => {
            println!("{}", handle_docs(args.get(1).map(String::as_str)));
        }
        "learn" => {
            let run_demo = args.iter().skip(1).any(|a| a == "--run" || a == "-r");
            let topic = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with('-'))
                .map(String::as_str);
            match handle_learn(topic, run_demo, config.personality) {
                Ok(out) => println!("{out}"),
                Err(e) => {
                    messenger.error(&e);
                    process::exit(2);
                }
            }
        }
        "forge" => match handle_forge(&args[1..]) {
            Ok(out) => messenger.success(&out),
            Err(e) => {
                messenger.error(&e);
                process::exit(1);
            }
        },
        "new" => {
            let mut forge_args = vec!["new".to_string()];
            forge_args.extend(args[1..].iter().cloned());
            match handle_forge(&forge_args) {
                Ok(out) => messenger.success(&out),
                Err(e) => {
                    messenger.error(&e);
                    process::exit(1);
                }
            }
        }
        "init" | "innit" => {
            let mut forge_args = vec!["new".to_string(), ".".to_string()];
            forge_args.extend(args[1..].iter().cloned());
            match handle_forge(&forge_args) {
                Ok(out) => messenger.success(&out),
                Err(e) => {
                    messenger.error(&e);
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
        "web" => match handle_web(&args[1..]) {
            Ok(out) => {
                if !out.is_empty() {
                    println!("{out}");
                }
            }
            Err(e) => {
                messenger.error(&e);
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
        "update" | "upgrade" => match handle_update_exec(&args[1..]) {
            Ok(out) => println!("{out}"),
            Err(err) => {
                messenger.error(&err);
                process::exit(1);
            }
        },
        "clean" => println!("{}", handle_clean()),
        "logs" => match handle_logs(&args[1..]) {
            Ok(out) => print!("{out}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        },
        "add" => match install_package_live(&args[1..], &config) {
            Ok(out) => {
                if !out.is_empty() {
                    println!("{out}");
                }
            }
            Err(e) => {
                messenger.error(&e);
                process::exit(1);
            }
        },
        "remove" => match remove_package_live(&args[1..], &config) {
            Ok(out) => {
                if !out.is_empty() {
                    println!("{out}");
                }
            }
            Err(e) => {
                messenger.error(&e);
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
            const COMMANDS: &[&str] = &[
                "run", "eval", "build", "check", "lint", "bench", "benchmark", "format", "fmt",
                "test", "coverage", "debug", "watch", "graph", "explain", "package", "playground",
                "doctor", "edit", "editor", "studio", "config", "repl", "inspect", "profile",
                "docs", "learn", "forge", "new", "init", "prompt", "completions", "web", "server",
                "system", "status", "env", "process", "permissions", "update", "upgrade",
                "clean", "logs", "add", "remove", "search", "list", "info", "sdu", "version",
                "help",
            ];
            if let Some(hint) = suggest_similar(unknown, COMMANDS.iter().copied()) {
                eprintln!(
                    "Unknown command '{unknown}'. Did you mean `dog {hint}`?\nRun `dog --help` to see all commands."
                );
            } else {
                eprintln!("Unknown command '{unknown}'. Run `dog --help` to see working commands.");
            }
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
                    "if" | "unless"
                        | "match"
                        | "try"
                        | "while"
                        | "until"
                        | "repeat"
                        | "for"
                        | "fn"
                        | "define"
                        | "function"
                        | "struct"
                        | "enum"
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

fn handle_config_cmd(args: &[String], config: &mut DogConfig) {
    let messenger = config.messenger();
    match args.first().map(String::as_str) {
        None | Some("show") => {
            let summary = messenger.render_summary(
                "DOG Configuration & Terminal Capabilities",
                &[
                    ("Personality", config.personality.as_str().to_string()),
                    ("Theme", config.theme.as_str().to_string()),
                    ("Timestamps", config.timestamps.to_string()),
                    ("Prefix", config.prefix.clone()),
                    ("Debug Logs", config.debug_logs.to_string()),
                    ("Install Style", config.install_style.as_str().to_string()),
                    ("TTY", messenger.caps.is_tty.to_string()),
                    ("ANSI Color", messenger.use_color().to_string()),
                    ("Unicode", messenger.use_unicode().to_string()),
                    ("Term Width", messenger.caps.width.to_string()),
                ],
            );
            println!("{summary}");
        }
        Some("personality") => {
            if let Some(mode_raw) = args.get(1) {
                if let Some(mode) = Personality::parse(mode_raw) {
                    config.personality = mode;
                    if let Err(err) = config.save() {
                        messenger.error(&err);
                        process::exit(1);
                    }
                    config
                        .messenger()
                        .success(&format!("DOG personality set to '{}'.", mode.as_str()));
                } else {
                    messenger.error(&format!(
                        "Invalid personality '{mode_raw}'. Valid modes: normal, sarcastic, chaotic, professional, silent."
                    ));
                    process::exit(2);
                }
            } else {
                messenger.info(&format!("Current personality: {}", config.personality.as_str()));
            }
        }
        Some("theme") => {
            if let Some(theme_raw) = args.get(1) {
                if let Some(theme) = TermTheme::parse(theme_raw) {
                    config.theme = theme;
                    if let Err(err) = config.save() {
                        messenger.error(&err);
                        process::exit(1);
                    }
                    config
                        .messenger()
                        .success(&format!("DOG terminal theme set to '{}'.", theme.as_str()));
                } else {
                    messenger.error(&format!(
                        "Invalid theme '{theme_raw}'. Valid themes: {}.",
                        TermTheme::available().join(", ")
                    ));
                    process::exit(2);
                }
            } else {
                messenger.info(&format!("Current theme: {}", config.theme.as_str()));
            }
        }
        Some("timestamps") => {
            if let Some(flag_raw) = args.get(1) {
                if let Some(enabled) = parse_bool_flag(flag_raw) {
                    config.timestamps = enabled;
                    if let Err(err) = config.save() {
                        messenger.error(&err);
                        process::exit(1);
                    }
                    config.messenger().success(&format!(
                        "DOG CLI timestamps {}.",
                        if enabled { "enabled" } else { "disabled" }
                    ));
                } else {
                    messenger.error(&format!(
                        "Invalid timestamps value '{flag_raw}'. Use on|off or true|false."
                    ));
                    process::exit(2);
                }
            } else {
                messenger.info(&format!("Timestamps enabled: {}", config.timestamps));
            }
        }
        Some("prefix") => {
            if let Some(pref) = args.get(1) {
                config.prefix = pref.clone();
                if let Err(err) = config.save() {
                    messenger.error(&err);
                    process::exit(1);
                }
                config
                    .messenger()
                    .success(&format!("DOG CLI prefix set to '{}'.", config.prefix));
            } else {
                messenger.info(&format!("Current CLI prefix: '{}'", config.prefix));
            }
        }
        Some("debug") => {
            if let Some(flag_raw) = args.get(1) {
                if let Some(enabled) = parse_bool_flag(flag_raw) {
                    config.debug_logs = enabled;
                    if let Err(err) = config.save() {
                        messenger.error(&err);
                        process::exit(1);
                    }
                    config.messenger().success(&format!(
                        "DOG debug messages {}.",
                        if enabled { "enabled" } else { "disabled" }
                    ));
                } else {
                    messenger.error(&format!(
                        "Invalid debug value '{flag_raw}'. Use on|off or true|false."
                    ));
                    process::exit(2);
                }
            } else {
                messenger.info(&format!("Debug messages enabled: {}", config.debug_logs));
            }
        }
        Some("install-style") | Some("install_style") => {
            if let Some(style_raw) = args.get(1) {
                if let Some(style) = InstallStyle::parse(style_raw) {
                    config.install_style = style;
                    if let Err(err) = config.save() {
                        messenger.error(&err);
                        process::exit(1);
                    }
                    config
                        .messenger()
                        .success(&format!("Install output style set to '{}'.", style.as_str()));
                } else {
                    messenger.error(&format!(
                        "Invalid install style '{style_raw}'. Use detailed, compact, or minimal."
                    ));
                    process::exit(2);
                }
            } else {
                messenger.info(&format!("Current install style: {}", config.install_style.as_str()));
            }
        }
        Some(other) => {
            messenger.error(&format!("Unknown config option '{other}'."));
            eprintln!(
                "Usage:\n  dog config show\n  dog config personality [normal|sarcastic|chaotic|professional|silent]\n  dog config theme [default|kali|dracula|solar|mono]\n  dog config timestamps [on|off]\n  dog config prefix <label>\n  dog config debug [on|off]\n  dog config install-style [detailed|compact|minimal]"
            );
            process::exit(2);
        }
    }
}

fn print_help() {
    println!(
        "DOG Programming Language CLI ({VERSION})

Core Language & Tooling:
  dog run <file.dg | file.dgb>      Execute a .dg program or compiled .dgb bundle
  dog eval "<expr | stmt>"          Evaluate inline DOG code/expression immediately
  dog build <file.dg> [--native|--web] Compile to .dgb, native binary, or web site bundle
  dog package [file.dg] [out.dgb]   Package entry or dog.pack project into .dgb archive
  dog check <file.dg>               Type-check a .dg program without running it
  dog lint [--strict] <file.dg>     Static analysis (unused bindings, empty catch, style)
  dog bench <file.dg> [runs]        Benchmark execution latency (alias: benchmark)
  dog format [--check] <file.dg>    Format a .dg file with 4-space indentation (alias: fmt)
  dog test [path]                   Run *_test.dg test files
  dog coverage [path]               Measure statement line coverage across tests
  dog debug <file.dg>               Interactive step/breakpoint/variable debugger
  dog watch <file.dg> [--test]      Watch .dg files & imports and auto-rerun on save
  dog graph [target]                Visualize module, std.*, and dog.pack dependencies
  dog explain <E-code | file.dg>    Explain error codes or summarize a .dg module
  dog inspect <file.dg>             Print tokens, AST outline, and checked symbols
  dog profile <file.dg>             Measure lex, parse, check, and runtime timings

Interactive, Prompt & Workspace IDEs:
  dog repl | dog playground         Interactive DOG terminal or scratchpad editor
  dog edit [file.dg]                Polished 256-color DOG code editor
  dog studio [dir]                  Split-pane DOG Studio terminal IDE
  dog prompt <install|status|...>   Add/remove Kali-style DOG shell prompt & completions
  dog completions [bash|zsh|fish]   Generate shell tab-completion script
  dog forge new <name> [--web|--bot] Scaffold a new project (alias: dog new / dog init)

Packages (DOG Hub & dog.pack):
  dog add <path | pkg | url>        Staged install with dependency, progress & SHA-256 checks
  dog remove <pkg> [--purge]        Remove dependency from dog.pack (optionally purge cache)
  dog search [query]                Search local ~/.dog/cache package index
  dog list                          List project dependencies
  dog info [pkg]                    Inspect project or cached package metadata

System, Server, CLI Themes, Docs & SDU:
  dog learn [lesson] [--run]        Interactive beginner lessons & English syntax guide
  dog docs [std | topic | fn]       Offline std.*, web, css, english, CLI & SDU docs
  dog web <build|css|serve> ...     Compile .dg + DOG CSS (.dgss) into standalone websites
  dog server [dir] [port] [--host]  Multi-threaded static HTTP file server
  dog doctor                        Inspect environment, Termux, rustc, capabilities & config
  dog system | status | env         Inspect host, project state, and masked env
  dog process | permissions         Inspect PID/memory and filesystem privileges
  dog update [--check|--force]    Check version and auto-upgrade DOG (alias: upgrade)
  dog logs [sdu] | clean          View logs or remove temporary .dgb artifacts
  dog config <setting> [value]      Configure theme, timestamps, prefix, personality & style
  dog sdu <subcommand>              Super Dog User status, scoped auth, and audit logs
  dog version | help                Print version or this help message"
    );
}
