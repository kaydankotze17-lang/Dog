use crate::ast::{BinOp, Expr, InterpPart, Program, Stmt};
use crate::checker::TypeChecker;
use crate::config::{DogConfig, Personality};
use crate::editor::BufferRunIo;
use crate::formatter::format_source;
use crate::hub::DogPack;
use crate::interpreter::{
    check_source, resolve_program, run_source_with_io, stmt_span, DebugStepDecision,
    ExecObserver, Interpreter,
};
use crate::lexer::Lexer;
use crate::net::guess_mime_type;
use crate::parser::Parser;
use crate::runtime::Environment;
use crate::sdu::{audit_log_path, detect_actual_root, session_path};
use crate::stdlib::{InterpreterIo, StdIo};
use crate::term::{DogMessenger, ProgressBar, ProgressUnit};
use crate::types::DogType;
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::net::TcpListener;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

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
        Stmt::StyleDef { rules, variants, .. } => {
            format!("style ({} rules, {} variants)", rules.len(), variants.len())
        }
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
    let raw_topic = topic.map(str::trim).unwrap_or("overview");
    let lower_topic = raw_topic.to_ascii_lowercase();
    if lower_topic == "std" {
        return crate::stdlib::modules::render_std_docs(None);
    }
    if lower_topic.starts_with("std.") {
        if crate::stdlib::modules::is_std_module(&lower_topic) {
            return crate::stdlib::modules::render_std_docs(Some(&lower_topic));
        }
        if let Some((mod_name, fn_name)) = lower_topic.rsplit_once('.') {
            if let Ok(canon) = crate::stdlib::modules::resolve_std_call(mod_name, fn_name) {
                let sig = crate::editor::builtin_signature_help(canon).unwrap_or("");
                return format!(
                    "DOG Standard Library Member: {lower_topic}\n\
                     ------------------------------------------\n\
                     Resolves to: {canon}()\n\
                     {sig}"
                );
            }
        }
        return crate::stdlib::modules::render_std_docs(None);
    }

    match lower_topic.as_str() {
        "syntax" | "english" | "cheat" => {
            "DOG Syntax & English-Style Reference\n\
             ------------------------------------\n\
             Variables  : let x = 10              (fixed) | var y = 0 (mutable)\n\
                          set y to 5              (English assign) | increase y by 2 | decrease y by 1\n\
             Output     : say(\"Hi!\") or print(\"Hi!\") | $\"Hi {name}, {x + 1}!\"\n\
             Compare    : == / != / > / < / >= / <=\n\
                          is / is not / equals / is equal to / is greater than / is less than\n\
                          is at least / is at most / is over / is under\n\
             Logic      : and / or / not\n\
             Branching  : if cond then ... elif ... else ... end   (or otherwise if / otherwise)\n\
                          unless cond then ... else ... end        (runs when cond is false)\n\
             Pattern    : match x case 1 ... else ... end          (or otherwise)\n\
             Errors     : try ... catch err ... end\n\
             Loops      : while cond ... end  | until cond ... end\n\
                          repeat 3 times ... end | for each item in items ... end\n\
                          for i from 1 to 5 ... end  (inclusive 1..5)\n\
             Functions  : fn add(a: number, b: number = 1) -> number ... end\n\
                          define greet(name: text) returns text ... end\n\
             Types      : struct Dog ... end | enum Role ... end\n\
             Modules    : use \"std.io\" | use \"module.dg\"\n\n\
             Tip: Run `dog learn` for guided step-by-step interactive lessons."
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
            "DOG Standard Library (v1.0.0 — Modular `std.*` + Global Built-ins)\n\
             -----------------------------------------------------------------\n\
             Modules : `dog docs std` for std.io, std.text, std.math, std.files, std.net,\n\
                       std.system, std.data, std.crypto, std.cli, std.collections\n\
             I/O     : say(), answer(), ask(), confirm(), choose(), password(), read_key(),\n\
                       clear(), progress(), table(), log_info(), log_warn(), log_error()\n\
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
             System  : args(), env_get(), env_set(), env_load(), exec_cmd(), os_name(),\n\
                       os_arch(), time_now_ms(), time_utc(), time_format(), sleep_ms(),\n\
                       uuid_v4()\n\
             Interop : json_parse(), json_stringify(), json_pretty(), json_has_key(),\n\
                       json_read(), json_write(), toml_parse(), csv_parse(),\n\
                       csv_stringify(), url_encode(), url_decode(), url_parse(),\n\
                       query_parse(), query_stringify(), cookie_parse()\n\
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
             Web/CSS : dog_css(), css_rule(), css_vars(), css_media(), css_keyframes(),\n\
                       css_theme(), css_bundle(), html_el(), html_page(), web_nav(), web_card(),\n\
                       web_grid(), web_button(), web_input(), web_title(), web_route(),\n\
                       web_navigate(), web_storage_get(), web_storage_set(), dom_mount(),\n\
                       dom_set_text(), dom_set_html(), dom_get_value(), dom_set_value(),\n\
                       dom_add_class(), dom_remove_class(), dom_toggle_class(), dom_set_style(),\n\
                       dom_on()\n\
             Errors  : assert(), fail()"
                .to_string()
        }
        "network" | "net" | "web" | "css" | "dgss" | "bot" => {
            "DOG Web Development, DOG CSS (.dgss), HTTP & WebSockets\n\
             --------------------------------------------------------\n\
             Client Web  : dog web build app.dg [--css style.dgss] [--out dist_web]\n\
                           dog web css style.dgss [--out style.css] | dog web serve app.dg [port]\n\
                           std.web DOM APIs: dom_mount, dom_set_text, dom_set_html, dom_get_value,\n\
                           dom_set_value, dom_add_class, dom_remove_class, dom_toggle_class,\n\
                           dom_set_style, dom_on, web_title, web_storage_set/get, web_route/navigate\n\
                           Components: html_el, html_page, web_nav, web_card, web_grid, web_button, web_input\n\
             DOG CSS     : vars ... end | rule .card ... end | on hover ... end | on mobile ... end | animate\n\
                           Shorthands: bg, ink, pad, px, py, m, mx, my, radius, cols, gap, size, shadow\n\
                           Functions : dog_css, css_rule, css_vars, css_media, css_keyframes, css_theme, css_bundle\n\
             HTTP Server : let srv = http_listen(\"127.0.0.1:8080\")  (\"0.0.0.0:8080\" for LAN)\n\
                           http_accept(srv) / http_try_accept(srv, timeout_ms)\n\
                           http_reply(req, 200, \"text/html\", html) | http_status_text(200)\n\
                           http_serve_file(req, \"public/index.html\") | http_route_match(\"/u/:id\", p)\n\
             HTTP Client : http_get(url), http_post(url, body, ctype), http_put(...), http_delete(url)\n\
                           http_request(method, url, hdrs, body) / http_request_timeout(..., ms)\n\
             WebSockets  : ws_upgrade(req) -> ws_id, ws_connect(\"ws://...\") -> ws_id\n\
                           ws_send(ws_id, msg), ws_recv(ws_id), ws_ping(ws_id, msg), ws_close(ws_id)\n\
             UDP & DNS   : udp_bind, udp_send_to, udp_recv_from, udp_close, dns_lookup, local_ip\n\
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
               std, syntax, english, types, stdlib, cli, network, errors, editor, prompt, hub, sdu\n\
             New to DOG? Run `dog learn` for step-by-step beginner lessons.\n\
             Inspect a std module: `dog docs std.io`, `dog docs std.net`, `dog docs std.data`\n\
             Or pass any built-in function name (e.g. `dog docs print`, `dog docs ask`)."
                .to_string()
        }
    }
}

