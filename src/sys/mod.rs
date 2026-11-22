use crate::ast::{BinOp, Expr, InterpPart, Program, Stmt};
use crate::checker::TypeChecker;
use crate::config::{DogConfig, Personality};
use crate::editor::BufferRunIo;
use crate::formatter::format_source;
use crate::hub::DogPack;
use crate::interpreter::{resolve_program, run_source_with_io, Interpreter};
use crate::lexer::Lexer;
use crate::net::guess_mime_type;
use crate::parser::Parser;
use crate::sdu::{audit_log_path, detect_actual_root, session_path};
use crate::stdlib::{InterpreterIo, StdIo};
use crate::term::{DogMessenger, ProgressBar, ProgressUnit};
use crate::types::DogType;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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
        Stmt::CompoundAssign { name, .. } => format!("compound_assign {name}"),
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
             var y = 0                mutable binding (`y += 5`, `-=`, `*=`, `/=`, `%=`)\n\
             const PI = 3.14          constant\n\
             $\"Hi {name}, {x + 1}!\"   string interpolation (plain \"...\" stays literal)\n\
             struct Dog ... end       product type\n\
             enum Role ... end        sum/variant type\n\
             if ... otherwise if ... otherwise ... end   conditional chain\n\
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
             CLI UI  : cli_info(), cli_success(), cli_warn(), cli_error(), cli_debug(),\n\
                       cli_progress(), cli_spinner(), cli_theme(), cli_prefix(),\n\
                       cli_timestamps(), cli_supports_color(), cli_is_tty()\n\
             Text    : lower(), upper(), trim(), contains(), starts_with(), ends_with(),\n\
                       replace(), slice(), index_of(), last_index_of(), count_matches(),\n\
                       char_at(), remove_prefix(), remove_suffix(), title_case(),\n\
                       reverse_text(), lines(), str_repeat(), pad_left(), pad_right(),\n\
                       split(), join(), glob_match(), html_escape(), len()\n\
             Cast    : text(), number(), decimal()\n\
             Math    : abs(), min(), max(), clamp(), round(), floor(), ceil(), sqrt(),\n\
                       pow(), pow_int(), sin(), cos(), tan(), atan2(), exp(), log(),\n\
                       log10(), sign(), gcd(), lcm(), math_pi(), math_e(), rand_int()\n\
             List    : push(), pop(), insert_at(), remove_at(), clear(), sort(), reverse(),\n\
                       list_contains(), list_index_of(), list_slice(), list_unique(),\n\
                       list_take(), list_skip(), list_flatten(), list_sum(), list_avg(),\n\
                       list_concat(), range(), len()\n\
             Map     : map_get(), map_get_or(), map_set(), map_has(), map_keys(),\n\
                       map_values(), map_remove(), map_clear(), map_merge(), map_from_lists()\n\
             Files   : file_read(), file_write(), file_append(), file_copy(), file_move(),\n\
                       file_exists(), file_delete(), file_size(), is_file(), is_dir(),\n\
                       path_join(), path_dirname(), path_basename(), path_ext(),\n\
                       dir_list(), mkdir(), rmdir(), temp_dir(), cwd()\n\
             KV DB   : kv_set(), kv_get(), kv_has(), kv_delete(), kv_keys()\n\
             System  : args(), env_get(), env_set(), exec_cmd(), os_name(), os_arch(),\n\
                       time_now_ms(), time_utc(), sleep_ms(), uuid_v4()\n\
             Interop : json_parse(), json_stringify(), json_pretty(), json_has_key(),\n\
                       csv_parse(), csv_stringify(), url_encode(), url_decode(),\n\
                       url_parse(), query_parse(), query_stringify()\n\
             Crypto  : hex_encode(), hex_decode(), base64_encode(), base64_decode(),\n\
                       base64_url_encode(), base64_url_decode(), sha256(), sha512(),\n\
                       hmac_sha256(), hmac_sha256_verify(), sha1(), rand_bytes_hex()\n\
             Network : http_listen(), http_port(), http_accept(), http_try_accept(),\n\
                       http_reply(), http_reply_headers(), http_redirect(), http_serve_file(),\n\
                       http_route_match(), http_status_text(), http_close(), http_get(),\n\
                       http_post(), http_put(), http_delete(), http_request(),\n\
                       http_request_timeout(), ws_upgrade(), ws_connect(), ws_send(),\n\
                       ws_recv(), ws_ping(), ws_close(), udp_bind(), udp_send_to(),\n\
                       udp_recv_from(), udp_close(), tcp_listen(), tcp_port(),\n\
                       tcp_accept(), tcp_connect(), tcp_send(), tcp_recv(),\n\
                       tcp_set_timeout(), tcp_remote_addr(), tcp_close(), dns_lookup(),\n\
                       port_is_open(), local_ip()\n\
             Errors  : assert(), fail()"
                .to_string()
        }
        "network" | "net" | "web" | "bot" => {
            "DOG Networking, WebSockets, UDP, Websites & Bots\n\
             ----------------------------------------------\n\
             HTTP Server : let srv = http_listen(\"127.0.0.1:8080\")  (\"0.0.0.0:8080\" for LAN)\n\
                           http_accept(srv) / http_try_accept(srv, timeout_ms)\n\
                           Supports Content-Length and Transfer-Encoding: chunked request bodies\n\
                           http_reply(req, 200, \"text/html\", html) | http_status_text(200)\n\
                           http_reply_headers(req, 200, hdrs_map, body) | http_redirect(req, 302, loc)\n\
                           http_serve_file(req, \"public/index.html\") | http_route_match(\"/u/:id\", p)\n\
                           url_parse(u), query_parse(q), query_stringify(m), html_escape(s)\n\
             HTTP Client : http_get(url), http_post(url, body, ctype), http_put(...), http_delete(url)\n\
                           http_request(method, url, hdrs, body) / http_request_timeout(..., ms)\n\
                           Follows redirects (301/302/303/307/308), returns status, body, headers, final_url, ok\n\
             WebSockets  : ws_upgrade(req) -> ws_id, ws_connect(\"ws://...\") -> ws_id\n\
                           ws_send(ws_id, msg), ws_recv(ws_id), ws_ping(ws_id, msg), ws_close(ws_id)\n\
             UDP & DNS   : udp_bind(addr), udp_send_to(id, addr, data), udp_recv_from(id, max), udp_close(id)\n\
                           dns_lookup(host), local_ip(), port_is_open(addr, ms),\n\
                           tcp_set_timeout(id, ms), tcp_remote_addr(id)\n\
             Raw TCP     : tcp_listen, tcp_accept, tcp_connect, tcp_send, tcp_recv, tcp_close\n\
             Static Host : dog server [dir] [port] [--host 0.0.0.0]"
                .to_string()
        }
        "errors" => {
            "DOG Error Codes\n\
             ---------------\n\
             E001-E005 : Lexer & Syntax errors (strings, numbers, missing 'end')\n\
             E010-E014 : Name (with 'Did you mean' hints), Mutability, Function & Arity errors\n\
             E021-E026 : Strict Type Mismatches, Condition bool errors, Return paths\n\
             E030-E035 : Runtime conversions, Division by zero, Bounds, assert(), fail()\n\
             E040-E041 : I/O, Network, Module loading & Circular import errors"
                .to_string()
        }
        "editor" => {
            "DOG Editor & Studio Shortcuts\n\
             -----------------------------\n\
             Alt+W / :wrap      : Toggle Soft Word-Wrap (auto-ON on phone screens < 72 cols)\n\
             Ctrl+Space / Alt+/ : Open Live IntelliSense Suggestion Popup\n\
             Tab / Right@EOL    : Accept Selected Suggestion / Snippet / Inline Ghost\n\
             Up / Down (popup)  : Cycle Contextual Suggestions (Up/Down/PgUp/PgDn in Output)\n\
             Ctrl+S Save        | Ctrl+O Open        | Ctrl+R Run\n\
             Ctrl+L Format      | Ctrl+F Find        | Ctrl+P Palette (:line, wrap, snippet)\n\
             Ctrl+D Dup Ln      | Ctrl+K Del Ln      | Ctrl+U Toggle # Comment\n\
             Ctrl+Z/Y Undo/Redo | Ctrl+T NewTab      | Ctrl+N Next | Ctrl+G Help\n\
             Studio Explorer    : `n` New .dg File   | `r` Refresh | `Ctrl+W` Switch Pane"
                .to_string()
        }
        "prompt" => {
            "DOG Terminal Prompt & Shell Integration\n\
             --------------------------------------\n\
             dog prompt preview              Preview Kali-style 2-line DOG prompt\n\
             dog prompt install              Install into ~/.bashrc, ~/.zshrc, or fish\n\
             dog prompt install --no-banner  Skip shell login greeting\n\
             dog prompt uninstall            Remove DOG prompt block cleanly\n\
             dog prompt status               Check installed RC files\n\
             dog completions [bash|zsh|fish] Print tab completion script"
                .to_string()
        }
        "cli" | "themes" | "term" => {
            "DOG CLI Experience & Custom Terminal Output\n\
             ------------------------------------------\n\
             Config   : dog config show | theme <default|kali|dracula|solar|mono>\n\
                        dog config timestamps <on|off> | prefix <label> | debug <on|off>\n\
             Scripts  : cli_theme(\"kali\"), cli_prefix(\"bot\"), cli_timestamps(true)\n\
                        cli_info(m), cli_success(m), cli_warn(m), cli_error(m), cli_debug(m)\n\
                        cli_progress(\"Sync\", done, total), cli_spinner(step, \"Working\")\n\
                        cli_supports_color(), cli_is_tty()\n\
             Fallback : Automatically respects NO_COLOR, TERM=dumb, CLICOLOR_FORCE, UTF-8/ASCII"
                .to_string()
        }
        "hub" => {
            "DOG Hub (Package Manager & Custom Lifecycle Messages)\n\
             -----------------------------------------------------\n\
             Manifest : dog.pack ([package], [dependencies], [scripts], [messages])\n\
             Custom   : Under [messages], developers can define install_banner, pre_install,\n\
                        post_install, uninstall_banner, pre_uninstall, post_uninstall\n\
             Lockfile : dog.lock (real verified SHA-256 tree digests)\n\
             Commands : dog add <path|pkg|http(s)://url>, dog remove <pkg> [--purge],\n\
                        dog search [q], dog list, dog info [pkg], dog forge new <name>"
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
            if let Some(raw_name) = topic.map(str::trim).filter(|s| !s.is_empty()) {
                if let Some(sig) = crate::editor::builtin_signature_help(raw_name) {
                    return format!(
                        "DOG Built-in Function: {raw_name}\n\
                         ----------------------------------\n\
                         {sig}\n\n\
                         Tip: Use `dog docs stdlib` for the full grouped library index."
                    );
                }
            }
            "DOG Documentation Topics\n\
             ------------------------\n\
             Run `dog docs <topic>` with one of:\n\
               syntax, types, stdlib, cli, network, errors, editor, prompt, hub, sdu\n\
             Or pass any built-in function name (e.g. `dog docs ws_ping`, `dog docs sha512`)."
                .to_string()
        }
    }
}

