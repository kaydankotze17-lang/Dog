use crate::checker::TypeChecker;
use crate::config::DogConfig;
use crate::hub::DogPack;
use crate::interpreter::{resolve_program, run_source_with_io};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::sdu::{audit_log_path, detect_actual_root, session_path};
use crate::stdlib::StdIo;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub fn handle_inspect(path: &str) -> Result<String, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    let tokens = Lexer::new(path, &source)
        .tokenize()
        .map_err(|d| format!("{d}"))?;
    let tok_count = tokens.len();

    let ast_raw = Parser::new(tokens).parse().map_err(|d| format!("{d}"))?;
    let resolved = resolve_program(path, &source).map_err(|d| format!("{d}"))?;
    let mut checker = TypeChecker::new();
    checker.check(&resolved).map_err(|d| format!("{d}"))?;

    let mut out = String::new();
    out.push_str(&format!("DOG Inspect: {path}\n"));
    out.push_str("-------------------------\n");
    out.push_str(&format!("Tokens (incl. EOF): {tok_count}\n"));
    out.push_str(&format!("Entry AST stmts   : {}\n", ast_raw.statements.len()));
    out.push_str(&format!("Resolved AST stmts: {}\n", resolved.statements.len()));
    out.push_str("\nAST Outline:\n");
    for (idx, stmt) in resolved.statements.iter().enumerate() {
        out.push_str(&format!("  {:>2}. {:?}\n", idx + 1, stmt_summary(stmt)));
    }
    out.push_str("\nType-Checked Symbols:\n");
    for (name, ty) in checker.list_symbols() {
        out.push_str(&format!("  {name:<24} : {ty}\n"));
    }
    Ok(out)
}

fn stmt_summary(stmt: &crate::ast::Stmt) -> String {
    use crate::ast::Stmt;
    match stmt {
        Stmt::Use { path, .. } => format!("use \"{path}\""),
        Stmt::StructDef { name, fields, .. } => format!("struct {name} ({} fields)", fields.len()),
        Stmt::EnumDef { name, variants, .. } => format!("enum {name} ({} variants)", variants.len()),
        Stmt::Binding { kind, name, .. } => format!("{kind:?} {name}"),
        Stmt::Assign { name, .. } => format!("assign {name}"),
        Stmt::FieldAssign { target, field, .. } => format!("assign {target}.{field}"),
        Stmt::IndexAssign { target, .. } => format!("assign {target}[...]"),
        Stmt::If { .. } => "if ... end".to_string(),
        Stmt::Match { cases, .. } => format!("match ({} cases)", cases.len()),
        Stmt::TryCatch { err_name, .. } => format!("try ... catch {err_name}"),
        Stmt::While { .. } => "while ... end".to_string(),
        Stmt::Repeat { .. } => "repeat ... end".to_string(),
        Stmt::For { var_name, .. } => format!("for {var_name} in ..."),
        Stmt::FnDef { name, params, .. } => format!("fn {name}({} params)", params.len()),
        Stmt::Return { .. } => "return".to_string(),
        Stmt::Break { .. } => "break".to_string(),
        Stmt::Continue { .. } => "continue".to_string(),
        Stmt::ExprStmt { .. } => "expr".to_string(),
    }
}

pub fn handle_profile(path: &str) -> Result<String, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;

    let t0 = Instant::now();
    let tokens = Lexer::new(path, &source)
        .tokenize()
        .map_err(|d| format!("{d}"))?;
    let lex_us = t0.elapsed().as_micros();

    let t1 = Instant::now();
    let _ast = Parser::new(tokens).parse().map_err(|d| format!("{d}"))?;
    let parse_us = t1.elapsed().as_micros();

    let t2 = Instant::now();
    let resolved = resolve_program(path, &source).map_err(|d| format!("{d}"))?;
    let mut checker = TypeChecker::new();
    checker.check(&resolved).map_err(|d| format!("{d}"))?;
    let check_us = t2.elapsed().as_micros();

    let mut stdio = StdIo;
    let t3 = Instant::now();
    run_source_with_io(path, &source, &mut stdio).map_err(|d| format!("{d}"))?;
    let run_us = t3.elapsed().as_micros();

    Ok(format!(
        "\nDOG Profile: {path}\n\
         -------------------------\n\
         Lexing        : {lex_us} us\n\
         Parsing       : {parse_us} us\n\
         Type Checking : {check_us} us\n\
         Execution     : {run_us} us\n\
         Total         : {} us\n\
         Resolved AST  : {} top-level statement(s)",
        lex_us + parse_us + check_us + run_us,
        resolved.statements.len()
    ))
}