pub fn handle_learn(
    topic: Option<&str>,
    run_demo: bool,
    personality: Personality,
) -> Result<String, String> {
    let key = topic.map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    if key.is_empty() || key == "list" || key == "all" {
        return Ok(
            "DOG Beginner Guide (`dog learn`)\n\
             -------------------------------\n\
             DOG is designed to read clearly like English while keeping strict types.\n\n\
             Lessons:\n\
               1. dog learn basics      -- print/say, let/var, and $\"hello {name}\" text\n\
               2. dog learn english     -- is/is not, unless, until, repeat N times, for..from..to\n\
               3. dog learn input       -- ask(), confirm(), choose(), number()\n\
               4. dog learn functions   -- define / fn, defaults, and returns\n\
               5. dog learn data        -- lists, maps, structs, enums, and match\n\
               6. dog learn errors      -- strict typing and try ... catch err ... end\n\
               7. dog learn web         -- build interactive websites & DOG CSS (.dgss)\n\n\
             Add `--run` to execute the lesson example live:\n\
               dog learn english --run\n\
             Quick cheat sheet: `dog docs english`"
                .to_string(),
        );
    }

    let (title, explanation, code, runnable_code) = match key.as_str() {
        "basics" | "1" => (
            "Lesson 1: Basics (Output & Variables)",
            "Use `print(...)` or `say(...)` to show output, `let` for fixed values,\n\
             `var` for values that change, and `$\"...\"` to place values inside text.",
            "let pet = \"Buddy\"\n\
             var treats = 2\n\
             increase treats by 1\n\
             print($\"{pet} earned {treats} treats!\")",
            "let pet = \"Buddy\"\nvar treats = 2\nincrease treats by 1\nprint($\"{pet} earned {treats} treats!\")",
        ),
        "english" | "2" => (
            "Lesson 2: Natural English Control Flow",
            "Write conditions and loops almost like plain English sentences:\n\
             `is`, `is not`, `is at least`, `then`, `else`, `unless`, `until`,\n\
             `repeat N times`, `for n from 1 to 5`, `set x to ...`, `increase x by ...`.",
            "var energy = 1\n\
             until energy is at least 3\n\
                 print($\"Charging... level {energy}\")\n\
                 increase energy by 1\n\
             end\n\
             unless energy is less than 3 then\n\
                 print(\"Ready to roll!\")\n\
             end\n\
             for step from 1 to 3\n\
                 print($\"Step #{step}\")\n\
             end",
            "var energy = 1\nuntil energy is at least 3\n    print($\"Charging... level {energy}\")\n    increase energy by 1\nend\nunless energy is less than 3 then\n    print(\"Ready to roll!\")\nend\nfor step from 1 to 3\n    print($\"Step #{step}\")\nend",
        ),
        "input" | "3" => (
            "Lesson 3: Talking to the User",
            "Ask questions interactively with `ask()`, `confirm()`, and `choose()`.\n\
             Convert text answers to numbers explicitly with `number(...)`.",
            "let name = ask(\"Your name? \")\n\
             let ready = confirm(\"Start adventure?\")\n\
             if ready is true then\n\
                 let mode = choose(\"Difficulty\", [\"Easy\", \"Normal\", \"Hard\"])\n\
                 print($\"Good luck, {name} on {mode}!\")\n\
             else\n\
                 print(\"See you next time!\")\n\
             end",
            "let name = \"Alex\"\nlet ready = true\nif ready is true then\n    print($\"Good luck, {name}!\")\nelse\n    print(\"See you next time!\")\nend",
        ),
        "functions" | "4" => (
            "Lesson 4: Writing Reusable Functions",
            "Declare functions with `define` (or `fn`), optional parameter defaults,\n\
             and `returns <Type>` (or `-> <Type>`).",
            "define greet(name: text, greeting: text = \"Hello\") returns text\n\
                 return $\"{greeting}, {name}!\"\n\
             end\n\
             \n\
             print(greet(\"DOG\"))\n\
             print(greet(\"Termux\", \"Welcome to\"))",
            "define greet(name: text, greeting: text = \"Hello\") returns text\n    return $\"{greeting}, {name}!\"\nend\nprint(greet(\"DOG\"))\nprint(greet(\"Termux\", \"Welcome to\"))",
        ),
        "data" | "5" => (
            "Lesson 5: Lists, Maps, Structs & Match",
            "Organize structured data with lists, maps, structs, enums, and `match`.",
            "let fruits = [\"apple\", \"mango\", \"pear\"]\n\
             for each fruit in fruits\n\
                 print($\"Fruit: {fruit}\")\n\
             end\n\
             \n\
             let rank = 2\n\
             match rank\n\
                 case 1\n\
                     print(\"Gold medal\")\n\
                 else\n\
                     print(\"Keep climbing!\")\n\
             end",
            "let fruits = [\"apple\", \"mango\", \"pear\"]\nfor each fruit in fruits\n    print($\"Fruit: {fruit}\")\nend\nlet rank = 2\nmatch rank\n    case 1\n        print(\"Gold medal\")\n    else\n        print(\"Keep climbing!\")\nend",
        ),
        "errors" | "6" => (
            "Lesson 6: Safety & Error Handling",
            "DOG prevents silent text+number bugs at check time. For runtime input\n\
             like parsing user text, wrap risky code in `try ... catch err ... end`.",
            "try\n\
                 let parsed = number(\"not-a-number\")\n\
                 print(parsed)\n\
             catch err\n\
                 print($\"Handled safely: {err}\")\n\
             end",
            "try\n    let parsed = number(\"not-a-number\")\n    print(parsed)\ncatch err\n    print(\"Handled safely!\")\nend",
        ),
        "web" | "css" | "7" => (
            "Lesson 7: Websites & DOG CSS (.dgss)",
            "Build interactive browser websites in DOG (`std.web` + `dom_on`)\n\
             and style them using DOG CSS (`dog_css` or `.dgss` files).",
            "let css = dog_css(\"vars\\n  accent: #C85A11\\nend\\nrule .card\\n  bg: #FFF\\n  pad: 20px\\n  radius: 10px\\nend\")\n\
             let card = web_card(\"Hello DOG Web\", web_button(\"go\", \"Click Me\"))\n\
             dom_mount(\"#app\", card)\n\
             print(html_page(\"My DOG Site\", css, card))",
            "let css = dog_css(\"vars\\n  accent: #C85A11\\nend\\nrule .card\\n  bg: #FFF\\n  pad: 20px\\n  radius: 10px\\nend\")\nlet card = web_card(\"Hello DOG Web\", web_button(\"go\", \"Click Me\"))\ndom_mount(\"#app\", card)\nprint(css)",
        ),
        other => {
            return Err(format!(
                "Unknown lesson '{other}'. Run `dog learn` to see available lessons (basics, english, input, functions, data, errors, web)."
            ));
        }
    };

    let indented_code: String = code
        .lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut out = format!(
        "DOG {title}\n{}\n{explanation}\n\nExample:\n{indented_code}\n",
        "-".repeat(title.len() + 4)
    );

    if run_demo {
        let output = eval_snippet_captured(runnable_code, personality)?;
        out.push_str("\nLive Output:\n");
        for line in output.lines() {
            out.push_str(&format!("  > {line}\n"));
        }
    } else {
        out.push_str(&format!("\nTry it live: `dog learn {key} --run`"));
    }

    Ok(out)
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
            let is_say =
                matches!(&expr, Expr::Call { callee, .. } if callee == "say" || callee == "print");
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
        Stmt::FnDef { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    collect_expr_reads(d, used);
                }
            }
            for s in body {
                collect_stmt_reads(s, used);
            }
        }
        Stmt::StyleDef {
            selector,
            rules,
            variants,
            ..
        } => {
            collect_expr_reads(selector, used);
            for r in rules {
                collect_expr_reads(&r.value, used);
            }
            for v in variants {
                for r in &v.rules {
                    collect_expr_reads(&r.value, used);
                }
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
        Expr::Call { args, .. } | Expr::NamespaceCall { args, .. } => {
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

const GITHUB_CARGO_URL: &str =
    "https://raw.githubusercontent.com/kaydankotze17-lang/Dog/main/Cargo.toml";
const TERMUX_APT_BASE_URL: &str = "https://kaydankotze17-lang.github.io/Dog";

fn parse_cargo_toml_version(raw: &str) -> Option<String> {
    let mut in_package = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && trimmed.starts_with("version") {
            if let Some((_, rhs)) = trimmed.split_once('=') {
                let cleaned = rhs.trim().trim_matches('"').trim_matches('\'');
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    None
}

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/kaydankotze17-lang/Dog/releases/latest";

#[derive(Default)]
struct RemoteVersionMeta {
    version: Option<String>,
    deb_version: Option<String>,
    sha256_by_arch: std::collections::BTreeMap<String, String>,
}

#[allow(dead_code)]
fn parse_version_json(raw: &str) -> (Option<String>, Option<String>) {
    let meta = parse_version_metadata(raw);
    (meta.version, meta.deb_version)
}

fn parse_version_metadata(raw: &str) -> RemoteVersionMeta {
    let mut meta = RemoteVersionMeta::default();
    if let Ok(crate::json::JsonValue::Object(map)) = crate::json::parse_json(raw) {
        if let Some(crate::json::JsonValue::Str(s)) = map.get("version") {
            meta.version = Some(s.clone());
        }
        if let Some(crate::json::JsonValue::Str(s)) = map.get("deb_version") {
            meta.deb_version = Some(s.clone());
        }
        if let Some(crate::json::JsonValue::Object(hashes)) = map.get("sha256") {
            for (k, v) in hashes {
                if let crate::json::JsonValue::Str(hex) = v {
                    meta.sha256_by_arch
                        .insert(k.clone(), hex.trim().to_ascii_lowercase());
                }
            }
        }
    }
    meta
}

fn parse_semver_tuple(v: &str) -> (u64, u64, u64) {
    let core = v
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or("0.0.0");
    let mut parts = core
        .split('.')
        .map(|s| s.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    parse_semver_tuple(a).cmp(&parse_semver_tuple(b))
}

fn fetch_remote_text(url: &str) -> Result<String, String> {
    let resp = crate::net::http_client_request_with_timeout("GET", url, &[], "", 8_000)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(resp.body)
    } else {
        Err(format!("HTTP {} from {url}", resp.status))
    }
}

fn detect_termux_prefix() -> Option<String> {
    if let Ok(prefix) = env::var("PREFIX") {
        if prefix.contains("com.termux") {
            return Some(prefix);
        }
    }
    if Path::new("/data/data/com.termux/files/usr").is_dir() {
        return Some("/data/data/com.termux/files/usr".to_string());
    }
    None
}

fn detect_termux_arch() -> &'static str {
    if let Ok(out) = Command::new("dpkg").arg("--print-architecture").output() {
        if out.status.success() {
            let arch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return match arch.as_str() {
                "aarch64" | "arm64" => "aarch64",
                "arm" | "armhf" | "armv7l" => "arm",
                "x86_64" | "amd64" => "x86_64",
                _ => "aarch64",
            };
        }
    }
    match env::consts::ARCH {
        "aarch64" => "aarch64",
        "arm" => "arm",
        "x86_64" => "x86_64",
        _ => "aarch64",
    }
}

fn read_installed_dog_version() -> Option<String> {
    let out = Command::new("dog").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().last().map(|s| s.to_string())
}

fn format_command_failure(label: &str, out: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    format!("{label} failed (exit {:?}): {detail}", out.status.code())
}

fn ensure_termux_apt_source(prefix: &str) -> Result<(), String> {
    let list_dir = Path::new(prefix).join("etc").join("apt").join("sources.list.d");
    fs::create_dir_all(&list_dir)
        .map_err(|e| format!("Failed creating '{}': {e}", list_dir.display()))?;
    let list_file = list_dir.join("dog.list");
    let expected = format!("deb [trusted=yes] {TERMUX_APT_BASE_URL} termux main\n");
    let current = fs::read_to_string(&list_file).unwrap_or_default();
    if current != expected {
        fs::write(&list_file, expected)
            .map_err(|e| format!("Failed writing '{}': {e}", list_file.display()))?;
    }
    Ok(())
}

fn is_html_payload(bytes: &[u8]) -> bool {
    let sample_len = bytes.len().min(512);
    let head = String::from_utf8_lossy(&bytes[..sample_len]).to_ascii_lowercase();
    head.contains("<!doctype html") || head.contains("<html") || head.contains("<title>404")
}

fn verify_deb_bytes(bytes: &[u8], expected_sha: Option<&str>) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("Downloaded .deb payload is empty.".to_string());
    }
    if is_html_payload(bytes) {
        return Err(
            "Received HTML error page instead of a .deb package (404 / mirror error)."
                .to_string(),
        );
    }
    if bytes.len() < 8 || &bytes[..8] != b"!<arch>\n" {
        return Err("Invalid .deb archive header (expected `!<arch>\\n`).".to_string());
    }
    if let Some(expected) = expected_sha {
        let actual = crate::crypto::sha256_hex(bytes);
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(format!(
                "SHA-256 verification failed for downloaded .deb (expected {}, got {actual}).",
                expected.trim()
            ));
        }
    }
    Ok(())
}

fn download_url_bytes(url: &str, tmp_path: &Path) -> Result<Vec<u8>, String> {
    if let Ok(b) = crate::net::http_download_bytes(url, |_, _| {}) {
        if !b.is_empty() && !is_html_payload(&b) {
            return Ok(b);
        }
    }
    let curl_out = Command::new("curl")
        .args([
            "--retry",
            "3",
            "--retry-delay",
            "1",
            "--connect-timeout",
            "10",
            "--max-time",
            "60",
            "-fsSL",
            "-H",
            "Cache-Control: no-cache",
            url,
            "-o",
        ])
        .arg(tmp_path)
        .output();
    if let Ok(out) = curl_out {
        if out.status.success() {
            if let Ok(b) = fs::read(tmp_path) {
                if !b.is_empty() {
                    return Ok(b);
                }
            }
        }
    }
    let wget_out = Command::new("wget")
        .args(["-q", "--timeout=30", "--tries=3", "--no-cache", "-O"])
        .arg(tmp_path)
        .arg(url)
        .output();
    if let Ok(out) = wget_out {
        if out.status.success() {
            if let Ok(b) = fs::read(tmp_path) {
                if !b.is_empty() {
                    return Ok(b);
                }
            }
        }
    }
    Err(format!("Failed downloading package from {url}"))
}

fn fetch_github_release_deb_url(arch: &str, epoch: u64) -> Option<String> {
    let api_url = format!("{GITHUB_RELEASES_API}?t={epoch}");
    let body = fetch_remote_text(&api_url).ok()?;
    let crate::json::JsonValue::Object(root) = crate::json::parse_json(&body).ok()? else {
        return None;
    };
    let crate::json::JsonValue::Array(assets) = root.get("assets")? else {
        return None;
    };
    let suffix = format!("_{arch}.deb");
    for item in assets {
        if let crate::json::JsonValue::Object(asset) = item {
            if let Some(crate::json::JsonValue::Str(url)) = asset.get("browser_download_url") {
                if url.ends_with(&suffix) {
                    return Some(url.clone());
                }
            }
        }
    }
    None
}

fn purge_stale_dog_apt_caches(prefix: &str) {
    let lists_dir = Path::new(prefix).join("var").join("lib").join("apt").join("lists");
    for dir in [lists_dir.clone(), lists_dir.join("partial")] {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name.contains("dog") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
    let archives_dir = Path::new(prefix)
        .join("var")
        .join("cache")
        .join("apt")
        .join("archives");
    if let Ok(entries) = fs::read_dir(&archives_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.starts_with("dog_") && name.ends_with(".deb") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn repair_dpkg_state() {
    if let Ok(audit) = Command::new("dpkg").arg("--audit").output() {
        if !audit.stdout.is_empty() || !audit.stderr.is_empty() {
            let _ = Command::new("dpkg").args(["--configure", "-a"]).status();
        }
    }
}

fn backup_dog_binary(prefix: &str, tmp_dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let target = env::current_exe()
        .ok()
        .filter(|p| p.is_file())
        .unwrap_or_else(|| Path::new(prefix).join("bin").join("dog"));
    if !target.is_file() {
        return None;
    }
    let backup = tmp_dir.join("dog.backup");
    fs::copy(&target, &backup).ok()?;
    Some((target, backup))
}

fn restore_dog_binary(pair: Option<&(PathBuf, PathBuf)>) {
    if let Some((target, backup)) = pair {
        if backup.is_file() {
            let _ = fs::copy(backup, target);
        }
    }
}

fn upgrade_via_termux_deb_direct(
    prefix: &str,
    arch: &str,
    epoch: u64,
    deb_version: Option<&str>,
    expected_sha: Option<&str>,
) -> Result<String, String> {
    let tmp_dir = env::var("TMPDIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let _ = fs::create_dir_all(&tmp_dir);
    let deb_path = tmp_dir.join(format!("dog_candidate_{arch}.deb"));
    let backup = backup_dog_binary(prefix, &tmp_dir);

    let mut candidates: Vec<(String, Option<&str>)> = vec![(
        format!("{TERMUX_APT_BASE_URL}/latest/dog_latest_{arch}.deb?t={epoch}"),
        expected_sha,
    )];
    if let Some(dv) = deb_version {
        candidates.push((
            format!("{TERMUX_APT_BASE_URL}/pool/main/d/dog/dog_{dv}_{arch}.deb?t={epoch}"),
            expected_sha,
        ));
    }
    if let Some(rel_url) = fetch_github_release_deb_url(arch, epoch) {
        candidates.push((rel_url, None));
    }

    let mut errors = Vec::new();
    for (url, sha_opt) in candidates {
        let _ = fs::remove_file(&deb_path);
        match download_url_bytes(&url, &deb_path) {
            Ok(bytes) => {
                if let Err(v_err) = verify_deb_bytes(&bytes, sha_opt) {
                    errors.push(format!("{url}: {v_err}"));
                    continue;
                }
                if let Err(w_err) = fs::write(&deb_path, &bytes) {
                    errors.push(format!("Failed writing '{}': {w_err}", deb_path.display()));
                    continue;
                }
                if let Ok(deb_check) = Command::new("dpkg-deb").arg("-I").arg(&deb_path).output() {
                    if !deb_check.status.success() {
                        errors.push(format!("{url}: {}", format_command_failure("dpkg-deb -I", &deb_check)));
                        let _ = fs::remove_file(&deb_path);
                        continue;
                    }
                }
                let dpkg_out = match Command::new("dpkg").arg("-i").arg(&deb_path).output() {
                    Ok(out) => out,
                    Err(e) => {
                        let _ = fs::remove_file(&deb_path);
                        errors.push(format!("Failed running dpkg -i: {e}"));
                        continue;
                    }
                };
                let _ = fs::remove_file(&deb_path);
                if !dpkg_out.status.success() {
                    let _ = Command::new("apt-get").args(["install", "-f", "-y"]).status();
                    errors.push(format_command_failure("dpkg -i", &dpkg_out));
                    continue;
                }
                if let Some(now_ver) = read_installed_dog_version() {
                    if let Some((_, backup_path)) = &backup {
                        let _ = fs::remove_file(backup_path);
                    }
                    return Ok(format!(
                        "Installed verified Termux .deb ({arch}) -> dog {now_ver}"
                    ));
                }
                errors.push("Installed .deb failed `dog --version` verification.".to_string());
                restore_dog_binary(backup.as_ref());
            }
            Err(e) => errors.push(e),
        }
    }

    restore_dog_binary(backup.as_ref());
    if let Some((_, backup_path)) = &backup {
        let _ = fs::remove_file(backup_path);
    }
    Err(errors.join(" | "))
}

pub fn handle_update() -> String {
    match handle_update_exec(&["--check".to_string()]) {
        Ok(s) => s,
        Err(e) => e,
    }
}

pub fn handle_update_exec(args: &[String]) -> Result<String, String> {
    let check_only = args.iter().any(|a| a == "--check" || a == "-c");
    let force = args.iter().any(|a| a == "--force" || a == "-f");
    let from_source = args.iter().any(|a| a == "--from-source" || a == "--source");
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(
            "DOG Updater\n\
             -----------\n\
             Usage:\n\
               dog update              Check GitHub/APT and install the newest version\n\
               dog update --check      Compare installed version vs GitHub main & Termux APT\n\
               dog update --force      Reinstall the latest package even if versions match\n\
               dog update --from-source Build & install directly from source with Cargo"
                .to_string(),
        );
    }

    let installed = env!("CARGO_PKG_VERSION");
    let epoch = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let github_ver = fetch_remote_text(GITHUB_CARGO_URL)
        .ok()
        .and_then(|s| parse_cargo_toml_version(&s));
    let version_json_url = format!("{TERMUX_APT_BASE_URL}/version.json?t={epoch}");
    let version_meta = fetch_remote_text(&version_json_url)
        .ok()
        .map(|s| parse_version_metadata(&s))
        .unwrap_or_default();
    let pages_ver = version_meta.version.clone();
    let pages_deb = version_meta.deb_version.clone();

    let termux_prefix = detect_termux_prefix();
    let arch = detect_termux_arch();

    let mut report = format!(
        "DOG Version Check\n\
         -----------------\n\
         Installed     : {installed}\n\
         GitHub main   : {}\n\
         Termux APT    : {}\n",
        github_ver.as_deref().unwrap_or("(offline / unreachable)"),
        match (&pages_ver, &pages_deb) {
            (Some(v), Some(d)) => format!("{v} (deb {d})"),
            (Some(v), None) => v.clone(),
            _ => "(waiting for gh-pages / unreachable)".to_string(),
        }
    );

    let remote_newest = github_ver
        .as_deref()
        .or(pages_ver.as_deref());
    let has_newer = remote_newest
        .is_some_and(|rv| compare_semver(rv, installed).is_gt())
        || pages_ver
            .as_deref()
            .is_some_and(|pv| compare_semver(pv, installed).is_gt());

    if check_only {
        if has_newer {
            report.push_str("\nUpdate available! Run `dog update` to upgrade automatically.");
        } else {
            report.push_str(&format!(
                "\nYou are on {installed}. Run `dog update --force` to reinstall the latest build."
            ));
        }
        return Ok(report);
    }

    if !force && !has_newer && (github_ver.is_some() || pages_ver.is_some()) {
        report.push_str(&format!(
            "\nDOG is already up to date (v{installed}). Run `dog update --force` to reinstall."
        ));
        return Ok(report);
    }

    if let Some(prefix) = termux_prefix {
        if !from_source {
            ensure_termux_apt_source(&prefix)?;
            repair_dpkg_state();
            purge_stale_dog_apt_caches(&prefix);
            // Sync ONLY dog.list with caching disabled so slow/broken upstream Termux mirrors never error out.
            let apt_update = Command::new("apt-get")
                .args([
                    "update",
                    "-o",
                    "Dir::Etc::sourcelist=sources.list.d/dog.list",
                    "-o",
                    "Dir::Etc::sourceparts=-",
                    "-o",
                    "APT::Get::List-Cleanup=0",
                    "-o",
                    "Acquire::http::No-Cache=true",
                    "-o",
                    "Acquire::Https::No-Cache=true",
                ])
                .output();

            let mut apt_upgraded = false;
            if let Ok(up) = apt_update {
                if up.status.success() {
                    if let Ok(inst) = Command::new("apt-get")
                        .args([
                            "install",
                            "-y",
                            "--reinstall",
                            "--allow-downgrades",
                            "-o",
                            "Acquire::http::No-Cache=true",
                            "-o",
                            "Acquire::Https::No-Cache=true",
                            "dog",
                        ])
                        .output()
                    {
                        if inst.status.success() {
                            let after = read_installed_dog_version().unwrap_or_default();
                            let stale_vs_pages = pages_ver
                                .as_deref()
                                .is_some_and(|pv| after.is_empty() || compare_semver(pv, &after).is_gt());
                            if !stale_vs_pages {
                                apt_upgraded = true;
                                report.push_str(&format!(
                                    "\nUpgraded via isolated DOG Termux APT (cache-free) -> dog {after}\n"
                                ));
                            }
                        }
                    }
                }
            }

            if !apt_upgraded {
                let expected_sha = version_meta.sha256_by_arch.get(arch).map(String::as_str);
                match upgrade_via_termux_deb_direct(
                    &prefix,
                    arch,
                    epoch,
                    pages_deb.as_deref(),
                    expected_sha,
                ) {
                    Ok(line) => report.push_str(&format!("\n{line}\n")),
                    Err(deb_err) => {
                        return Err(format!(
                            "{report}\nAutomatic Termux upgrade failed: {deb_err}\n\
                             One-line installer fallback:\n  curl -fsSL {TERMUX_APT_BASE_URL}/update.sh | bash"
                        ));
                    }
                }
            }

            if let (Some(gv), Some(pv)) = (&github_ver, &pages_ver) {
                if compare_semver(gv, pv).is_gt() {
                    report.push_str(&format!(
                        "\nNote: GitHub main is at v{gv}, while Termux APT currently hosts v{pv}.\n\
                         Wait ~2 minutes for GitHub Actions to publish v{gv}, or run `dog update --from-source`."
                    ));
                }
            }
            return Ok(report);
        }
    }

    // Source upgrade path (local git repo or Cargo git install)
    if Path::new("Cargo.toml").is_file() && Path::new(".git").exists() {
        let pull = Command::new("git")
            .args(["pull", "--ff-only"])
            .output()
            .map_err(|e| format!("{report}\nFailed running `git pull`: {e}"))?;
        if !pull.status.success() {
            return Err(format!("{report}\n{}", format_command_failure("git pull --ff-only", &pull)));
        }
        let cargo = Command::new("cargo")
            .args(["install", "--path", ".", "--force"])
            .output()
            .map_err(|e| format!("{report}\nFailed running `cargo install`: {e}"))?;
        if !cargo.status.success() {
            return Err(format!("{report}\n{}", format_command_failure("cargo install --path . --force", &cargo)));
        }
    } else {
        let cargo = Command::new("cargo")
            .args([
                "install",
                "--git",
                "https://github.com/kaydankotze17-lang/Dog.git",
                "--force",
            ])
            .output()
            .map_err(|e| format!("{report}\nFailed running `cargo install`: {e}"))?;
        if !cargo.status.success() {
            return Err(format!("{report}\n{}", format_command_failure("cargo install --git", &cargo)));
        }
    }

    let now_ver = read_installed_dog_version().unwrap_or_else(|| "latest".to_string());
    report.push_str(&format!("\nUpdated from source -> dog {now_ver}"));
    Ok(report)
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

// =========================================================================
// DOG 0.3.0 Developer Tools: Debug, Watch, Graph, Explain
// =========================================================================

pub struct DebugObserver<'a> {
    source_lines: Vec<String>,
    breakpoints: BTreeSet<usize>,
    stepping: bool,
    cmd_in: &'a mut dyn BufRead,
    cmd_out: &'a mut dyn Write,
}

impl<'a> DebugObserver<'a> {
    pub fn new(
        source: &str,
        cmd_in: &'a mut dyn BufRead,
        cmd_out: &'a mut dyn Write,
    ) -> Self {
        Self {
            source_lines: source.lines().map(String::from).collect(),
            breakpoints: BTreeSet::new(),
            stepping: true,
            cmd_in,
            cmd_out,
        }
    }

    fn print_source_window(&mut self, current_line: usize) {
        let total = self.source_lines.len();
        let start = current_line.saturating_sub(3).max(1);
        let end = (current_line + 3).min(total);
        for ln in start..=end {
            let marker = if ln == current_line { "=>" } else { "  " };
            let bp = if self.breakpoints.contains(&ln) { "*" } else { " " };
            let text = self
                .source_lines
                .get(ln.saturating_sub(1))
                .map(String::as_str)
                .unwrap_or("");
            let _ = writeln!(self.cmd_out, "{marker}{bp} {ln:>4} | {text}");
        }
    }
}

impl<'a> ExecObserver for DebugObserver<'a> {
    fn on_before_stmt(&mut self, stmt: &Stmt, env: &Environment) -> DebugStepDecision {
        let span = stmt_span(stmt);
        let line = span.line;
        if !self.stepping && !self.breakpoints.contains(&line) {
            return DebugStepDecision::Continue;
        }
        self.stepping = true;
        let code_line = self
            .source_lines
            .get(line.saturating_sub(1))
            .map(|s| s.trim())
            .unwrap_or("");
        let _ = writeln!(
            self.cmd_out,
            "[debug] {}:{}  {}",
            span.file, line, code_line
        );

        loop {
            let _ = write!(self.cmd_out, "(dog-dbg) ");
            let _ = self.cmd_out.flush();
            let mut input = String::new();
            match self.cmd_in.read_line(&mut input) {
                Ok(0) => {
                    self.stepping = false;
                    return DebugStepDecision::Continue;
                }
                Ok(_) => {
                    let cmd = input.trim();
                    let mut parts = cmd.split_whitespace();
                    match parts.next().unwrap_or("") {
                        "" | "n" | "next" | "s" | "step" => {
                            self.stepping = true;
                            return DebugStepDecision::Continue;
                        }
                        "c" | "cont" | "continue" => {
                            self.stepping = false;
                            return DebugStepDecision::Continue;
                        }
                        "b" | "break" => {
                            if let Some(ln) = parts.next().and_then(|s| s.parse::<usize>().ok()) {
                                if self.breakpoints.contains(&ln) {
                                    self.breakpoints.remove(&ln);
                                    let _ = writeln!(self.cmd_out, "Removed breakpoint at line {ln}");
                                } else {
                                    self.breakpoints.insert(ln);
                                    let _ = writeln!(self.cmd_out, "Set breakpoint at line {ln}");
                                }
                            } else {
                                let _ = writeln!(self.cmd_out, "Usage: b <line-number>");
                            }
                        }
                        "p" | "print" | "vars" => {
                            let syms = env.snapshot_all_symbols();
                            if let Some(target) = parts.next() {
                                if let Some((_, val)) = syms.iter().find(|(k, _)| k == target) {
                                    let _ = writeln!(self.cmd_out, "  {target} = {val}");
                                } else {
                                    let _ = writeln!(self.cmd_out, "  <{target} is not defined in current scope>");
                                }
                            } else if syms.is_empty() {
                                let _ = writeln!(self.cmd_out, "  (no variables in scope)");
                            } else {
                                for (k, v) in syms {
                                    let _ = writeln!(self.cmd_out, "  {k} = {v}");
                                }
                            }
                        }
                        "l" | "list" => {
                            self.print_source_window(line);
                        }
                        "h" | "help" | "?" => {
                            let _ = writeln!(
                                self.cmd_out,
                                "Commands: n/step, c/continue, b <line>, p [var]/vars, l/list, q/quit"
                            );
                        }
                        "q" | "quit" | "exit" => {
                            let _ = writeln!(self.cmd_out, "Debugger aborted execution.");
                            return DebugStepDecision::Abort;
                        }
                        other => {
                            let _ = writeln!(
                                self.cmd_out,
                                "Unknown debugger command '{other}'. Type 'h' for help."
                            );
                        }
                    }
                }
                Err(_) => return DebugStepDecision::Abort,
            }
        }
    }
}

pub fn run_debug_with_streams(
    path: &str,
    source: &str,
    personality: Personality,
    io: &mut dyn InterpreterIo,
    cmd_in: &mut dyn BufRead,
    cmd_out: &mut dyn Write,
) -> Result<(), String> {
    let program = check_source(path, source)
        .map_err(|d| d.format_with(personality, Some(source)))?;
    let _ = writeln!(
        cmd_out,
        "DOG Debugger attached to {path} (type 'h' for commands: n, c, b <line>, p, l, q)"
    );
    let mut observer = DebugObserver::new(source, cmd_in, cmd_out);
    let mut interp = Interpreter::with_observer(io, &mut observer);
    match interp.eval_program(&program) {
        Ok(()) => Ok(()),
        Err(diag) if diag.code == "E035" && diag.title.contains("aborted by debugger") => Ok(()),
        Err(diag) => Err(diag.format_with(personality, Some(source))),
    }
}

pub fn handle_debug(path: &str, personality: Personality) -> Result<(), String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    let mut stdio = StdIo;
    let stdin = io::stdin();
    let mut cmd_in = stdin.lock();
    let mut cmd_out = io::stderr();
    run_debug_with_streams(path, &source, personality, &mut stdio, &mut cmd_in, &mut cmd_out)
}

pub fn collect_watched_files(entry: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    queue.push_back(PathBuf::from(entry));

    while let Some(path) = queue.pop_front() {
        let canon = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen.insert(canon) {
            continue;
        }
        result.push(path.clone());
        if let Ok(src) = fs::read_to_string(&path) {
            if let Ok(tokens) = Lexer::new(&path.display().to_string(), &src).tokenize() {
                if let Ok(program) = Parser::new(tokens).parse() {
                    let base = path.parent().unwrap_or_else(|| Path::new("."));
                    for stmt in &program.statements {
                        if let Stmt::Use { path: rel, .. } = stmt {
                            if !rel.starts_with("std.") {
                                queue.push_back(base.join(rel));
                            }
                        }
                    }
                }
            }
        }
    }
    result.sort();
    result
}

pub fn snapshot_mtimes(files: &[PathBuf]) -> Vec<(PathBuf, Option<SystemTime>)> {
    files
        .iter()
        .map(|p| {
            let mtime = fs::metadata(p).and_then(|m| m.modified()).ok();
            (p.clone(), mtime)
        })
        .collect()
}

pub fn handle_watch(args: &[String], personality: Personality) -> Result<(), String> {
    let mut test_mode = false;
    let mut check_only = false;
    let mut target: Option<String> = None;

    for a in args {
        match a.as_str() {
            "--test" | "-t" => test_mode = true,
            "--check" | "-c" => check_only = true,
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "Unknown watch flag '{flag}'. Usage: dog watch <file.dg> [--test | --check]"
                ));
            }
            val if target.is_none() => target = Some(val.to_string()),
            extra => {
                return Err(format!(
                    "Unexpected argument '{extra}'. Usage: dog watch <file.dg> [--test | --check]"
                ));
            }
        }
    }

    let entry = target.unwrap_or_else(|| {
        if Path::new("src/main.dg").exists() {
            "src/main.dg".to_string()
        } else {
            "main.dg".to_string()
        }
    });
    if !Path::new(&entry).exists() && !test_mode {
        return Err(format!(
            "Watch target '{entry}' does not exist. Usage: dog watch <file.dg> [--test | --check]"
        ));
    }

    let messenger = DogConfig::load().messenger();
    loop {
        messenger.info(&format!("[watch] Running '{entry}' ..."));
        if test_mode {
            if let Err(err) = crate::tester::run_tests(Some(&entry), personality) {
                messenger.error(&err);
            }
        } else {
            match fs::read_to_string(&entry) {
                Ok(source) => {
                    if check_only {
                        match check_source(&entry, &source) {
                            Ok(_) => messenger.success(&format!("{entry} passed checks.")),
                            Err(diag) => eprintln!("{}", diag.format_with(personality, Some(&source))),
                        }
                    } else {
                        let mut stdio = StdIo;
                        if let Err(diag) = run_source_with_io(&entry, &source, &mut stdio) {
                            eprintln!("{}", diag.format_with(personality, Some(&source)));
                        }
                    }
                }
                Err(err) => messenger.error(&format!("Failed to read '{entry}': {err}")),
            }
        }

        let watched = if Path::new(&entry).is_dir() {
            crate::tester::collect_test_files(Some(&entry)).unwrap_or_default()
        } else {
            collect_watched_files(&entry)
        };
        let mut last_snap = snapshot_mtimes(&watched);
        messenger.info(&format!(
            "[watch] Watching {} file(s) for changes (Ctrl+C to stop)...",
            watched.len()
        ));

        loop {
            thread::sleep(Duration::from_millis(450));
            let cur_files = if Path::new(&entry).is_dir() {
                crate::tester::collect_test_files(Some(&entry)).unwrap_or_default()
            } else {
                collect_watched_files(&entry)
            };
            let cur_snap = snapshot_mtimes(&cur_files);
            if cur_snap != last_snap {
                last_snap = cur_snap;
                break;
            }
        }
    }
}