pub fn handle_eval_with_io(
    snippet: &str,
    personality: Personality,
    io: &mut dyn InterpreterIo,
) -> Result<(), String> {
    let trimmed = snippet.trim();
    if trimmed.is_empty() {
        return Err("Provide a DOG expression or statement to evaluate.".to_string());
    }
    let tokens = Lexer::new("<eval>", trimmed)
        .tokenize()
        .map_err(|d| d.format_with(personality, Some(trimmed)))?;
    let mut program = Parser::new(tokens)
        .parse()
        .map_err(|d| d.format_with(personality, Some(trimmed)))?;

    let mut checker = TypeChecker::new();
    checker
        .check(&program)
        .map_err(|d| d.format_with(personality, Some(trimmed)))?;

    if program.statements.len() == 1 {
        if let Some(Stmt::ExprStmt { expr, span }) = program.statements.first().cloned() {
            let is_say = matches!(&expr, Expr::Call { callee, .. } if callee == "say");
            if !is_say {
                let mut single_checker = TypeChecker::new();
                if let Ok(ty) = single_checker.infer_single_expr(&expr) {
                    if ty != DogType::Void {
                        program = Program {
                            statements: vec![Stmt::ExprStmt {
                                expr: Expr::Call {
                                    callee: "say".to_string(),
                                    args: vec![expr],
                                    span: span.clone(),
                                },
                                span,
                            }],
                        };
                    }
                }
            }
        }
    }

    let mut interp = Interpreter::new(io);
    interp
        .eval_program(&program)
        .map_err(|d| d.format_with(personality, Some(trimmed)))
}