pub fn handle_docs(topic: Option<&str>) -> String {
    match topic.unwrap_or("overview").to_ascii_lowercase().as_str() {
        "syntax" => {
            "DOG Syntax Reference\n\
             --------------------\n\
             let x = 10               immutable binding\n\
             var y = 0                mutable binding\n\
             const PI = 3.14          constant\n\
             struct Dog ... end       product type\n\
             enum Role ... end        sum/variant type\n\
             if ... otherwise ... end conditional\n\
             match x case 1 ... end   pattern match\n\
             try ... catch err ... end error handling\n\
             while / repeat / for     loops (break/continue supported)\n\
             fn add(a: number, b: number) -> number ... end\n\
             use \"module.dg\"          local module import"
                .to_string()
        }
        "types" => {
            "DOG Type System\n\
             ---------------\n\
             Primitives : text, number (i64), decimal (f64), bool, void\n\
             Generics   : list<T>, map<K, V>\n\
             User types : struct Name ... end, enum Name ... end\n\
             Rules      : Strict typing (no implicit text + number coercion).\n\
                          Explicit conversion: text(x), number(x), decimal(x)."
                .to_string()
        }
        "stdlib" => {
            "DOG Standard Library\n\
             --------------------\n\
             I/O     : say(), answer(), log_info(), log_warn(), log_error()\n\
             Text    : lower(), upper(), trim(), contains(), split(), join(), len()\n\
             Cast    : text(), number(), decimal()\n\
             Math    : abs(), min(), max()\n\
             List    : push(), pop(), len()\n\
             Map     : map_get(), map_set(), map_has(), map_keys(), map_remove()\n\
             Files   : file_read(), file_write(), file_exists(), file_delete(), dir_list()\n\
             System  : env_get(), env_set(), time_now_ms(), sleep_ms()\n\
             JSON    : json_parse(), json_stringify()\n\
             Crypto  : hex_encode(), hex_decode(), base64_encode(), base64_decode(), sha256()\n\
             Network : http_get() (plain http://)\n\
             Errors  : assert(), fail()"
                .to_string()
        }
        "errors" => {
            "DOG Error Codes\n\
             ---------------\n\
             E001-E005 : Lexer & Syntax errors (strings, numbers, missing 'end')\n\
             E010-E014 : Name, Mutability (`let`/`const`), Function & Arity errors\n\
             E021-E026 : Strict Type Mismatches, Condition bool errors, Return paths\n\
             E030-E035 : Runtime conversions, Division by zero, Bounds, assert(), fail()\n\
             E040-E041 : I/O, Network, Module loading & Circular import errors"
                .to_string()
        }
        "editor" => {
            "DOG Editor & Studio Shortcuts\n\
             -----------------------------\n\
             Ctrl+S Save   | Ctrl+O Open   | Ctrl+R Run\n\
             Ctrl+L Format | Ctrl+F Find   | Ctrl+Z/Y Undo/Redo\n\
             Ctrl+T NewTab | Ctrl+N Next   | Ctrl+G Help\n\
             Ctrl+W Switch Pane (Studio)   | Ctrl+P Palette (Studio)\n\
             Ctrl+Q Quit   | Ctrl+X Force Quit"
                .to_string()
        }
        "hub" => {
            "DOG Hub (Offline Package & Project Manifest)\n\
             --------------------------------------------\n\
             Manifest : dog.pack ([package], [dependencies], [scripts])\n\
             Lockfile : dog.lock (SHA-256 hashes)\n\
             Commands : dog add <path|pkg>, dog remove <pkg>, dog search [q],\n\
                        dog list, dog info [pkg], dog forge new <name>"
                .to_string()
        }
        "sdu" => {
            "SDU (Super Dog User)\n\
             --------------------\n\
             dog sdu status          Show real kernel UID & SDU session\n\
             dog sdu auth --confirm  Authorize temporary scoped session\n\
             dog sdu permissions     Inspect active SDU capability matrix\n\
             dog sdu revoke          Invalidate active SDU session\n\
             dog sdu logs            View ~/.dog/logs/sdu_audit.log"
                .to_string()
        }
        _ => {
            "DOG Documentation Topics\n\
             ------------------------\n\
             Run `dog docs <topic>` with one of:\n\
               syntax, types, stdlib, errors, editor, hub, sdu"
                .to_string()
        }
    }
}