pub fn handle_graph(target: Option<&str>) -> Result<String, String> {
    let mut out = String::from("DOG Dependency Graph\n--------------------\n");
    let pack_path = Path::new("dog.pack");
    if pack_path.is_file() {
        if let Ok(pack) = DogPack::load(pack_path) {
            out.push_str(&format!("Package: {} v{}\n", pack.name, pack.version));
            if pack.dependencies.is_empty() {
                out.push_str("  [dependencies] (none)\n");
            } else {
                for (dep, spec) in &pack.dependencies {
                    out.push_str(&format!("  +-- pkg:{dep} ({spec})\n"));
                }
            }
            out.push('\n');
        }
    }

    let mut files: Vec<PathBuf> = Vec::new();
    match target {
        Some(p) => {
            let path = PathBuf::from(p);
            if path.is_file() {
                files = collect_watched_files(p);
            } else if path.is_dir() {
                collect_all_dg_files(&path, &mut files);
                files.sort();
            } else {
                return Err(format!("Graph target '{p}' does not exist."));
            }
        }
        None => {
            if Path::new("src/main.dg").is_file() {
                files = collect_watched_files("src/main.dg");
            } else {
                collect_all_dg_files(Path::new("."), &mut files);
                files.sort();
            }
        }
    }

    if files.is_empty() {
        out.push_str("No `.dg` files found in target.\n");
        return Ok(out);
    }

    out.push_str("Module Edges:\n");
    for file in &files {
        let label = file.display().to_string();
        let src = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut local_uses: Vec<String> = Vec::new();
        let mut std_mods: BTreeSet<String> = BTreeSet::new();
        if let Ok(tokens) = Lexer::new(&label, &src).tokenize() {
            if let Ok(program) = Parser::new(tokens).parse() {
                for stmt in &program.statements {
                    if let Stmt::Use { path, .. } = stmt {
                        if path.starts_with("std.") {
                            std_mods.insert(path.clone());
                        } else {
                            local_uses.push(path.clone());
                        }
                    } else {
                        collect_std_namespaces_in_stmt(stmt, &mut std_mods);
                    }
                }
            }
        }
        out.push_str(&format!("  {label}\n"));
        for u in &local_uses {
            out.push_str(&format!("    +-- use \"{u}\"\n"));
        }
        for s in &std_mods {
            out.push_str(&format!("    +-- {s}\n"));
        }
        if local_uses.is_empty() && std_mods.is_empty() {
            out.push_str("    +-- (no imports)\n");
        }
    }
    Ok(out)
}