pub fn eval_snippet_captured(snippet: &str, personality: Personality) -> Result<String, String> {
    let mut io = BufferRunIo {
        output: String::new(),
    };
    handle_eval_with_io(snippet, personality, &mut io)?;
    Ok(io.output)
}

pub fn handle_lint(path: &str, strict: bool) -> Result<(String, usize), String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    let tokens = Lexer::new(path, &source)
        .tokenize()
        .map_err(|d| format!("{d}"))?;
    let raw_program = Parser::new(tokens).parse().map_err(|d| format!("{d}"))?;
    let resolved = resolve_program(path, &source).map_err(|d| format!("{d}"))?;
    let mut checker = TypeChecker::new();
    checker.check(&resolved).map_err(|d| format!("{d}"))?;

    let mut warnings: Vec<String> = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let ln = idx + 1;
        if line.starts_with('\t') {
            warnings.push(format!(
                "[L002] {path}:{ln}: Leading tab indentation detected; prefer 4 spaces."
            ));
        }
        if line.ends_with(' ') || line.ends_with('\t') {
            warnings.push(format!(
                "[L003] {path}:{ln}: Trailing whitespace detected."
            ));
        }
        if line.chars().count() > 100 {
            warnings.push(format!(
                "[L001] {path}:{ln}: Line exceeds 100 characters ({} chars).",
                line.chars().count()
            ));
        }
    }

    if format_source(&source) != source {
        warnings.push(format!(
            "[L004] {path}:1: File formatting differs from canonical `dog fmt` output."
        ));
    }

    collect_lint_ast_warnings(path, &raw_program.statements, &mut warnings);

    let count = warnings.len();
    let mut report = format!("DOG Lint: {path}\n-----------------\n");
    if warnings.is_empty() {
        report.push_str("0 warnings — clean code.");
    } else {
        for w in &warnings {
            report.push_str(&format!("  {w}\n"));
        }
        report.push_str(&format!(
            "Summary: {count} warning(s){}",
            if strict { " (strict mode: failing)" } else { "" }
        ));
    }
    Ok((report, count))
}