pub fn handle_forge(args: &[String]) -> Result<String, String> {
    if args.first().map(String::as_str) != Some("new") {
        return Err("Usage: dog forge new <project-name>".to_string());
    }
    let Some(name) = args.get(1) else {
        return Err("Usage: dog forge new <project-name>".to_string());
    };
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(format!("Invalid project directory name '{name}'."));
    }
    let root = Path::new(name);
    if root.exists() {
        return Err(format!("Directory '{name}' already exists."));
    }

    fs::create_dir_all(root.join("src"))
        .map_err(|e| format!("Failed to create '{name}/src': {e}"))?;
    fs::create_dir_all(root.join("tests"))
        .map_err(|e| format!("Failed to create '{name}/tests': {e}"))?;

    let mut pack = DogPack {
        name: name.clone(),
        version: "0.1.0".to_string(),
        entry: "src/main.dg".to_string(),
        description: format!("{name} built with DOG"),
        ..DogPack::default()
    };
    pack.scripts.insert("start".to_string(), "dog run src/main.dg".to_string());
    pack.scripts.insert("test".to_string(), "dog test tests".to_string());

    fs::write(root.join("dog.pack"), pack.render())
        .map_err(|e| format!("Failed to write dog.pack: {e}"))?;
    fs::write(
        root.join("src/main.dg"),
        format!("let app_name = \"{name}\"\nsay(\"Welcome to \" + app_name + \"!\")\n"),
    )
    .map_err(|e| format!("Failed to write src/main.dg: {e}"))?;
    fs::write(
        root.join("tests/main_test.dg"),
        "fn add(a: number, b: number) -> number\n    return a + b\nend\n\nassert(add(2, 3) == 5, \"math works\")\n",
    )
    .map_err(|e| format!("Failed to write tests/main_test.dg: {e}"))?;
    fs::write(
        root.join("README.md"),
        format!("# {name}\n\n```bash\ndog run src/main.dg\ndog test tests\n```\n"),
    )
    .map_err(|e| format!("Failed to write README.md: {e}"))?;
    fs::write(root.join(".gitignore"), "*.dgb\n")
        .map_err(|e| format!("Failed to write .gitignore: {e}"))?;

    Ok(format!(
        "Forged new DOG project '{name}':\n  {name}/dog.pack\n  {name}/src/main.dg\n  {name}/tests/main_test.dg\n  {name}/README.md"
    ))
}