fn collect_all_dg_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                if name == "target" || name.starts_with('.') {
                    continue;
                }
            }
            collect_all_dg_files(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("dg") {
            out.push(p);
        }
    }
}

fn collect_std_namespaces_in_stmt(stmt: &Stmt, out: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Binding { value, .. } | Stmt::Assign { value, .. } | Stmt::CompoundAssign { value, .. } => {
            collect_std_namespaces_in_expr(value, out);
        }
        Stmt::FieldAssign { value, .. } => collect_std_namespaces_in_expr(value, out),
        Stmt::IndexAssign { index, value, .. } => {
            collect_std_namespaces_in_expr(index, out);
            collect_std_namespaces_in_expr(value, out);
        }
        Stmt::If {
            condition,
            then_branch,
            otherwise_branch,
            ..
        } => {
            collect_std_namespaces_in_expr(condition, out);
            for s in then_branch {
                collect_std_namespaces_in_stmt(s, out);
            }
            if let Some(el) = otherwise_branch {
                for s in el {
                    collect_std_namespaces_in_stmt(s, out);
                }
            }
        }
        Stmt::Match {
            subject,
            cases,
            otherwise_branch,
            ..
        } => {
            collect_std_namespaces_in_expr(subject, out);
            for c in cases {
                collect_std_namespaces_in_expr(&c.pattern, out);
                for s in &c.body {
                    collect_std_namespaces_in_stmt(s, out);
                }
            }
            if let Some(el) = otherwise_branch {
                for s in el {
                    collect_std_namespaces_in_stmt(s, out);
                }
            }
        }
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => {
            for s in try_body {
                collect_std_namespaces_in_stmt(s, out);
            }
            for s in catch_body {
                collect_std_namespaces_in_stmt(s, out);
            }
        }
        Stmt::While { condition, body, .. } => {
            collect_std_namespaces_in_expr(condition, out);
            for s in body {
                collect_std_namespaces_in_stmt(s, out);
            }
        }
        Stmt::Repeat { count, body, .. } => {
            collect_std_namespaces_in_expr(count, out);
            for s in body {
                collect_std_namespaces_in_stmt(s, out);
            }
        }
        Stmt::For { iterable, body, .. } => {
            collect_std_namespaces_in_expr(iterable, out);
            for s in body {
                collect_std_namespaces_in_stmt(s, out);
            }
        }
        Stmt::FnDef { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    collect_std_namespaces_in_expr(d, out);
                }
            }
            for s in body {
                collect_std_namespaces_in_stmt(s, out);
            }
        }
        Stmt::StyleDef {
            selector,
            rules,
            variants,
            ..
        } => {
            collect_std_namespaces_in_expr(selector, out);
            for r in rules {
                collect_std_namespaces_in_expr(&r.value, out);
            }
            for v in variants {
                for r in &v.rules {
                    collect_std_namespaces_in_expr(&r.value, out);
                }
            }
        }
        Stmt::Return { value: Some(e), .. } | Stmt::ExprStmt { expr: e, .. } => {
            collect_std_namespaces_in_expr(e, out);
        }
        Stmt::Use { .. }
        | Stmt::StructDef { .. }
        | Stmt::EnumDef { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => {}
    }
}