fn collect_lint_ast_warnings(path: &str, stmts: &[Stmt], warnings: &mut Vec<String>) {
    let mut used = HashSet::new();
    for s in stmts {
        collect_stmt_reads(s, &mut used);
    }

    for s in stmts {
        match s {
            Stmt::Binding { name, span, .. } => {
                if !name.starts_with('_') && !used.contains(name) {
                    warnings.push(format!(
                        "[L005] {path}:{}: Binding '{name}' appears unused (prefix with `_{name}` to silence).",
                        span.line
                    ));
                }
            }
            Stmt::TryCatch {
                try_body,
                catch_body,
                span,
                ..
            } => {
                if catch_body.is_empty() {
                    warnings.push(format!(
                        "[L006] {path}:{}: Empty `catch` block silently swallows errors.",
                        span.line
                    ));
                }
                collect_lint_ast_warnings(path, try_body, warnings);
                collect_lint_ast_warnings(path, catch_body, warnings);
            }
            Stmt::FnDef { params, body, .. } => {
                let mut fn_used = HashSet::new();
                for inner in body {
                    collect_stmt_reads(inner, &mut fn_used);
                }
                for p in params {
                    if !p.name.starts_with('_') && !fn_used.contains(&p.name) {
                        warnings.push(format!(
                            "[L005] {path}:{}: Parameter '{}' is never used in function body.",
                            p.span.line, p.name
                        ));
                    }
                }
                collect_lint_ast_warnings(path, body, warnings);
            }
            Stmt::If {
                then_branch,
                otherwise_branch,
                ..
            } => {
                collect_lint_ast_warnings(path, then_branch, warnings);
                if let Some(el) = otherwise_branch {
                    collect_lint_ast_warnings(path, el, warnings);
                }
            }
            Stmt::Match {
                cases,
                otherwise_branch,
                ..
            } => {
                for c in cases {
                    collect_lint_ast_warnings(path, &c.body, warnings);
                }
                if let Some(el) = otherwise_branch {
                    collect_lint_ast_warnings(path, el, warnings);
                }
            }
            Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::For { body, .. } => {
                collect_lint_ast_warnings(path, body, warnings);
            }
            _ => {}
        }
    }
}