pub fn handle_server(args: &[String]) -> Result<(), String> {
    let dir = args.first().map(String::as_str).unwrap_or(".");
    let port: u16 = args
        .get(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8080);

    let root = fs::canonicalize(dir)
        .map_err(|e| format!("Failed to resolve directory '{dir}': {e}"))?;
    let addr = format!("127.0.0.1:{port}");
    let listener =
        TcpListener::bind(&addr).map_err(|e| format!("Failed to bind {addr}: {e}"))?;

    println!("DOG static server listening on http://{addr} (root: {})", root.display());
    println!("Press Ctrl+C to stop.");

    for stream_res in listener.incoming() {
        let Ok(mut stream) = stream_res else {
            continue;
        };
        let mut buf = [0u8; 4096];
        let Ok(n) = stream.read(&mut buf) else {
            continue;
        };
        if n == 0 {
            continue;
        }
        let req = String::from_utf8_lossy(&buf[..n]);
        let req_line = req.lines().next().unwrap_or("");
        let mut parts = req_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let raw_target = parts.next().unwrap_or("/");

        if method != "GET" && method != "HEAD" {
            let body = b"405 Method Not Allowed\n";
            let _ = write_http_response(&mut stream, "405 Method Not Allowed", "text/plain", body);
            continue;
        }

        let clean_path = raw_target.split('?').next().unwrap_or("/");
        if let Some(target_file) = resolve_safe_path(&root, clean_path) {
            if target_file.is_dir() {
                let index = target_file.join("index.html");
                if index.is_file() {
                    if let Ok(data) = fs::read(&index) {
                        let _ = write_http_response(&mut stream, "200 OK", "text/html; charset=utf-8", &data);
                        continue;
                    }
                }
                let listing = build_dir_listing(&target_file, clean_path);
                let _ = write_http_response(
                    &mut stream,
                    "200 OK",
                    "text/html; charset=utf-8",
                    listing.as_bytes(),
                );
            } else if target_file.is_file() {
                match fs::read(&target_file) {
                    Ok(data) => {
                        let ctype = guess_content_type(&target_file);
                        let _ = write_http_response(&mut stream, "200 OK", ctype, &data);
                    }
                    Err(_) => {
                        let _ = write_http_response(
                            &mut stream,
                            "500 Internal Server Error",
                            "text/plain",
                            b"Read Error\n",
                        );
                    }
                }
            } else {
                let _ = write_http_response(&mut stream, "404 Not Found", "text/plain", b"404 Not Found\n");
            }
        } else {
            let _ = write_http_response(&mut stream, "403 Forbidden", "text/plain", b"403 Forbidden\n");
        }
    }
    Ok(())
}

fn resolve_safe_path(root: &Path, url_path: &str) -> Option<PathBuf> {
    let mut rel = PathBuf::new();
    for seg in url_path.trim_start_matches('/').split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return None;
        }
        let p = Path::new(seg);
        if matches!(p.components().next(), Some(Component::Normal(_))) {
            rel.push(seg);
        } else {
            return None;
        }
    }
    let joined = root.join(rel);
    if !joined.exists() {
        return None;
    }
    let canon = fs::canonicalize(&joined).ok()?;
    if canon.starts_with(root) {
        Some(canon)
    } else {
        None
    }
}

fn build_dir_listing(dir: &Path, url_path: &str) -> String {
    let mut items = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                let suffix = if entry.path().is_dir() { "/" } else { "" };
                let prefix = url_path.trim_end_matches('/');
                items.push(format!("<li><a href=\"{prefix}/{name}{suffix}\">{name}{suffix}</a></li>"));
            }
        }
    }
    items.sort();
    format!(
        "<!doctype html><html><body><h1>DOG Server: {url_path}</h1><ul>{}</ul></body></html>",
        items.join("\n")
    )
}

fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "dg" | "md" | "txt" | "rs" | "toml" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn write_http_response(
    stream: &mut std::net::TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)
}

pub fn handle_system() -> String {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let is_termux = env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    let version = env!("CARGO_PKG_VERSION");

    format!(
        "DOG System Info\n\
         ---------------\n\
         CLI Version : dog {version}\n\
         OS / Arch   : {}/{}\n\
         CPU Threads : {cpus}\n\
         Termux      : {}\n\
         Actual Root : {}",
        env::consts::OS,
        env::consts::ARCH,
        if is_termux { "yes" } else { "no" },
        if detect_actual_root() { "yes" } else { "no" }
    )
}

pub fn handle_status() -> String {
    let cfg = DogConfig::load();
    let has_pack = Path::new("dog.pack").is_file();
    let has_lock = Path::new("dog.lock").is_file();
    let dg_count = count_dg_files(Path::new("."));
    let sdu_active = session_path().exists();

    format!(
        "DOG Workspace Status\n\
         --------------------\n\
         Personality   : {}\n\
         Sessions Run  : {}\n\
         dog.pack      : {}\n\
         dog.lock      : {}\n\
         .dg files     : {}\n\
         SDU session   : {}",
        cfg.personality.as_str(),
        cfg.runs_completed,
        if has_pack { "present" } else { "none" },
        if has_lock { "present" } else { "none" },
        dg_count,
        if sdu_active { "present" } else { "none" }
    )
}