fn collect_std_namespaces_in_expr(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::NamespaceCall { module, args, .. } => {
            out.insert(module.clone());
            for a in args {
                collect_std_namespaces_in_expr(a, out);
            }
        }
        Expr::Call { args, .. } | Expr::ListLit { elements: args, .. } => {
            for a in args {
                collect_std_namespaces_in_expr(a, out);
            }
        }
        Expr::InterpString { parts, .. } => {
            for p in parts {
                if let InterpPart::Expr(e) = p {
                    collect_std_namespaces_in_expr(e, out);
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_std_namespaces_in_expr(k, out);
                collect_std_namespaces_in_expr(v, out);
            }
        }
        Expr::StructInit { fields, .. } => {
            for (_, v) in fields {
                collect_std_namespaces_in_expr(v, out);
            }
        }
        Expr::FieldAccess { target, .. } | Expr::Unary { expr: target, .. } => {
            collect_std_namespaces_in_expr(target, out);
        }
        Expr::Index { target, index, .. } => {
            collect_std_namespaces_in_expr(target, out);
            collect_std_namespaces_in_expr(index, out);
        }
        Expr::Binary { left, right, .. } => {
            collect_std_namespaces_in_expr(left, out);
            collect_std_namespaces_in_expr(right, out);
        }
        Expr::StringLit { .. }
        | Expr::NumberLit { .. }
        | Expr::DecimalLit { .. }
        | Expr::BoolLit { .. }
        | Expr::Identifier { .. } => {}
    }
}