fn collect_stmt_reads(stmt: &Stmt, used: &mut HashSet<String>) {
    match stmt {
        Stmt::Binding { value, .. } | Stmt::Assign { value, .. } => collect_expr_reads(value, used),
        Stmt::CompoundAssign { name, value, .. } => {
            used.insert(name.clone());
            collect_expr_reads(value, used);
        }
        Stmt::IndexAssign {
            target,
            index,
            value,
            ..
        } => {
            used.insert(target.clone());
            collect_expr_reads(index, used);
            collect_expr_reads(value, used);
        }
        Stmt::FieldAssign { target, value, .. } => {
            used.insert(target.clone());
            collect_expr_reads(value, used);
        }
        Stmt::If {
            condition,
            then_branch,
            otherwise_branch,
            ..
        } => {
            collect_expr_reads(condition, used);
            for s in then_branch {
                collect_stmt_reads(s, used);
            }
            if let Some(el) = otherwise_branch {
                for s in el {
                    collect_stmt_reads(s, used);
                }
            }
        }
        Stmt::Match {
            subject,
            cases,
            otherwise_branch,
            ..
        } => {
            collect_expr_reads(subject, used);
            for c in cases {
                collect_expr_reads(&c.pattern, used);
                for s in &c.body {
                    collect_stmt_reads(s, used);
                }
            }
            if let Some(el) = otherwise_branch {
                for s in el {
                    collect_stmt_reads(s, used);
                }
            }
        }
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => {
            for s in try_body {
                collect_stmt_reads(s, used);
            }
            for s in catch_body {
                collect_stmt_reads(s, used);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            collect_expr_reads(condition, used);
            for s in body {
                collect_stmt_reads(s, used);
            }
        }
        Stmt::Repeat { count, body, .. } => {
            collect_expr_reads(count, used);
            for s in body {
                collect_stmt_reads(s, used);
            }
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_reads(iterable, used);
            for s in body {
                collect_stmt_reads(s, used);
            }
        }
        Stmt::FnDef { body, .. } => {
            for s in body {
                collect_stmt_reads(s, used);
            }
        }
        Stmt::Return { value: Some(v), .. } | Stmt::ExprStmt { expr: v, .. } => {
            collect_expr_reads(v, used);
        }
        Stmt::Use { .. }
        | Stmt::StructDef { .. }
        | Stmt::EnumDef { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => {}
    }
}

fn collect_expr_reads(expr: &Expr, used: &mut HashSet<String>) {
    match expr {
        Expr::Identifier { name, .. } => {
            used.insert(name.clone());
        }
        Expr::InterpString { parts, .. } => {
            for p in parts {
                if let InterpPart::Expr(e) = p {
                    collect_expr_reads(e, used);
                }
            }
        }
        Expr::ListLit { elements, .. } => {
            for e in elements {
                collect_expr_reads(e, used);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_expr_reads(k, used);
                collect_expr_reads(v, used);
            }
        }
        Expr::StructInit { fields, .. } => {
            for (_, v) in fields {
                collect_expr_reads(v, used);
            }
        }
        Expr::FieldAccess { target, .. } => collect_expr_reads(target, used),
        Expr::Index { target, index, .. } => {
            collect_expr_reads(target, used);
            collect_expr_reads(index, used);
        }
        Expr::Unary { expr: inner, .. } => collect_expr_reads(inner, used),
        Expr::Binary { left, right, .. } => {
            collect_expr_reads(left, used);
            collect_expr_reads(right, used);
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_expr_reads(a, used);
            }
        }
        Expr::StringLit { .. }
        | Expr::NumberLit { .. }
        | Expr::DecimalLit { .. }
        | Expr::BoolLit { .. } => {}
    }
}

struct SilentBenchIo;

impl InterpreterIo for SilentBenchIo {
    fn write_stdout(&mut self, _text: &str) {}
    fn writeln_stdout(&mut self, _text: &str) {}
    fn read_line(&mut self) -> Result<String, String> {
        Ok(String::new())
    }
}

pub fn handle_bench(
    path: &str,
    runs: usize,
    messenger: &DogMessenger,
    emit_progress: bool,
) -> Result<String, String> {
    let iters = runs.clamp(1, 5_000);
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    let program = resolve_program(path, &source).map_err(|d| format!("{d}"))?;
    let mut checker = TypeChecker::new();
    checker.check(&program).map_err(|d| format!("{d}"))?;

    let mut samples_us = Vec::with_capacity(iters);
    let mut bar = ProgressBar::new(
        format!("Bench {path}"),
        iters as u64,
        ProgressUnit::Items,
        messenger.clone(),
    );

    let total_start = Instant::now();
    for i in 0..iters {
        let mut silent_io = SilentBenchIo;
        let mut interp = Interpreter::new(&mut silent_io);
        let t0 = Instant::now();
        interp.eval_program(&program).map_err(|d| format!("{d}"))?;
        samples_us.push(t0.elapsed().as_micros());
        if emit_progress && (i == 0 || (i + 1) % (iters / 10).max(1) == 0 || i + 1 == iters) {
            bar.update_stdout((i + 1) as u64);
        }
    }
    if emit_progress {
        bar.finish_stdout(Some("Benchmark complete"));
    }

    let total_us = total_start.elapsed().as_micros();
    samples_us.sort_unstable();
    let min_us = samples_us[0];
    let max_us = samples_us[samples_us.len() - 1];
    let sum_us: u128 = samples_us.iter().sum();
    let avg_us = sum_us as f64 / samples_us.len() as f64;
    let p50_us = samples_us[samples_us.len() / 2];
    let p95_idx = ((samples_us.len() * 95) / 100).min(samples_us.len() - 1);
    let p95_us = samples_us[p95_idx];
    let ops_sec = if total_us > 0 {
        (iters as f64) * 1_000_000.0 / (total_us as f64)
    } else {
        iters as f64 * 1_000_000.0
    };

    Ok(format!(
        "DOG Benchmark: {path} ({iters} runs)\n\
         ------------------------------------\n\
         Min     : {min_us} us\n\
         Median  : {p50_us} us\n\
         Avg     : {avg_us:.2} us\n\
         P95     : {p95_us} us\n\
         Max     : {max_us} us\n\
         Total   : {total_us} us\n\
         Rate    : {ops_sec:.1} runs/sec"
    ))
}

pub fn handle_forge(args: &[String]) -> Result<String, String> {
    if args.first().map(String::as_str) != Some("new") {
        return Err("Usage: dog forge new <project-name | .> [--web | --bot]".to_string());
    }
    let mut target: Option<&str> = None;
    let mut template = "default";
    for arg in &args[1..] {
        match arg.as_str() {
            "--web" | "-w" => template = "web",
            "--bot" | "--discord" | "-b" => template = "bot",
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "Unknown template option '{flag}'. Use --web or --bot."
                ));
            }
            val if target.is_none() => target = Some(val),
            extra => {
                return Err(format!(
                    "Unexpected argument '{extra}'. Usage: dog forge new <project-name | .> [--web | --bot]"
                ));
            }
        }
    }

    let Some(name) = target else {
        return Err("Usage: dog forge new <project-name | .> [--web | --bot]".to_string());
    };
    if name.is_empty() || name.contains('\\') || name == ".." {
        return Err(format!("Invalid project directory name '{name}'."));
    }

    let in_place = name == ".";
    let root = Path::new(name);
    if !in_place && root.exists() {
        return Err(format!("Directory '{name}' already exists."));
    }
    if in_place && (root.join("dog.pack").exists() || root.join("src/main.dg").exists()) {
        return Err("Current directory already contains dog.pack or src/main.dg.".to_string());
    }

    let pkg_name = if in_place {
        env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "dog_app".to_string())
    } else {
        Path::new(name)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| name.to_string())
    };

    fs::create_dir_all(root.join("src"))
        .map_err(|e| format!("Failed to create '{name}/src': {e}"))?;
    fs::create_dir_all(root.join("tests"))
        .map_err(|e| format!("Failed to create '{name}/tests': {e}"))?;

    let mut pack = DogPack {
        name: pkg_name.clone(),
        version: "0.1.0".to_string(),
        entry: "src/main.dg".to_string(),
        description: match template {
            "bot" => format!("{pkg_name} Discord bot built with DOG"),
            "web" => format!("{pkg_name} local web server built with DOG"),
            _ => format!("{pkg_name} built with DOG"),
        },
        ..DogPack::default()
    };
    pack.scripts
        .insert("start".to_string(), "dog run src/main.dg".to_string());
    pack.scripts
        .insert("test".to_string(), "dog test tests".to_string());

    let (main_src, readme_src) = match template {
        "bot" => (
            include_str!("../../examples/discord_bot.dg").to_string(),
            format!(
                "# {pkg_name} (DOG Discord Bot)\n\n```bash\nexport DISCORD_WEBHOOK_URL=\"https://discord.com/api/webhooks/...\"\ndog run src/main.dg\n# Trigger locally:\ncurl -X POST http://127.0.0.1:8090/cmd -d '!ping'\n```\n"
            ),
        ),
        "web" => (
            include_str!("../../examples/web_server.dg").to_string(),
            format!(
                "# {pkg_name} (DOG Web Server)\n\n```bash\ndog run src/main.dg\n# Open http://127.0.0.1:8080\n```\n"
            ),
        ),
        _ => (
            format!("let app_name = \"{pkg_name}\"\nsay(\"Welcome to \" + app_name + \"!\")\n"),
            format!("# {pkg_name}\n\n```bash\ndog run src/main.dg\ndog test tests\n```\n"),
        ),
    };

    fs::write(root.join("dog.pack"), pack.render())
        .map_err(|e| format!("Failed to write dog.pack: {e}"))?;
    fs::write(root.join("src/main.dg"), main_src)
        .map_err(|e| format!("Failed to write src/main.dg: {e}"))?;
    fs::write(
        root.join("tests/main_test.dg"),
        "fn add(a: number, b: number) -> number\n    return a + b\nend\n\nassert(add(2, 3) == 5, \"math works\")\n",
    )
    .map_err(|e| format!("Failed to write tests/main_test.dg: {e}"))?;
    fs::write(root.join("README.md"), readme_src)
        .map_err(|e| format!("Failed to write README.md: {e}"))?;
    fs::write(root.join(".gitignore"), "*.dgb\n*.json\n")
        .map_err(|e| format!("Failed to write .gitignore: {e}"))?;

    let prefix = if in_place { "." } else { name };
    Ok(format!(
        "Forged new DOG project '{pkg_name}' ({template} template):\n  {prefix}/dog.pack\n  {prefix}/src/main.dg\n  {prefix}/tests/main_test.dg\n  {prefix}/README.md"
    ))
}