fn count_dg_files(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0usize;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                if n == "target" || n.starts_with('.') {
                    continue;
                }
            }
            total += count_dg_files(&p);
        } else if p.extension().and_then(|s| s.to_str()) == Some("dg") {
            total += 1;
        }
    }
    total
}

pub fn handle_env() -> String {
    let mut vars: Vec<(String, String)> = env::vars().collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::from("Environment Variables (sensitive values masked):\n");
    for (k, v) in vars {
        let upper = k.to_ascii_uppercase();
        let sensitive = upper.contains("KEY")
            || upper.contains("TOKEN")
            || upper.contains("SECRET")
            || upper.contains("PASS")
            || upper.contains("AUTH")
            || upper.contains("CREDENTIAL");
        let display = if sensitive { "[REDACTED]" } else { &v };
        out.push_str(&format!("  {k}={display}\n"));
    }
    out
}

pub fn handle_process() -> String {
    let pid = std::process::id();
    let exe = env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    let mut rss = "unavailable".to_string();
    let mut threads = "unavailable".to_string();
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(v) = line.strip_prefix("VmRSS:") {
                rss = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("Threads:") {
                threads = v.trim().to_string();
            }
        }
    }

    format!(
        "DOG Process Info\n\
         ----------------\n\
         PID        : {pid}\n\
         Executable : {exe}\n\
         CWD        : {cwd}\n\
         Threads    : {threads}\n\
         Memory RSS : {rss}"
    )
}

pub fn handle_permissions() -> String {
    let cwd_writable = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(".dog_perm_probe")
        .map(|_| {
            let _ = fs::remove_file(".dog_perm_probe");
            true
        })
        .unwrap_or(false);
    let home = DogConfig::home_dir();
    let home_exists = home.exists();
    format!(
        "DOG Permissions\n\
         ---------------\n\
         CWD writable : {}\n\
         ~/.dog path  : {} ({})\n\
         Actual root  : {}",
        cwd_writable,
        home.display(),
        if home_exists { "exists" } else { "not yet initialized" },
        detect_actual_root()
    )
}

pub fn handle_update() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut msg = format!("Current DOG CLI version: {version}\n");
    if Path::new(".git").exists() {
        if let Ok(out) = Command::new("git").args(["status", "-sb"]).output() {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            msg.push_str(&format!("Local git checkout : {branch}\n"));
        }
        msg.push_str("To update from source: git pull && cargo install --path .");
    } else {
        msg.push_str(
            "Self-update from a remote binary host is not configured in v0.1.0.\nUpdate via git & cargo: `git pull && cargo install --path .`",
        );
    }
    msg
}

pub fn handle_clean() -> String {
    let mut removed = Vec::new();
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("dgb") {
                if fs::remove_file(&p).is_ok() {
                    removed.push(p.display().to_string());
                }
            }
        }
    }
    if removed.is_empty() {
        "Workspace clean: no temporary .dgb artifacts found.".to_string()
    } else {
        format!("Removed {} artifact(s):\n  {}", removed.len(), removed.join("\n  "))
    }
}

pub fn handle_logs(args: &[String]) -> Result<String, String> {
    if args.first().map(String::as_str) == Some("sdu") {
        let p = audit_log_path();
        if !p.exists() {
            return Ok(format!("No SDU audit logs yet at {}.", p.display()));
        }
        return fs::read_to_string(&p).map_err(|e| format!("Failed to read SDU logs: {e}"));
    }
    let log_file = DogConfig::home_dir().join("logs").join("dog.log");
    if !log_file.exists() {
        return Ok(format!("No DOG CLI logs recorded yet at {}.", log_file.display()));
    }
    fs::read_to_string(&log_file).map_err(|e| format!("Failed to read DOG logs: {e}"))
}