pub fn handle_explain(target: &str) -> Result<String, String> {
    let upper = target.trim().to_ascii_uppercase();
    if upper.starts_with('E')
        && upper.len() == 4
        && upper[1..].chars().all(|c| c.is_ascii_digit())
    {
        return explain_error_code(&upper)
            .ok_or_else(|| format!("Unknown DOG error code '{upper}'. Valid codes span E001-E041."));
    }

    let source =
        fs::read_to_string(target).map_err(|e| format!("Failed to read '{target}': {e}"))?;
    let tokens = Lexer::new(target, &source)
        .tokenize()
        .map_err(|d| format!("{d}"))?;
    let program = Parser::new(tokens).parse().map_err(|d| format!("{d}"))?;

    let mut uses: Vec<String> = Vec::new();
    let mut structs: Vec<String> = Vec::new();
    let mut enums: Vec<String> = Vec::new();
    let mut functions: Vec<String> = Vec::new();
    let mut bindings: Vec<String> = Vec::new();
    let mut has_loops = false;
    let mut has_try = false;
    let mut std_used: BTreeSet<String> = BTreeSet::new();

    for stmt in &program.statements {
        collect_std_namespaces_in_stmt(stmt, &mut std_used);
        match stmt {
            Stmt::Use { path, .. } => uses.push(path.clone()),
            Stmt::StructDef { name, fields, .. } => {
                structs.push(format!("struct {name} ({} fields)", fields.len()));
            }
            Stmt::EnumDef { name, variants, .. } => {
                enums.push(format!("enum {name} ({})", variants.join(", ")));
            }
            Stmt::FnDef {
                name,
                params,
                return_type,
                ..
            } => {
                let req = params.iter().filter(|p| p.default.is_none()).count();
                let opt = params.len() - req;
                let opt_note = if opt > 0 {
                    format!(", {opt} default")
                } else {
                    String::new()
                };
                functions.push(format!(
                    "fn {name}({} params{opt_note}) -> {:?}",
                    params.len(),
                    return_type
                ));
            }
            Stmt::Binding { kind, name, .. } => {
                bindings.push(format!("{kind:?} {name}"));
            }
            Stmt::While { .. } | Stmt::Repeat { .. } | Stmt::For { .. } => has_loops = true,
            Stmt::TryCatch { .. } => has_try = true,
            _ => {}
        }
    }

    let mut caps: Vec<&str> = Vec::new();
    if source.contains("http_") || source.contains("ws_") || source.contains("tcp_") || source.contains("udp_") {
        caps.push("Networking (HTTP/WS/TCP/UDP)");
    }
    if source.contains("file_") || source.contains("dir_list") || source.contains("kv_") || source.contains("json_read") {
        caps.push("Filesystem / Persistent KV");
    }
    if source.contains("ask(") || source.contains("answer(") || source.contains("confirm(") || source.contains("choose(") || source.contains("password(") {
        caps.push("Interactive Terminal I/O");
    }
    if source.contains("sha") || source.contains("hmac_") || source.contains("base64") {
        caps.push("Cryptography");
    }
    if source.contains("exec_cmd") {
        caps.push("Subprocess Execution");
    }
    if source.contains("dom_") || source.contains("html_") || source.contains("web_") || source.contains("style \"") || source.contains("css_") {
        caps.push("DOG Web & CSS UI");
    }

    let mut out = format!(
        "DOG Code Explanation: {target}\n\
         ------------------------------\n\
         Lines       : {}\n\
         Statements  : {}\n\
         Imports     : {}\n\
         Std Modules : {}\n\
         Control Flow: loops={}, try/catch={}\n\
         Capabilities: {}\n",
        source.lines().count(),
        program.statements.len(),
        if uses.is_empty() {
            "none".to_string()
        } else {
            uses.join(", ")
        },
        if std_used.is_empty() {
            "global built-ins".to_string()
        } else {
            std_used.into_iter().collect::<Vec<_>>().join(", ")
        },
        has_loops,
        has_try,
        if caps.is_empty() {
            "Pure computation".to_string()
        } else {
            caps.join(", ")
        }
    );

    if !structs.is_empty() || !enums.is_empty() {
        out.push_str("\nData Types:\n");
        for s in structs {
            out.push_str(&format!("  - {s}\n"));
        }
        for e in enums {
            out.push_str(&format!("  - {e}\n"));
        }
    }
    if !functions.is_empty() {
        out.push_str("\nFunctions:\n");
        for f in functions {
            out.push_str(&format!("  - {f}\n"));
        }
    }
    if !bindings.is_empty() {
        out.push_str("\nTop-Level Bindings:\n");
        for b in bindings {
            out.push_str(&format!("  - {b}\n"));
        }
    }
    Ok(out)
}