pub fn handle_server(args: &[String]) -> Result<(), String> {
    let mut dir: Option<String> = None;
    let mut port: u16 = 8080;
    let mut host = String::from("127.0.0.1");
    let mut port_set = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--host" | "--bind" | "-H" => {
                if let Some(h) = args.get(i + 1) {
                    host = h.clone();
                    i += 2;
                    continue;
                }
                return Err("Missing IP/hostname after --host".to_string());
            }
            "--port" | "-p" => {
                if let Some(p) = args.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                    port = p;
                    port_set = true;
                    i += 2;
                    continue;
                }
                return Err("Invalid port number after --port".to_string());
            }
            val if !val.starts_with('-') => {
                if dir.is_none() {
                    if let Ok(p) = val.parse::<u16>() {
                        if !port_set && !Path::new(val).exists() {
                            port = p;
                            port_set = true;
                            i += 1;
                            continue;
                        }
                    }
                    dir = Some(val.to_string());
                } else if !port_set {
                    if let Ok(p) = val.parse::<u16>() {
                        port = p;
                        port_set = true;
                    }
                }
            }
            unknown => {
                return Err(format!(
                    "Unknown option '{unknown}'. Usage: dog server [dir] [port] [--host 0.0.0.0]"
                ));
            }
        }
        i += 1;
    }

    let dir_str = dir.unwrap_or_else(|| ".".to_string());
    let root = Arc::new(
        fs::canonicalize(&dir_str)
            .map_err(|e| format!("Failed to resolve directory '{dir_str}': {e}"))?,
    );
    let addr = format!("{host}:{port}");
    let listener =
        TcpListener::bind(&addr).map_err(|e| format!("Failed to bind {addr}: {e}"))?;

    println!(
        "DOG multi-threaded HTTP server listening on http://{addr} (root: {})",
        root.display()
    );
    if host == "0.0.0.0" {
        println!("LAN access enabled: reachable from devices on your local network.");
    }
    println!("Press Ctrl+C to stop.");

    for stream_res in listener.incoming() {
        let Ok(mut stream) = stream_res else {
            continue;
        };
        let root_ref = Arc::clone(&root);
        thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
            let mut buf = [0u8; 4096];
            let Ok(n) = stream.read(&mut buf) else {
                return;
            };
            if n == 0 {
                return;
            }
            let req = String::from_utf8_lossy(&buf[..n]);
            let req_line = req.lines().next().unwrap_or("");
            let mut parts = req_line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let raw_target = parts.next().unwrap_or("/");
            let is_head = method == "HEAD";

            if method != "GET" && !is_head {
                let body = b"405 Method Not Allowed\n";
                let _ = write_http_response(
                    &mut stream,
                    "405 Method Not Allowed",
                    "text/plain",
                    body,
                    false,
                );
                return;
            }

            let clean_path = raw_target.split('?').next().unwrap_or("/");
            if let Some(target_file) = resolve_safe_path(&root_ref, clean_path) {
                if target_file.is_dir() {
                    let index = target_file.join("index.html");
                    if index.is_file() {
                        if let Ok(data) = fs::read(&index) {
                            let _ = write_http_response(
                                &mut stream,
                                "200 OK",
                                "text/html; charset=utf-8",
                                &data,
                                is_head,
                            );
                            return;
                        }
                    }
                    let listing = build_dir_listing(&target_file, clean_path);
                    let _ = write_http_response(
                        &mut stream,
                        "200 OK",
                        "text/html; charset=utf-8",
                        listing.as_bytes(),
                        is_head,
                    );
                } else if target_file.is_file() {
                    match fs::read(&target_file) {
                        Ok(data) => {
                            let ctype = guess_mime_type(&target_file);
                            let _ = write_http_response(&mut stream, "200 OK", ctype, &data, is_head);
                        }
                        Err(_) => {
                            let _ = write_http_response(
                                &mut stream,
                                "500 Internal Server Error",
                                "text/plain",
                                b"Read Error\n",
                                is_head,
                            );
                        }
                    }
                } else {
                    let _ = write_http_response(
                        &mut stream,
                        "404 Not Found",
                        "text/plain",
                        b"404 Not Found\n",
                        is_head,
                    );
                }
            } else {
                let _ = write_http_response(
                    &mut stream,
                    "403 Forbidden",
                    "text/plain",
                    b"403 Forbidden\n",
                    is_head,
                );
            }
        });
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

fn write_http_response(
    stream: &mut std::net::TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    Ok(())
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
    let is_termux = env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    let mut msg = format!("Current DOG CLI version: {version}\n");
    if is_termux {
        msg.push_str(
            "Termux APT update : pkg update && pkg upgrade -y dog\n\
             Direct DOG upgrade (bypasses syncing Termux mirrors):\n\
               apt-get update -o Dir::Etc::sourcelist=\"sources.list.d/dog.list\" -o Dir::Etc::sourceparts=\"-\" -o APT::Get::List-Cleanup=\"0\" && apt-get install --only-upgrade -y dog\n",
        );
    }
    if Path::new(".git").exists() {
        if let Ok(out) = Command::new("git").args(["status", "-sb"]).output() {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            msg.push_str(&format!("Local git checkout : {branch}\n"));
        }
        msg.push_str("To update from source: git pull && cargo install --path .");
    } else if !is_termux {
        msg.push_str("Update from source: `git pull && cargo install --path .`");
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