fn explain_error_code(code: &str) -> Option<String> {
    let (title, cause, example_bad, example_fix) = match code {
        "E001" => (
            "Unexpected character / invalid token",
            "Source contains a character or operator not recognized by the DOG lexer.",
            "let x = 1 @ 2",
            "Use valid DOG operators: +, -, *, /, %, ==, !=, <, >, <=, >=",
        ),
        "E002" => (
            "Unterminated string literal",
            "A plain \"...\" or triple-quoted \"\"\"...\"\"\" string was opened without a matching closing quote.",
            "let name = \"DOG",
            "let name = \"DOG\" (use \"\"\"...\"\"\" for multi-line strings)",
        ),
        "E003" => (
            "Malformed numeric literal",
            "A decimal, hex (0x...), or binary (0b...) literal has invalid digits or overflows i64/f64.",
            "let bad = 0xZZ",
            "let ok = 0xFF_00",
        ),
        "E004" => (
            "Unexpected syntax token",
            "Parser expected a keyword, identifier, delimiter, or expression at this position.",
            "let = 5",
            "let x = 5",
        ),
        "E005" => (
            "Unclosed block / missing `end`",
            "A block opened by if, while, for, repeat, fn, match, try, struct, or enum was never closed with `end`.",
            "if true\n    say(\"hi\")",
            "if true\n    say(\"hi\")\nend",
        ),
        "E010" => (
            "Undefined identifier, field, or enum variant",
            "Referenced variable, function, struct field, or enum variant does not exist in scope.",
            "say(usre_name)",
            "Declare with `let user_name = ...` or apply the suggested spelling fix.",
        ),
        "E011" => (
            "Duplicate definition",
            "A variable, constant, parameter, struct, enum, or function was declared more than once in the same scope.",
            "let x = 1\nlet x = 2",
            "Use distinct names or `var x = 1` followed by `x = 2`.",
        ),
        "E012" => (
            "Mutability violation",
            "Attempted to reassign a `let`/`const` binding or pass an immutable binding where `var` is required (e.g. push/sort).",
            "let count = 1\ncount = 2",
            "var count = 1\ncount = 2",
        ),
        "E013" => (
            "Unknown function or wrong callable",
            "Called a function name that is neither a user `fn` nor a DOG built-in.",
            "prnit(\"hi\")",
            "say(\"hi\")",
        ),
        "E014" => (
            "Function arity mismatch",
            "Passed too few or too many arguments to a function or built-in.",
            "fn add(a: number, b: number) -> number ... end\nadd(1)",
            "Pass all required parameters or define a default value: `b: number = 0`.",
        ),
        "E021" => (
            "Strict type mismatch",
            "DOG forbids implicit coercion between incompatible types such as `text + number`.",
            "say(\"Age: \" + 42)",
            "say(\"Age: \" + text(42)) or say($\"Age: {42}\")",
        ),
        "E022" | "E023" => (
            "Non-boolean condition or logical operand",
            "`if`, `while`, `and`, `or`, and `not` require explicit `bool` values.",
            "if 1\n    say(\"yes\")\nend",
            "if 1 == 1\n    say(\"yes\")\nend",
        ),
        "E024" => (
            "Empty collection requires type annotation",
            "An empty `[]` or `{}` needs an explicit `: list<T>` or `: map<K, V>` type.",
            "var items = []",
            "var items: list<text> = []",
        ),
        "E025" => (
            "Control flow outside valid context",
            "`return` used outside a function, or `break`/`continue` used outside a loop.",
            "break",
            "Use `break`/`continue` inside `while`, `for`, or `repeat` loops only.",
        ),
        "E026" => (
            "Missing return path in function",
            "A function with a non-void return type has an execution path that finishes without returning a value.",
            "fn f(ok: bool) -> number\n    if ok\n        return 1\n    end\nend",
            "Ensure every branch (including `otherwise`) returns a value.",
        ),
        "E030" => (
            "Runtime conversion / value domain error",
            "`number(...)` or `decimal(...)` received non-numeric text, or negative repeat count.",
            "number(\"abc\")",
            "Validate input inside `try ... catch err ... end`.",
        ),
        "E031" => (
            "Division or modulo by zero",
            "Denominator evaluated to 0 or 0.0 at runtime.",
            "let q = 10 / 0",
            "if d != 0\n    let q = 10 / d\nend",
        ),
        "E032" => (
            "Call stack overflow",
            "Recursion exceeded MAX_CALL_DEPTH (256 frames).",
            "fn loop_forever() -> void\n    loop_forever()\nend",
            "Add a terminating base case to recursive functions.",
        ),
        "E033" => (
            "Assertion failure",
            "`assert(cond, msg)` evaluated to false.",
            "assert(2 + 2 == 5, \"bad math\")",
            "Verify the condition or input invariants.",
        ),
        "E034" => (
            "Index out of bounds",
            "Attempted to read or write an index < 0 or >= len(collection).",
            "let xs = [1, 2]\nsay(xs[5])",
            "Check `idx >= 0 and idx < len(xs)` before indexing.",
        ),
        "E035" => (
            "Explicit runtime failure (`fail()`)",
            "Raised by `fail(reason)` or uncaught error.",
            "fail(\"missing config\")",
            "Wrap fallible calls in `try ... catch err ... end`.",
        ),
        "E040" => (
            "I/O, network, or module load error",
            "File not found, invalid `std.*` module name, socket error, or HTTP/WS failure.",
            "use \"std.unknown\"",
            "Check file paths, permissions, or `dog docs std` for valid modules.",
        ),
        "E041" => (
            "Circular module import",
            "Module A `use`s Module B which directly or indirectly `use`s Module A.",
            "a.dg -> use \"b.dg\"; b.dg -> use \"a.dg\"",
            "Extract shared definitions into a third `common.dg` module.",
        ),
        _ => return None,
    };

    Some(format!(
        "DOG Error Reference: {code} — {title}\n\
         ------------------------------------------\n\
         Cause  : {cause}\n\
         Invalid: {example_bad}\n\
         Fix    : {example_fix}"
    ))
}

pub fn handle_web(args: &[String]) -> Result<String, String> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    match sub {
        "css" => {
            let Some(path) = args.get(1) else {
                return Err("Usage: dog web css <style.dgss> [--out style.css]".to_string());
            };
            let mut out_path: Option<String> = None;
            let mut i = 2usize;
            while i < args.len() {
                if args[i] == "--out" || args[i] == "-o" {
                    i += 1;
                    out_path = Some(
                        args.get(i)
                            .ok_or_else(|| "Expected file path after --out".to_string())?
                            .clone(),
                    );
                } else {
                    return Err(format!("Unexpected option '{}'", args[i]));
                }
                i += 1;
            }
            let raw = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read DOG CSS '{path}': {e}"))?;
            let compiled = crate::web::compile_dog_css(&raw)?;
            if let Some(dst) = out_path {
                fs::write(&dst, format!("{compiled}\n"))
                    .map_err(|e| format!("Failed to write '{dst}': {e}"))?;
                Ok(format!("Compiled DOG CSS '{path}' -> '{dst}'"))
            } else {
                Ok(compiled)
            }
        }
        "build" => {
            let Some(entry) = args.get(1) else {
                return Err(
                    "Usage: dog web build <app.dg> [--css style.dgss] [--out dist_web] [--title Title]"
                        .to_string(),
                );
            };
            let mut css_path: Option<String> = None;
            let mut out_dir = String::from("dist_web");
            let mut title = String::from("DOG Web App");
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--css" | "-c" => {
                        i += 1;
                        css_path = Some(
                            args.get(i)
                                .ok_or_else(|| "Expected file path after --css".to_string())?
                                .clone(),
                        );
                    }
                    "--out" | "-o" => {
                        i += 1;
                        out_dir = args
                            .get(i)
                            .ok_or_else(|| "Expected directory path after --out".to_string())?
                            .clone();
                    }
                    "--title" | "-t" => {
                        i += 1;
                        title = args
                            .get(i)
                            .ok_or_else(|| "Expected title string after --title".to_string())?
                            .clone();
                    }
                    other => {
                        return Err(format!("Unknown option '{other}' in `dog web build`."));
                    }
                }
                i += 1;
            }
            let dg_src = fs::read_to_string(entry)
                .map_err(|e| format!("Failed to read '{entry}': {e}"))?;
            let auto_css = if css_path.is_none() {
                let candidate = Path::new(entry).with_extension("dgss");
                if candidate.is_file() {
                    Some(candidate)
                } else {
                    None
                }
            } else {
                css_path.map(PathBuf::from)
            };
            let dgss_src = match &auto_css {
                Some(p) => Some(
                    fs::read_to_string(p)
                        .map_err(|e| format!("Failed to read '{}': {e}", p.display()))?,
                ),
                None => None,
            };
            let bundle =
                crate::web::build_web_bundle(&dg_src, dgss_src.as_deref(), &title)?;
            let out_root = Path::new(&out_dir);
            fs::create_dir_all(out_root)
                .map_err(|e| format!("Failed to create '{out_dir}': {e}"))?;
            fs::write(out_root.join("index.html"), &bundle.index_html)
                .map_err(|e| format!("Failed to write index.html: {e}"))?;
            fs::write(out_root.join("styles.css"), format!("{}\n", bundle.css))
                .map_err(|e| format!("Failed to write styles.css: {e}"))?;
            fs::write(out_root.join("app.js"), format!("{}\n", bundle.js))
                .map_err(|e| format!("Failed to write app.js: {e}"))?;
            Ok(format!(
                "Built DOG website bundle in '{out_dir}/' (index.html, styles.css, app.js)"
            ))
        }
        "serve" => {
            let Some(entry) = args.get(1) else {
                return Err(
                    "Usage: dog web serve <app.dg> [--css style.dgss] [--port 8080]"
                        .to_string(),
                );
            };
            let mut build_args = vec![
                "build".to_string(),
                entry.clone(),
                "--out".to_string(),
                ".dog_web_dist".to_string(),
            ];
            let mut port = String::from("8080");
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--port" | "-p" => {
                        i += 1;
                        port = args
                            .get(i)
                            .ok_or_else(|| "Expected port number after --port".to_string())?
                            .clone();
                    }
                    "--css" | "-c" | "--title" | "-t" => {
                        let flag = args[i].clone();
                        i += 1;
                        let val = args
                            .get(i)
                            .ok_or_else(|| format!("Expected value after {flag}"))?
                            .clone();
                        build_args.push(flag);
                        build_args.push(val);
                    }
                    arg if arg.chars().all(|c| c.is_ascii_digit()) => {
                        port = arg.to_string();
                    }
                    other => {
                        return Err(format!("Unknown option '{other}' in `dog web serve`."));
                    }
                }
                i += 1;
            }
            let summary = handle_web(&build_args)?;
            println!("{summary}");
            handle_server(&[".dog_web_dist".to_string(), port])?;
            Ok(String::new())
        }
        _ => Err(
            "DOG Web & CSS Toolchain:\n  dog web build <app.dg> [--css <style.dgss>] [--out dist_web]\n  dog web css <style.dgss> [--out style.css]\n  dog web serve <app.dg> [--port 8080]"
                .to_string(),
        ),
    }
}

