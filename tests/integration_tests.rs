use dog_lang::bytecode::{
    build_bundle, decode_bundle, run_bundle_with_io, transpile_program_to_rust,
};
use dog_lang::checker::builtin_names;
use dog_lang::config::Personality;
use dog_lang::crypto::{
    base64_decode, base64_encode, base64_url_decode, base64_url_encode, constant_time_eq,
    hex_decode, hex_encode, hmac_sha256_hex, hmac_sha256_verify, rand_bytes_hex, sha256_hex,
    sha512_hex,
};
use dog_lang::editor::{
    builtin_signature_help, render_editor_frame_rows, visible_ansi_len, EditorBuffer,
};
use dog_lang::formatter::format_source;
use dog_lang::hub::DogPack;
use dog_lang::interpreter::{check_source, resolve_program, run_source_with_io};
use dog_lang::json::parse_json;
use dog_lang::lexer::Lexer;
use dog_lang::net::{http_status_text, local_ip};
use dog_lang::prompt::{handle_completions, handle_prompt};
use dog_lang::sdu::handle_sdu;
use dog_lang::stdlib::modules::{resolve_std_call, STD_MODULES};
use dog_lang::stdlib::InterpreterIo;
use dog_lang::sys::{
    collect_watched_files, eval_snippet_captured, handle_bench, handle_docs, handle_explain,
    handle_graph, handle_learn, handle_lint, run_debug_with_streams,
};
use dog_lang::term::{DogMessenger, TermCapabilities, TermStyleConfig};
use dog_lang::tester::run_coverage;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io::Cursor;

struct MockIo {
    inputs: VecDeque<String>,
    output: String,
}

impl MockIo {
    fn new(inputs: Vec<&str>) -> Self {
        Self {
            inputs: inputs.into_iter().map(String::from).collect(),
            output: String::new(),
        }
    }
}

impl InterpreterIo for MockIo {
    fn write_stdout(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn writeln_stdout(&mut self, text: &str) {
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn read_line(&mut self) -> Result<String, String> {
        Ok(self.inputs.pop_front().unwrap_or_default())
    }
}

#[test]
fn executes_hello_example() {
    let mut io = MockIo::new(vec![]);
    let src = include_str!("../examples/hello.dg");
    run_source_with_io("examples/hello.dg", src, &mut io).expect("hello.dg should run");
    assert_eq!(io.output, "DOG is alive!\n");
}

#[test]
fn enforces_strict_typing_e021() {
    let src = "let msg = \"Age: \" + 42\nsay(msg)\n";
    let diag = check_source("bad.dg", src).expect_err("text + number must fail");
    assert_eq!(diag.code, "E021");
    assert_eq!(diag.span.line, 1);
}

#[test]
fn enforces_let_immutability_e012() {
    let src = "let x = 10\nx = 20\n";
    let diag = check_source("immut.dg", src).expect_err("reassignment to let must fail");
    assert_eq!(diag.code, "E012");
}

#[test]
fn runs_if_otherwise_end_regardless_of_indentation() {
    let src = "\
let response = lower(answer(\"Continue? \"))
if response == \"yes\" or response == \"y\"
say(\"Let's go.\")
        otherwise
  say(\"No problem.\")
end
";
    let mut io = MockIo::new(vec!["Y"]);
    run_source_with_io("indent.dg", src, &mut io).expect("irregular indentation should still run");
    assert_eq!(io.output, "Continue? Let's go.\n");
}

#[test]
fn runs_age_check_with_number_conversion() {
    let src = include_str!("../examples/conditions.dg");
    let mut io = MockIo::new(vec!["21", "no"]);
    run_source_with_io("examples/conditions.dg", src, &mut io).expect("conditions.dg should run");
    assert!(io.output.contains("Welcome!"));
    assert!(io.output.contains("No problem."));
}

#[test]
fn runs_functions_and_recursion() {
    let src = include_str!("../examples/functions.dg");
    let mut io = MockIo::new(vec![]);
    run_source_with_io("examples/functions.dg", src, &mut io).expect("functions.dg should run");
    assert_eq!(io.output, "Hello, DOG Developer\n5! = 120\n");
}

#[test]
fn runs_loops_and_lists() {
    let src = include_str!("../examples/loops.dg");
    let mut io = MockIo::new(vec![]);
    run_source_with_io("examples/loops.dg", src, &mut io).expect("loops.dg should run");
    assert_eq!(
        io.output,
        "ALPHA\nGAMMA\nDELTA\ntick 0\ntick 1\ntick 2\nbark!\nbark!\n"
    );
}

#[test]
fn runs_structs_and_enums_example() {
    let src = include_str!("../examples/structs_enums.dg");
    let mut io = MockIo::new(vec![]);
    run_source_with_io("examples/structs_enums.dg", src, &mut io)
        .expect("structs_enums.dg should run");
    assert!(io.output.contains("User: Ada (30)"));
    assert!(io.output.contains("Access: full pack leader"));
}

#[test]
fn runs_json_crypto_and_try_catch_example() {
    let src = include_str!("../examples/json_crypto.dg");
    let mut io = MockIo::new(vec![]);
    run_source_with_io("examples/json_crypto.dg", src, &mut io)
        .expect("json_crypto.dg should run");
    assert!(io.output.contains("Parsed lang: DOG"));
    assert!(io.output.contains("Recovered safely:"));
}

#[test]
fn runs_module_imports_from_disk() {
    let src = include_str!("../examples/modules_main.dg");
    let mut io = MockIo::new(vec![]);
    run_source_with_io("examples/modules_main.dg", src, &mut io)
        .expect("modules_main.dg should import helpers.dg");
    assert_eq!(io.output, "[DOG-MOD] MODULAR DOG PROJECT\n");
}

#[test]
fn detects_circular_module_imports_e041() {
    let dir = env::temp_dir().join("dog_cycle_test");
    let _ = fs::create_dir_all(&dir);
    let a = dir.join("a.dg");
    let b = dir.join("b.dg");
    fs::write(&a, "use \"b.dg\"\nsay(\"a\")\n").unwrap();
    fs::write(&b, "use \"a.dg\"\nsay(\"b\")\n").unwrap();

    let src = fs::read_to_string(&a).unwrap();
    let diag = check_source(&a.display().to_string(), &src).expect_err("cycle must fail");
    assert_eq!(diag.code, "E041");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_missing_return_path_e026() {
    let src = "\
fn maybe(flag: bool) -> number
    if flag
        return 1
    end
end
";
    let diag = check_source("ret.dg", src).expect_err("missing return path must fail");
    assert_eq!(diag.code, "E026");
}

#[test]
fn rejects_break_outside_loop_e025() {
    let diag = check_source("brk.dg", "break\n").expect_err("break outside loop must fail");
    assert_eq!(diag.code, "E025");
}

#[test]
fn runs_language_test_file() {
    let src = include_str!("language_test.dg");
    let mut io = MockIo::new(vec![]);
    run_source_with_io("tests/language_test.dg", src, &mut io)
        .expect("language_test.dg assertions should pass");
}

#[test]
fn formats_blocks_including_match_and_try_cleanly() {
    let messy = "match x\ncase 1\nif true\nsay(\"a\")\notherwise\nsay(\"b\")\nend\notherwise\nsay(\"c\")\nend\ntry\nsay(\"ok\")\ncatch err\nsay(err)\nend\n";
    let expected = "\
match x
    case 1
        if true
            say(\"a\")
        otherwise
            say(\"b\")
        end
    otherwise
        say(\"c\")
end
try
    say(\"ok\")
catch err
    say(err)
end
";
    assert_eq!(format_source(messy), expected);
}

#[test]
fn crypto_and_json_primitives_verify_known_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(hex_decode(&hex_encode(b"DOG!")).unwrap(), b"DOG!");
    assert_eq!(base64_decode(&base64_encode(b"DOG!")).unwrap(), b"DOG!");
    let val = parse_json("{\"ok\":true,\"n\":7}").unwrap();
    assert_eq!(val.stringify(), "{\"n\": 7, \"ok\": true}");
}

#[test]
fn builds_verifies_and_executes_dgb_bundle() {
    let dir = env::temp_dir().join("dog_dgb_test");
    let _ = fs::create_dir_all(&dir);
    let src_path = dir.join("app.dg");
    let dgb_path = dir.join("app.dgb");
    fs::write(&src_path, "let who = \"Bundled\"\nsay(\"Hello \" + who)\n").unwrap();

    let (out_path, bundle) = build_bundle(
        &src_path.display().to_string(),
        Some(&dgb_path.display().to_string()),
    )
    .expect("bundle build must succeed");
    assert_eq!(out_path, dgb_path);

    let decoded = decode_bundle(&fs::read(&dgb_path).unwrap()).expect("checksum must match");
    assert_eq!(decoded.sha256, bundle.sha256);

    let mut io = MockIo::new(vec![]);
    run_bundle_with_io(&dgb_path.display().to_string(), &mut io)
        .expect("bundle execution must succeed");
    assert_eq!(io.output, "Hello Bundled\n");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn parses_and_renders_dog_pack_and_reports_sdu_status() {
    let raw = "[package]\nname = \"kennel\"\nversion = \"0.2.0\"\nentry = \"src/main.dg\"\n\n[dependencies]\ncore = \"0.1.0\"\n";
    let pack = DogPack::parse(raw).expect("dog.pack should parse");
    assert_eq!(pack.name, "kennel");
    assert_eq!(pack.dependencies.get("core").map(String::as_str), Some("0.1.0"));
    assert!(pack.render().contains("[dependencies]"));

    let sdu_out = handle_sdu(&["status".to_string()]).expect("sdu status should succeed");
    assert!(sdu_out.contains("Actual kernel EUID=0"));
}

#[test]
fn editor_buffer_auto_indents_undoes_and_replaces() {
    let mut buf = EditorBuffer::empty("main.dg");
    for ch in "if true".chars() {
        buf.insert_char(ch);
    }
    buf.insert_newline_auto_indent();
    assert_eq!(buf.lines[1], "    ");
    assert_eq!(buf.cursor_col, 4);

    for ch in "say(\"hi\")".chars() {
        buf.insert_char(ch);
    }
    let replaced = buf.replace_all("hi", "dog");
    assert_eq!(replaced, 1);
    assert!(buf.lines[1].contains("dog"));

    assert!(buf.undo());
    assert!(buf.lines[1].contains("hi"));
    assert!(buf.redo());
    assert!(buf.lines[1].contains("dog"));
}

#[test]
fn editor_buffer_enhanced_helpers_autocomplete_hints_and_snippets() {
    let mut buf = EditorBuffer::empty("app.dg");
    assert!(buf.insert_snippet("fn"));
    assert_eq!(buf.lines[0], "fn my_fn(x: number) -> number");
    let outline = buf.extract_outline();
    assert_eq!(outline.len(), 1);
    assert_eq!(outline[0].0, 1);

    buf.cursor_row = 1;
    buf.cursor_col = 4;
    buf.toggle_comment();
    assert_eq!(buf.lines[1], "    # return x");
    buf.toggle_comment();
    assert_eq!(buf.lines[1], "    return x");

    buf.duplicate_line();
    assert_eq!(buf.lines.len(), 4);
    buf.delete_line();
    assert_eq!(buf.lines.len(), 3);

    let mut ac_buf = EditorBuffer::empty("ac.dg");
    for ch in "hmac_s".chars() {
        ac_buf.insert_char_smart(ch);
    }
    assert!(ac_buf.accept_autocomplete());
    assert_eq!(ac_buf.lines[0], "hmac_sha256()");
    assert!(ac_buf.contextual_hint().is_some());
}

#[test]
fn prompt_install_uninstall_and_completions_round_trip() {
    let dir = env::temp_dir().join("dog_prompt_test");
    let _ = fs::create_dir_all(&dir);
    let rc_file = dir.join(".bashrc");
    fs::write(&rc_file, "export FOO=bar\n").unwrap();

    let rc_str = rc_file.display().to_string();
    let msg = handle_prompt(&[
        "install".to_string(),
        "--shell".to_string(),
        "bash".to_string(),
        "--rc".to_string(),
        rc_str.clone(),
    ])
    .expect("prompt install should succeed");
    assert!(msg.contains("Installed DOG terminal prompt"));

    let contents = fs::read_to_string(&rc_file).unwrap();
    assert!(contents.contains("export FOO=bar"));
    assert!(contents.contains("__dog_prompt_cmd"));
    assert!(contents.contains("⩫dog"));

    let rm_msg = handle_prompt(&[
        "uninstall".to_string(),
        "--rc".to_string(),
        rc_str,
    ])
    .expect("prompt uninstall should succeed");
    assert!(rm_msg.contains("Removed DOG terminal prompt"));

    let restored = fs::read_to_string(&rc_file).unwrap();
    assert!(restored.contains("export FOO=bar"));
    assert!(!restored.contains("__dog_prompt_cmd"));

    let zsh_comp = handle_completions(Some("zsh")).unwrap();
    assert!(zsh_comp.contains("#compdef dog"));

    assert_eq!(
        hmac_sha256_hex(b"key", b"The quick brown fox jumps over the lazy dog"),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );

    let _ = fs::remove_dir_all(&dir);
}

struct ArgsMockIo {
    args: Vec<String>,
    output: String,
}

impl InterpreterIo for ArgsMockIo {
    fn write_stdout(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn writeln_stdout(&mut self, text: &str) {
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn read_line(&mut self) -> Result<String, String> {
        Ok(String::new())
    }

    fn script_args(&self) -> Vec<String> {
        self.args.clone()
    }
}

#[test]
fn passes_script_args_to_args_builtin() {
    let mut io = ArgsMockIo {
        args: vec!["alpha".to_string(), "beta".to_string()],
        output: String::new(),
    };
    let src = "let a = args()\nsay(join(a, \":\"))\n";
    run_source_with_io("args.dg", src, &mut io).expect("args() should work");
    assert_eq!(io.output, "alpha:beta\n");
}

#[test]
fn checks_web_server_and_discord_bot_examples() {
    let web_src = include_str!("../examples/web_server.dg");
    check_source("examples/web_server.dg", web_src).expect("web_server.dg must type-check");

    let bot_src = include_str!("../examples/discord_bot.dg");
    check_source("examples/discord_bot.dg", bot_src).expect("discord_bot.dg must type-check");
}

#[test]
fn kv_store_persists_reads_lists_and_deletes() {
    let dir = env::temp_dir().join("dog_kv_test");
    let _ = fs::create_dir_all(&dir);
    let db_path = dir.join("store.json").display().to_string();

    let src = "\
let db = args()[0]
kv_set(db, \"token\", \"abc123\")
kv_set(db, \"mode\", \"local\")
assert(kv_has(db, \"token\"), \"kv_has detects key\")
assert(kv_get(db, \"token\") == \"abc123\", \"kv_get reads value\")
let keys = kv_keys(db)
assert(len(keys) == 2 and list_contains(keys, \"mode\"), \"kv_keys lists keys\")
assert(kv_delete(db, \"token\"), \"kv_delete removes key\")
assert(not kv_has(db, \"token\"), \"removed key stays gone\")
";
    let mut io = ArgsMockIo {
        args: vec![db_path],
        output: String::new(),
    };
    run_source_with_io("kv_test.dg", src, &mut io).expect("kv_store script should pass");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn http_server_and_client_loopback_round_trip() {
    let dir = env::temp_dir().join("dog_http_loopback_test");
    let _ = fs::create_dir_all(&dir);
    let port_file = dir.join("port.txt");
    let html_file = dir.join("hello.html");
    let _ = fs::remove_file(&port_file);
    fs::write(&html_file, "<h1>Static Asset</h1>").unwrap();

    let srv_src = "\
let port_path = args()[0]
let asset_path = args()[1]
let srv = http_listen(\"127.0.0.1:0\")
let p = http_port(srv)
file_write(port_path, text(p))

let r1 = http_accept(srv)
assert(map_get(r1, \"method\") == \"POST\" and map_get(r1, \"path\") == \"/echo\", \"r1 matches\")
http_reply(r1, 200, \"text/plain\", \"echo:\" + map_get(r1, \"body\") + \":\" + map_get(r1, \"query\"))

let r2 = http_accept(srv)
http_serve_file(r2, asset_path)

let r3 = http_accept(srv)
assert(map_get(r3, \"authorization\") == \"Bot secret\", \"custom header forwarded\")
http_reply(r3, 201, \"application/json\", \"{\\\"ok\\\":true}\")
http_close(srv)
";
    let p_arg = port_file.display().to_string();
    let a_arg = html_file.display().to_string();
    let srv_handle = std::thread::spawn(move || {
        let mut io = ArgsMockIo {
            args: vec![p_arg, a_arg],
            output: String::new(),
        };
        run_source_with_io("http_srv.dg", srv_src, &mut io).expect("server script must pass");
    });

    let mut bound_port = String::new();
    for _ in 0..80 {
        if let Ok(s) = fs::read_to_string(&port_file) {
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() {
                bound_port = trimmed;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!bound_port.is_empty(), "server must publish ephemeral port");

    let client_src = "\
let base = \"http://127.0.0.1:\" + args()[0]
let post_res = http_post(base + \"/echo?q=7\", \"hello-dog\", \"text/plain\")
assert(post_res == \"echo:hello-dog:q=7\", \"http_post round-trips\")

let asset = http_get(base + \"/hello.html\")
assert(asset == \"<h1>Static Asset</h1>\", \"http_serve_file serves file\")

var hdrs: map<text, text> = {}
map_set(hdrs, \"Authorization\", \"Bot secret\")
let full = http_request(\"PUT\", base + \"/custom\", hdrs, \"payload\")
assert(map_get(full, \"status\") == \"201\", \"http_request returns status\")
assert(map_get(full, \"body\") == \"{\\\"ok\\\":true}\", \"http_request returns body\")
";
    let mut client_io = ArgsMockIo {
        args: vec![bound_port],
        output: String::new(),
    };
    run_source_with_io("http_client.dg", client_src, &mut client_io)
        .expect("client script must pass");
    srv_handle.join().expect("server thread must join cleanly");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn raw_tcp_sockets_loopback_round_trip() {
    let dir = env::temp_dir().join("dog_tcp_loopback_test");
    let _ = fs::create_dir_all(&dir);
    let port_file = dir.join("tcp_port.txt");
    let _ = fs::remove_file(&port_file);

    let srv_src = "\
let port_path = args()[0]
let ln = tcp_listen(\"127.0.0.1:0\")
file_write(port_path, text(tcp_port(ln)))
let conn = tcp_accept(ln)
let msg = tcp_recv(conn, 1024)
tcp_send(conn, \"pong:\" + msg)
tcp_close(conn)
tcp_close(ln)
";
    let p_arg = port_file.display().to_string();
    let srv_handle = std::thread::spawn(move || {
        let mut io = ArgsMockIo {
            args: vec![p_arg],
            output: String::new(),
        };
        run_source_with_io("tcp_srv.dg", srv_src, &mut io).expect("tcp server script must pass");
    });

    let mut bound_port = String::new();
    for _ in 0..80 {
        if let Ok(s) = fs::read_to_string(&port_file) {
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() {
                bound_port = trimmed;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!bound_port.is_empty(), "tcp listener must publish ephemeral port");

    let client_src = "\
let conn = tcp_connect(\"127.0.0.1:\" + args()[0])
tcp_send(conn, \"ping\")
let reply = tcp_recv(conn, 1024)
assert(reply == \"pong:ping\", \"TCP round-trip matches\")
tcp_close(conn)
";
    let mut client_io = ArgsMockIo {
        args: vec![bound_port],
        output: String::new(),
    };
    run_source_with_io("tcp_client.dg", client_src, &mut client_io)
        .expect("tcp client script must pass");
    srv_handle.join().expect("tcp server thread must join");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn forge_scaffolds_bot_and_web_templates() {
    let dir = env::temp_dir().join("dog_forge_bot_test");
    let _ = fs::remove_dir_all(&dir);
    let target = dir.display().to_string();

    let out = dog_lang::sys::handle_forge(&[
        "new".to_string(),
        target.clone(),
        "--bot".to_string(),
    ])
    .expect("forge --bot should succeed");
    assert!(out.contains("bot template"));

    let main_dg = fs::read_to_string(dir.join("src/main.dg")).unwrap();
    check_source("forged_bot.dg", &main_dg).expect("forged bot template must type-check");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn term_capability_detection_and_fallbacks() {
    use dog_lang::term::TermCapabilities;
    use std::collections::HashMap;

    let mut env_no_color = HashMap::new();
    env_no_color.insert("NO_COLOR", "1");
    env_no_color.insert("LANG", "en_US.UTF-8");
    env_no_color.insert("COLUMNS", "120");
    let caps_no_color = TermCapabilities::detect_with(true, true, |k| {
        env_no_color.get(k).map(|s| (*s).to_string())
    });
    assert!(caps_no_color.is_tty);
    assert!(!caps_no_color.color, "NO_COLOR must disable ANSI color");
    assert!(caps_no_color.unicode);
    assert_eq!(caps_no_color.width, 120);

    let mut env_dumb = HashMap::new();
    env_dumb.insert("TERM", "dumb");
    env_dumb.insert("COLUMNS", "15");
    let caps_dumb = TermCapabilities::detect_with(true, false, |k| {
        env_dumb.get(k).map(|s| (*s).to_string())
    });
    assert!(!caps_dumb.color, "TERM=dumb must disable ANSI color");
    assert!(!caps_dumb.unicode, "TERM=dumb must fall back to ASCII");
    assert_eq!(caps_dumb.width, 40, "width should clamp to minimum 40");

    let mut env_force = HashMap::new();
    env_force.insert("CLICOLOR_FORCE", "1");
    env_force.insert("LANG", "C");
    let caps_force = TermCapabilities::detect_with(false, false, |k| {
        env_force.get(k).map(|s| (*s).to_string())
    });
    assert!(caps_force.color, "CLICOLOR_FORCE should enable color on non-TTY");
    assert!(!caps_force.unicode, "LANG=C must select ASCII fallback");
}

#[test]
fn term_themes_prefixes_timestamps_and_levels() {
    use dog_lang::term::{
        format_utc_datetime, format_utc_hms, DogMessenger, MsgLevel, TermCapabilities,
        TermStyleConfig, TermTheme,
    };

    assert_eq!(format_utc_hms(0), "00:00:00");
    assert_eq!(format_utc_datetime(0), "1970-01-01 00:00:00");
    assert_eq!(TermTheme::parse("kali"), Some(TermTheme::Kali));
    assert_eq!(TermTheme::parse("DRACULA"), Some(TermTheme::Dracula));
    assert_eq!(TermTheme::parse("solarized"), Some(TermTheme::Solar));
    assert_eq!(TermTheme::parse("mono"), Some(TermTheme::Mono));

    let plain_cfg = TermStyleConfig {
        theme: TermTheme::Kali,
        timestamps: true,
        prefix: "kennel".to_string(),
        debug: true,
        ..TermStyleConfig::default()
    };
    let plain_msg = DogMessenger::new(plain_cfg.clone(), TermCapabilities::plain());
    let line = plain_msg.render_line(MsgLevel::Success, "Boot ready", Some(3661));
    assert!(!line.contains('\x1b'), "plain fallback must emit zero ANSI codes");
    assert!(line.contains("[01:01:01]"), "timestamp must format UTC HH:MM:SS");
    assert!(line.contains("[kennel]"), "custom prefix must appear");
    assert!(line.contains("[+ OK] Boot ready"), "ASCII success badge expected: {line}");

    for lvl in [
        MsgLevel::Info,
        MsgLevel::Success,
        MsgLevel::Warning,
        MsgLevel::Error,
        MsgLevel::Debug,
        MsgLevel::Progress,
    ] {
        let rich = DogMessenger::new(plain_cfg.clone(), TermCapabilities::rich());
        let colored = rich.render_line(lvl, "msg", Some(0));
        assert!(colored.contains('\x1b'), "rich Kali theme must include ANSI escapes");
    }

    let mono_cfg = TermStyleConfig {
        theme: TermTheme::Mono,
        ..plain_cfg
    };
    let mono_on_rich = DogMessenger::new(mono_cfg, TermCapabilities::rich());
    assert!(!mono_on_rich.use_color());
    assert!(!mono_on_rich.render_line(MsgLevel::Info, "hi", None).contains('\x1b'));
}

#[test]
fn progress_bar_and_spinner_real_calculations_and_fallbacks() {
    use dog_lang::term::{
        DogMessenger, ProgressBar, ProgressUnit, Spinner, TermCapabilities, TermStyleConfig,
    };

    let ascii_m = DogMessenger::new(TermStyleConfig::default(), TermCapabilities::plain());
    let mut bar = ProgressBar::new("Transfer", 2048, ProgressUnit::Bytes, ascii_m.clone());
    bar.set(1024);
    assert_eq!(bar.percent(), 50);
    let rendered_50 = bar.render();
    assert!(rendered_50.contains(" 50%"), "50% expected: {rendered_50}");
    assert!(rendered_50.contains("1.0 KiB / 2.0 KiB"), "{rendered_50}");
    assert!(rendered_50.contains('#') && rendered_50.contains('-'));

    let empty_bar = ProgressBar::new("Empty", 0, ProgressUnit::Items, ascii_m.clone());
    assert_eq!(empty_bar.percent(), 100);

    let spin_ascii = Spinner::new("Verifying", ascii_m);
    assert_eq!(Spinner::glyph(0, false), "|");
    assert_eq!(Spinner::glyph(1, false), "/");
    assert!(spin_ascii.render_step(2, "sha256").contains("- Verifying — sha256"));

    let rich_m = DogMessenger::new(TermStyleConfig::default(), TermCapabilities::rich());
    let mut rich_bar = ProgressBar::new("Install", 10, ProgressUnit::Items, rich_m);
    rich_bar.set(10);
    let rendered_100 = rich_bar.render();
    assert!(rendered_100.contains("100%") && rendered_100.contains('█'));
}

#[test]
fn config_persists_theme_timestamps_prefix_and_install_style() {
    use dog_lang::config::{DogConfig, Personality};
    use dog_lang::term::{InstallStyle, TermTheme};

    let legacy = DogConfig::parse_str("personality=professional\nruns_completed=9\n");
    assert_eq!(legacy.personality, Personality::Professional);
    assert_eq!(legacy.runs_completed, 9);
    assert_eq!(legacy.theme, TermTheme::Default);
    assert!(!legacy.timestamps);
    assert_eq!(legacy.prefix, "dog");

    let custom = DogConfig {
        personality: Personality::Chaotic,
        runs_completed: 14,
        theme: TermTheme::Kali,
        timestamps: true,
        prefix: "wolf".to_string(),
        debug_logs: false,
        install_style: InstallStyle::Compact,
    };
    let serialized = custom.render();
    let parsed = DogConfig::parse_str(&serialized);
    assert_eq!(parsed.personality, Personality::Chaotic);
    assert_eq!(parsed.runs_completed, 14);
    assert_eq!(parsed.theme, TermTheme::Kali);
    assert!(parsed.timestamps);
    assert_eq!(parsed.prefix, "wolf");
    assert!(!parsed.debug_logs);
    assert_eq!(parsed.install_style, InstallStyle::Compact);
}

#[test]
fn dog_script_cli_builtins_work_through_interpreter_io() {
    let src = "\
cli_theme(\"mono\")
cli_prefix(\"mybot\")
cli_timestamps(true)
cli_info(\"starting\")
cli_success(\"ready\")
cli_warn(\"low disk\")
cli_error(\"bad packet\")
cli_debug(\"trace 1\")
cli_progress(\"Sync\", 3, 4)
cli_spinner(1, \"Indexing\")
";
    let mut io = MockIo::new(vec![]);
    run_source_with_io("cli_api.dg", src, &mut io).expect("cli_* builtins must run");
    assert!(io.output.contains("[mybot]"));
    assert!(io.output.contains("INFO") && io.output.contains("starting"));
    assert!(io.output.contains("OK") && io.output.contains("ready"));
    assert!(io.output.contains("WARN") && io.output.contains("low disk"));
    assert!(io.output.contains("ERR") && io.output.contains("bad packet"));
    assert!(io.output.contains("DEBUG") && io.output.contains("trace 1"));
    assert!(io.output.contains(" 75% (3/4)"), "progress bar missing: {}", io.output);
    assert!(io.output.contains("Indexing"));

    let bad = "cli_theme(\"unknown_theme\")\n";
    let mut bad_io = MockIo::new(vec![]);
    let err = run_source_with_io("bad_theme.dg", bad, &mut bad_io)
        .expect_err("invalid cli_theme must error");
    assert_eq!(err.code, "E030");

    let showcase = include_str!("../examples/cli_experience.dg");
    let mut showcase_io = MockIo::new(vec![]);
    run_source_with_io("examples/cli_experience.dg", showcase, &mut showcase_io)
        .expect("examples/cli_experience.dg must run cleanly");
    assert!(showcase_io.output.contains("[pack-sync]"));
    assert!(showcase_io.output.contains("All stages completed cleanly!"));
}

#[test]
fn hub_package_install_uninstall_with_custom_messages_deps_bytes_and_sha256() {
    use dog_lang::hub::{compute_dir_sha256, install_package_in, remove_package_in};
    use dog_lang::term::{DogMessenger, TermCapabilities, TermStyleConfig};

    let root = env::temp_dir().join("dog_hub_custom_lifecycle_test");
    let _ = fs::remove_dir_all(&root);
    let project_dir = root.join("app");
    let cache_root = root.join("cache");
    let dep_src = root.join("dep_core");
    let pkg_src = root.join("fancy_pkg");

    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(dep_src.join("src")).unwrap();
    fs::create_dir_all(pkg_src.join("src")).unwrap();

    fs::write(
        dep_src.join("dog.pack"),
        "[package]\nname = \"dep_core\"\nversion = \"0.1.0\"\nentry = \"src/lib.dg\"\n",
    )
    .unwrap();
    fs::write(dep_src.join("src/lib.dg"), "fn ping() -> text\n    return \"pong\"\nend\n").unwrap();

    fs::write(
        pkg_src.join("dog.pack"),
        "\
[package]
name = \"fancy_pkg\"
version = \"1.2.0\"
entry = \"src/main.dg\"
description = \"Custom CLI toolkit\"

[dependencies]
dep_core = \"0.1.0\"

[messages]
install_banner = \"Welcome to Fancy Package!\"
pre_install = \"Preparing fancy assets...\"
post_install = \"Run dog run examples/demo.dg to get started!\"
uninstall_banner = \"Saying goodbye to Fancy Package\"
pre_uninstall = \"Cleaning up fancy hooks...\"
post_uninstall = \"Fancy Package cleanly uninstalled.\"
",
    )
    .unwrap();
    fs::write(pkg_src.join("src/main.dg"), "say(\"fancy\")\n").unwrap();

    let messenger = DogMessenger::new(TermStyleConfig::default(), TermCapabilities::plain());

    // Without dep_core in cache, installing fancy_pkg must fail honestly at dependency resolution
    let missing_err = install_package_in(
        &[pkg_src.display().to_string()],
        &project_dir,
        &cache_root,
        &messenger,
        false,
    )
    .expect_err("missing transitive dep must fail");
    assert!(missing_err.contains("dep_core"));

    // Install dep_core first, then fancy_pkg
    install_package_in(
        &[dep_src.display().to_string()],
        &project_dir,
        &cache_root,
        &messenger,
        false,
    )
    .expect("dep_core install should succeed");

    let out = install_package_in(
        &[pkg_src.display().to_string()],
        &project_dir,
        &cache_root,
        &messenger,
        false,
    )
    .expect("fancy_pkg install should succeed");

    let real_sha = compute_dir_sha256(&cache_root.join("fancy_pkg")).unwrap();
    assert!(out.contains("Welcome to Fancy Package!"));
    assert!(out.contains("Preparing fancy assets..."));
    assert!(out.contains("Resolved dep_core@0.1.0 (cached)"));
    assert!(out.contains("100%"), "real 100% progress expected:\n{out}");
    assert!(out.contains(&real_sha), "real SHA-256 must appear:\n{out}");
    assert!(out.contains("Run dog run examples/demo.dg to get started!"));
    assert!(out.contains("DOG Package Installed"));

    let lock = fs::read_to_string(project_dir.join("dog.lock")).unwrap();
    assert!(lock.contains(&real_sha), "dog.lock must record verified tree SHA-256");

    let rm_out = remove_package_in(
        &["fancy_pkg".to_string(), "--purge".to_string()],
        &project_dir,
        &cache_root,
        &messenger,
        false,
    )
    .expect("uninstall should succeed");
    assert!(rm_out.contains("Saying goodbye to Fancy Package"));
    assert!(rm_out.contains("Cleaning up fancy hooks..."));
    assert!(rm_out.contains("Fancy Package cleanly uninstalled."));
    assert!(rm_out.contains("purged "));
    assert!(!cache_root.join("fancy_pkg").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hub_remote_http_package_download_real_loopback() {
    use dog_lang::crypto::sha256_hex;
    use dog_lang::hub::install_package_in;
    use dog_lang::term::{DogMessenger, TermCapabilities, TermStyleConfig};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let root = env::temp_dir().join("dog_hub_http_download_test");
    let _ = fs::remove_dir_all(&root);
    let project_dir = root.join("app");
    let cache_root = root.join("cache");
    fs::create_dir_all(&project_dir).unwrap();

    let payload = "fn hello_net() -> text\n    return \"fetched\"\nend\n";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let url = format!("http://127.0.0.1:{port}/remote_util.dg");
    let messenger = DogMessenger::new(TermStyleConfig::default(), TermCapabilities::plain());
    let out = install_package_in(&[url], &project_dir, &cache_root, &messenger, false)
        .expect("HTTP package download should succeed");
    server.join().unwrap();

    assert!(out.contains("remote_util"));
    assert!(out.contains("100%"));
    assert!(out.contains("DOG Package Installed"));
    let saved = fs::read_to_string(cache_root.join("remote_util/src/remote_util.dg")).unwrap();
    assert_eq!(saved, payload);
    assert_eq!(sha256_hex(saved.as_bytes()), sha256_hex(payload.as_bytes()));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn did_you_mean_levenshtein_diagnostics() {
    let var_err = check_source("typo_var.dg", "let counter = 1\nsay(contre)\n")
        .expect_err("unknown var must fail");
    assert_eq!(var_err.code, "E010");

    let var_err2 = check_source("typo_var2.dg", "let counter = 1\nsay(countr)\n")
        .expect_err("1-edit typo must fail with hint");
    assert_eq!(var_err2.code, "E010");
    assert!(
        var_err2.fix.contains("Did you mean 'counter'?"),
        "expected Did you mean hint, got: {}",
        var_err2.fix
    );

    let fn_err = check_source("typo_fn.dg", "http_listn(\"127.0.0.1:0\")\n")
        .expect_err("typo builtin must emit E013 hint");
    assert_eq!(fn_err.code, "E013");
    assert!(
        fn_err.fix.contains("http_listen"),
        "expected http_listen suggestion, got: {}",
        fn_err.fix
    );

    let ty_err = check_source("typo_ty.dg", "let x: numbr = 5\n")
        .expect_err("typo type must emit E004 hint");
    assert_eq!(ty_err.code, "E004");
    assert!(
        ty_err.fix.contains("number"),
        "expected number suggestion, got: {}",
        ty_err.fix
    );
}

#[test]
fn http_chunked_redirect_try_accept_and_headers_round_trip() {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let dir = env::temp_dir().join("dog_http_v2_test");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    let port_file = dir.join("port.txt");

    let srv_src = "\
let port_path = args()[0]
let srv = http_listen(\"127.0.0.1:0\")
file_write(port_path, text(http_port(srv)))

# Non-blocking poll returns _ok = \"false\" when idle
let idle = http_try_accept(srv, 40)
assert(map_get(idle, \"_ok\") == \"false\", \"idle try_accept times out cleanly\")

# Req 1: chunked POST
let r1 = http_accept(srv)
assert(map_get(r1, \"body\") == \"HelloChunkedWorld\", \"server decodes Transfer-Encoding: chunked body\")
http_reply(r1, 200, \"text/plain\", \"chunk-ok\")

# Req 2: 302 Redirect -> Req 3: Final target with custom header
let r2 = http_accept(srv)
assert(map_get(r2, \"path\") == \"/go\", \"redirect entry hit\")
http_redirect(r2, 302, \"/dest\")

let r3 = http_accept(srv)
assert(map_get(r3, \"path\") == \"/dest\", \"redirect followed to /dest\")
var hdrs: map<text, text> = {}
map_set(hdrs, \"X-Dog-Version\", \"0.2.0\")
map_set(hdrs, \"Content-Type\", \"text/plain\")
http_reply_headers(r3, 200, hdrs, \"arrived\")
http_close(srv)
";
    let p_arg = port_file.display().to_string();
    let srv_handle = std::thread::spawn(move || {
        let mut io = ArgsMockIo {
            args: vec![p_arg],
            output: String::new(),
        };
        run_source_with_io("http_v2_srv.dg", srv_src, &mut io)
            .expect("http v2 server script must pass");
    });

    let mut bound_port = String::new();
    for _ in 0..80 {
        if let Ok(s) = fs::read_to_string(&port_file) {
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() {
                bound_port = trimmed;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!bound_port.is_empty());

    // Wait briefly so idle http_try_accept finishes before Req 1 arrives
    std::thread::sleep(std::time::Duration::from_millis(70));

    // Send raw Transfer-Encoding: chunked request
    let mut stream = TcpStream::connect(format!("127.0.0.1:{bound_port}")).unwrap();
    let raw_chunked = concat!(
        "POST /chunk HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "Connection: close\r\n\r\n",
        "5\r\nHello\r\n",
        "7\r\nChunked\r\n",
        "5\r\nWorld\r\n",
        "0\r\n\r\n"
    );
    stream.write_all(raw_chunked.as_bytes()).unwrap();
    let mut raw_resp = String::new();
    stream.read_to_string(&mut raw_resp).unwrap();
    assert!(raw_resp.contains("chunk-ok"));

    // Now test DOG client following 302 redirect + exposing response headers & final_url
    let client_src = "\
let base = $\"http://127.0.0.1:{args()[0]}\"
var hdrs: map<text, text> = {}
let res = http_request_timeout(\"GET\", $\"{base}/go\", hdrs, \"\", 5000)
assert(map_get(res, \"status\") == \"200\", \"redirect followed to 200\")
assert(map_get(res, \"ok\") == \"true\", \"ok field reflects 2xx status\")
assert(map_get(res, \"body\") == \"arrived\", \"final body matches\")
assert(contains(map_get(res, \"final_url\"), \"/dest\"), \"final_url records redirected URL\")
let resp_hdrs = json_parse(map_get(res, \"headers\"))
assert(map_get(resp_hdrs, \"x-dog-version\") == \"0.2.0\", \"custom response headers captured\")
";
    let mut client_io = ArgsMockIo {
        args: vec![bound_port],
        output: String::new(),
    };
    run_source_with_io("http_v2_client.dg", client_src, &mut client_io)
        .expect("http v2 client script must pass");
    srv_handle.join().unwrap();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn websocket_rfc6455_loopback_round_trip() {
    let dir = env::temp_dir().join("dog_ws_loopback_test");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    let port_file = dir.join("ws_port.txt");

    let srv_src = "\
let srv = http_listen(\"127.0.0.1:0\")
file_write(args()[0], text(http_port(srv)))
let req = http_accept(srv)
let ws = ws_upgrade(req)
let msg = ws_recv(ws)
ws_send(ws, $\"ws-echo:{msg}\")
ws_close(ws)
http_close(srv)
";
    let p_arg = port_file.display().to_string();
    let srv_handle = std::thread::spawn(move || {
        let mut io = ArgsMockIo {
            args: vec![p_arg],
            output: String::new(),
        };
        run_source_with_io("ws_srv.dg", srv_src, &mut io).expect("ws server must pass");
    });

    let mut bound_port = String::new();
    for _ in 0..80 {
        if let Ok(s) = fs::read_to_string(&port_file) {
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() {
                bound_port = trimmed;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!bound_port.is_empty());

    let client_src = "\
let ws = ws_connect($\"ws://127.0.0.1:{args()[0]}/chat\")
ws_send(ws, \"bark-socket\")
let reply = ws_recv(ws)
assert(reply == \"ws-echo:bark-socket\", \"RFC 6455 WebSocket loopback round-trip\")
ws_close(ws)
";
    let mut client_io = ArgsMockIo {
        args: vec![bound_port],
        output: String::new(),
    };
    run_source_with_io("ws_client.dg", client_src, &mut client_io)
        .expect("ws client must pass");
    srv_handle.join().unwrap();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn udp_dns_port_probe_and_file_helpers_work() {
    let dir = env::temp_dir().join("dog_udp_fs_test");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);

    let src = "\
let base = args()[0]
let f1 = path_join(base, \"alpha.txt\")
let f2 = path_join(base, \"beta.txt\")
file_write(f1, \"hello-fs\")
file_copy(f1, f2)
assert(is_file(f2) and is_dir(base) and file_size(f2) == 8, \"file helpers work\")

let addrs = dns_lookup(\"127.0.0.1\")
assert(len(addrs) >= 1 and list_contains(addrs, \"127.0.0.1\"), \"dns_lookup works\")
assert(not port_is_open(\"127.0.0.1:9\", 100), \"closed port probe returns false fast\")

let s1 = udp_bind(\"127.0.0.1:0\")
let p1 = tcp_port(s1)
let s2 = udp_bind(\"127.0.0.1:0\")
udp_send_to(s2, $\"127.0.0.1:{p1}\", \"udp-bark\")
let pkt = udp_recv_from(s1, 512)
assert(map_get(pkt, \"data\") == \"udp-bark\", \"UDP loopback round-trip matches\")
assert(contains(map_get(pkt, \"remote\"), \"127.0.0.1:\"), \"UDP packet reports sender addr\")
udp_close(s2)
udp_close(s1)
";
    let mut io = ArgsMockIo {
        args: vec![dir.display().to_string()],
        output: String::new(),
    };
    run_source_with_io("udp_fs.dg", src, &mut io).expect("udp + fs helpers must pass");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn editor_buffer_live_intellisense_dot_fuzzy_ghost_and_dedent() {
    let mut buf = EditorBuffer::empty("bot.dg");
    buf.lines = vec![
        "enum Mode".to_string(),
        "    Fast".to_string(),
        "    Safe".to_string(),
        "end".to_string(),
        "struct Config".to_string(),
        "    port: number".to_string(),
        "    host: text".to_string(),
        "end".to_string(),
        "let cfg: Config = Config { port: 8080, host: \"127.0.0.1\" }".to_string(),
        "cfg.p".to_string(),
    ];
    buf.cursor_row = 9;
    buf.cursor_col = 5;

    let field_sugs = buf.ranked_suggestions(false);
    assert!(
        field_sugs.iter().any(|s| s.label == "port" && s.kind == "fld"),
        "expected struct field suggestion for cfg.p, got: {:?}",
        field_sugs
    );
    let ghost = buf.ghost_completion_for(&field_sugs[0]).unwrap();
    assert_eq!(ghost, "ort");
    assert!(buf.accept_suggestion_item(&field_sugs[0]));
    assert_eq!(buf.lines[9], "cfg.port");

    // Enum dot completion with empty partial after '.'
    buf.lines.push("Mode.".to_string());
    buf.cursor_row = 10;
    buf.cursor_col = 5;
    let enum_sugs = buf.ranked_suggestions(false);
    assert!(enum_sugs.iter().any(|s| s.label == "Fast" && s.kind == "enum"));
    assert!(enum_sugs.iter().any(|s| s.label == "Safe" && s.kind == "enum"));

    // Fuzzy subsequence completion ("hl" -> "http_listen(")
    buf.lines.push("htl".to_string());
    buf.cursor_row = 11;
    buf.cursor_col = 3;
    let fuzzy = buf.ranked_suggestions(false);
    assert!(
        fuzzy.iter().any(|s| s.insert == "http_listen("),
        "fuzzy htl should surface http_listen(, got: {:?}",
        fuzzy
    );

    // Smart auto-dedent on closing 'end' inside indented block
    buf.lines = vec!["if true".to_string(), "    ".to_string()];
    buf.cursor_row = 1;
    buf.cursor_col = 4;
    for ch in "end".chars() {
        buf.insert_char_smart(ch);
    }
    assert_eq!(buf.lines[1], "end");
}

#[test]
fn mobile_editor_36col_soft_wrap_never_overflows_screen() {
    let mut buf = EditorBuffer::empty("very_long_mobile_discord_bot_handler_name.dg");
    buf.lines = vec![
        "\tlet message = \"This is a very long line designed to exceed 36 terminal columns on Android Termux screens\"".to_string(),
        "let extra: number = \"bad_type\"".to_string(),
        "say(message)".to_string(),
    ];
    buf.cursor_row = 0;
    buf.cursor_col = 45;
    buf.last_match_line = Some(0);

    let tabs = vec![buf];
    let diag = check_source(
        "mobile.dg",
        "let extra: number = \"bad_type\"\n",
    )
    .err();

    let sugs = tabs[0].ranked_suggestions(true);
    let cols = 36usize;
    let rows = 18usize;

    // Code mode with soft-wrap ON and error diagnostic banner
    let (frame, cr, cc) = render_editor_frame_rows(
        &tabs,
        0,
        diag.as_ref(),
        Some("main*"),
        "Soft-wrapped long line status message that should safely truncate",
        false,
        None,
        0,
        true,
        &sugs,
        0,
        rows,
        cols,
    );
    assert_eq!(frame.len(), rows);
    for (idx, line) in frame.iter().enumerate() {
        let vis = visible_ansi_len(line);
        assert!(
            vis <= cols,
            "row {idx} overflowed {cols} cols (got {vis}): {line:?}"
        );
    }
    assert!((1..=rows).contains(&cr), "cursor row {cr} out of bounds");
    assert!((1..=cols).contains(&cc), "cursor col {cc} out of bounds");

    // Help screen mode at 36 cols
    let (help_frame, _, _) = render_editor_frame_rows(
        &tabs,
        0,
        None,
        None,
        "Help",
        true,
        None,
        0,
        true,
        &[],
        0,
        rows,
        cols,
    );
    for (idx, line) in help_frame.iter().enumerate() {
        let vis = visible_ansi_len(line);
        assert!(
            vis <= cols,
            "help row {idx} overflowed {cols} cols (got {vis})"
        );
    }

    // Long run output overlay scrolled at 36 cols
    let long_out = "Line 1: started local bot listener on 127.0.0.1:8080\nLine 2: connected gateway heartbeat ok\nLine 3: dispatched command payload successfully\n";
    let (out_frame, _, _) = render_editor_frame_rows(
        &tabs,
        0,
        None,
        None,
        "Output",
        false,
        Some(long_out),
        1,
        true,
        &[],
        0,
        rows,
        cols,
    );
    for (idx, line) in out_frame.iter().enumerate() {
        let vis = visible_ansi_len(line);
        assert!(
            vis <= cols,
            "output row {idx} overflowed {cols} cols (got {vis})"
        );
    }
}

#[test]
fn lexer_triple_strings_hex_bin_underscores_and_formatter_preservation() {
    let src = "\
let n = 1_000_000
let h = 0xFF_00
let b = 0b1010_0101
let d = 3.14_15
let txt = \"\"\"
line one
  indented raw line
\"\"\"
assert(n == 1000000 and h == 65280 and b == 165)
assert(d > 3.14 and contains(txt, \"  indented raw line\"))
";
    let tokens = Lexer::new("lit.dg", src).tokenize().expect("tokenize ok");
    assert!(!tokens.is_empty());

    let mut io = MockIo::new(vec![]);
    run_source_with_io("lit.dg", src, &mut io).expect("run triple string & numeric literals");

    let formatted = format_source(src);
    assert!(
        formatted.contains("  indented raw line"),
        "formatter must preserve interior indentation inside triple-quoted strings:\n{formatted}"
    );
}

#[test]
fn crypto_and_network_upgrades_behave_accurately() {
    // NIST SHA-512("abc")
    assert_eq!(
        sha512_hex(b"abc"),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );

    let sig = hmac_sha256_hex(b"key", b"message");
    assert!(hmac_sha256_verify(b"key", b"message", &sig));
    assert!(!hmac_sha256_verify(b"key", b"message", "deadbeef"));
    assert!(constant_time_eq(b"dog123", b"dog123"));
    assert!(!constant_time_eq(b"dog123", b"dog124"));

    let encoded = base64_url_encode(b"dog?lang=rust+std/1");
    assert!(!encoded.contains('=') && !encoded.contains('+') && !encoded.contains('/'));
    assert_eq!(base64_url_decode(&encoded).unwrap(), b"dog?lang=rust+std/1");

    let rnd = rand_bytes_hex(16);
    assert_eq!(rnd.len(), 32);
    assert!(rnd.chars().all(|c| c.is_ascii_hexdigit()));

    assert_eq!(http_status_text(200), "OK");
    assert_eq!(http_status_text(404), "Not Found");
    assert_eq!(http_status_text(418), "I'm a teapot");
    assert!(!local_ip().is_empty());
}

#[test]
fn cli_eval_lint_bench_and_docs_builtin_lookup_work() {
    let eval_out = eval_snippet_captured("0x10 + 0b0100 + 1_000", Personality::Normal)
        .expect("eval should print numeric result");
    assert_eq!(eval_out.trim(), "1020");

    let dir = env::temp_dir().join("dog_eval_lint_bench_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("sample.dg");
    fs::write(
        &script,
        "let unused_x = 123\nfn greet(unused_p: text) -> text\n    return \"hi\"\nend\ntry\n    let y = greet(\"dog\")\n    say(y)\ncatch _e\nend\n",
    )
    .unwrap();

    let (lint_report, warn_count) =
        handle_lint(&script.display().to_string(), false).expect("lint succeeds");
    assert!(warn_count >= 2, "expected lint warnings:\n{lint_report}");
    assert!(lint_report.contains("[L005]") && lint_report.contains("[L006]"));

    let clean_script = dir.join("clean.dg");
    fs::write(&clean_script, "let x = 2 + 3\nassert(x == 5)\n").unwrap();
    let (clean_report, clean_warns) =
        handle_lint(&clean_script.display().to_string(), true).expect("clean lint ok");
    assert_eq!(clean_warns, 0, "clean script had warnings:\n{clean_report}");

    let messenger = DogMessenger::new(TermStyleConfig::default(), TermCapabilities::plain());
    let bench_report = handle_bench(&clean_script.display().to_string(), 5, &messenger, false)
        .expect("bench succeeds");
    assert!(bench_report.contains("5 runs") && bench_report.contains("Median"));

    let docs_ws = handle_docs(Some("ws_ping"));
    assert!(docs_ws.contains("ws_ping("));
    let docs_sha = handle_docs(Some("sha512"));
    assert!(docs_sha.contains("sha512("));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stdlib_file_move_rmdir_and_collections_work() {
    let dir = env::temp_dir().join("dog_file_move_rmdir_test");
    let _ = fs::remove_dir_all(&dir);
    let sub = dir.join("nested");
    fs::create_dir_all(&sub).unwrap();

    let src_dg = format!(
        "\
let dir = args()[0]
let sub = args()[1]
let a = path_join(dir, \"a.txt\")
let b = path_join(sub, \"b.txt\")
file_write(a, \"moved payload\")
file_move(a, b)
assert(not file_exists(a) and file_read(b) == \"moved payload\")
rmdir(sub)
assert(not is_dir(sub))
"
    );
    let mut io = ArgsMockIo {
        args: vec![dir.display().to_string(), sub.display().to_string()],
        output: String::new(),
    };
    run_source_with_io("fs_move.dg", &src_dg, &mut io).expect("file_move and rmdir pass");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn interactive_io_ask_confirm_choose_password_read_key_table_progress_clear() {
    let src = "\
let name = ask(\"Your name? \")
let ok = confirm(\"Deploy now\")
let target = choose(\"Pick target\", [\"dev\", \"staging\", \"prod\"])
let default_target = choose(\"Default pick\", [\"alpha\", \"beta\"])
let secret = password(\"PIN: \")
let key = read_key()
progress(\"Setup\", 2, 4)
table([\"Field\", \"Value\"], [[\"name\", name], [\"target\", target]])
clear()
var xs: list<number> = [1, 2, 3]
clear(xs)
assert(name == \"Kaydan\")
assert(ok == true)
assert(target == \"prod\")
assert(default_target == \"alpha\")
assert(secret == \"s3cr3t\")
assert(key == \"q\")
assert(len(xs) == 0)
";
    let mut io = MockIo::new(vec!["Kaydan", "yes", "3", "", "s3cr3t", "q"]);
    run_source_with_io("interactive.dg", src, &mut io).expect("interactive builtins pass");
    assert!(io.output.contains("Your name? "));
    assert!(io.output.contains("Deploy now (y/n): "));
    assert!(io.output.contains("3) prod"));
    assert!(io.output.contains("| Field"));
    assert!(io.output.contains("\x1b[2J\x1b[H"));

    let bad_choice = "let x = choose(\"Pick\", [\"one\"])\n";
    let mut bad_io = MockIo::new(vec!["9"]);
    let err = run_source_with_io("bad_choice.dg", bad_choice, &mut bad_io)
        .expect_err("out of range choose() must fail with E030");
    assert_eq!(err.code, "E030");
}

#[test]
fn modular_stdlib_namespaces_bundling_and_suggestions_work() {
    let dir = env::temp_dir().join("dog_modular_std_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let script = dir.join("app.dg");
    let src = "\
use \"std.io\"
use \"std.text\"
use \"std.math\"
use \"std.crypto\"
use \"std.collections\"

let upper_name = std.text.upper(\"dog-pack\")
let g = std.math.gcd(42, 18)
let digest = std.crypto.sha256(\"abc\")
var items: list<number> = [3, 1, 2]
std.collections.sort(items)
std.io.say($\"{upper_name}:{g}:{items[0]}:{slice(digest, 0, 6)}\")
";
    fs::write(&script, src).unwrap();

    let mut io = MockIo::new(vec![]);
    run_source_with_io(&script.display().to_string(), src, &mut io)
        .expect("std.* namespaces execute cleanly");
    assert_eq!(io.output, "DOG-PACK:6:1:ba7816\n");

    let (bundle_path, bundle) =
        build_bundle(&script.display().to_string(), None).expect("bundle with std.* builds");
    let decoded = decode_bundle(&fs::read(&bundle_path).unwrap()).unwrap();
    assert_eq!(decoded.sha256, bundle.sha256);

    let mut bundle_io = MockIo::new(vec![]);
    run_bundle_with_io(&bundle_path.display().to_string(), &mut bundle_io)
        .expect("bundled std.* script runs");
    assert_eq!(bundle_io.output, "DOG-PACK:6:1:ba7816\n");

    let bad_mod = "use \"std.tex\"\n";
    let diag_mod = check_source("bad_mod.dg", bad_mod).expect_err("unknown std module fails");
    assert_eq!(diag_mod.code, "E040");
    assert!(diag_mod.fix.contains("std.text"));

    let bad_fn = "let x = std.text.uppre(\"hi\")\n";
    let diag_fn = check_source("bad_fn.dg", bad_fn).expect_err("unknown std export fails");
    assert_eq!(diag_fn.code, "E013");
    assert!(diag_fn.fix.contains("std.text.upper"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn default_function_parameters_checker_interpreter_and_native_transpile() {
    let src = "\
use \"std.text\"

fn format_tag(label: text, prefix: text = \"[\", suffix: text = \"]\") -> text
    return prefix + std.text.upper(label) + suffix
end

assert(format_tag(\"dog\") == \"[DOG]\")
assert(format_tag(\"dog\", \"<\") == \"<DOG]\")
assert(format_tag(\"dog\", \"(\", \")\") == \"(DOG)\")
";
    let mut io = MockIo::new(vec![]);
    run_source_with_io("defaults.dg", src, &mut io).expect("default params execute");

    let prog = resolve_program("defaults.dg", src).unwrap();
    let rs = transpile_program_to_rust(&prog).expect("transpiles defaults and std.* to Rust");
    assert!(rs.contains("format_tag("));
    assert!(rs.contains(".to_uppercase()"));

    let bad_order = "\
fn broken(a: text = \"x\", b: text) -> text
    return a + b
end
";
    let err_order = check_source("bad_order.dg", bad_order)
        .expect_err("required param after default param must fail");
    assert_eq!(err_order.code, "E004");

    let too_few = "\
fn greet(name: text, hi: text = \"yo\") -> text
    return hi + name
end
let x = greet()
";
    let err_arity = check_source("too_few.dg", too_few).expect_err("missing required arg fails");
    assert_eq!(err_arity.code, "E014");
}

#[test]
fn runtime_stack_traces_capture_nested_calls() {
    let src = "\
fn inner(v: number) -> number
    assert(v > 0, \"positive only\")
    return v
end

fn outer(v: number) -> number
    return inner(v)
end

let r = outer(-5)
say(r)
";
    let mut io = MockIo::new(vec![]);
    let diag = run_source_with_io("trace.dg", src, &mut io)
        .expect_err("assertion inside nested call must fail with stack trace");
    assert_eq!(diag.code, "E033");
    assert!(
        diag.stack_trace.iter().any(|f| f.contains("inner"))
            && diag.stack_trace.iter().any(|f| f.contains("outer")),
        "expected inner and outer frames in {:?}",
        diag.stack_trace
    );
    let rendered = diag.format_with(Personality::Professional, Some(src));
    assert!(rendered.contains("STACK TRACE:"));
    assert!(rendered.contains("outer"));
    assert!(rendered.contains("inner"));
}

#[test]
fn data_time_env_and_cookie_builtins_work() {
    let dir = env::temp_dir().join("dog_030_data_builtins");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let json_path = dir.join("cfg.json");
    let env_path = dir.join(".env");
    fs::write(
        &env_path,
        "# sample dotenv\nDOG_TEST_ENV_KEY=\"bark_99\"\nexport DOG_TEST_ENV_PORT=9090\n",
    )
    .unwrap();

    let src = "\
let jpath = args()[0]
let epath = args()[1]
var data: map<text, text> = {}
map_set(data, \"lang\", \"dog\")
map_set(data, \"ver\", \"0.3.0\")
json_write(jpath, data)
let loaded_json = json_read(jpath)
assert(map_get(loaded_json, \"lang\") == \"dog\" and map_get(loaded_json, \"ver\") == \"0.3.0\")

let toml = toml_parse(\"name = 'dog'\\n[net]\\nhost = \\\"0.0.0.0\\\" # bind all\\nport = 3000\\n\")
assert(map_get(toml, \"name\") == \"dog\" and map_get(toml, \"net.host\") == \"0.0.0.0\" and map_get(toml, \"net.port\") == \"3000\")

let n_env = env_load(epath)
assert(n_env == 2)
assert(env_get(\"DOG_TEST_ENV_KEY\") == \"bark_99\" and env_get(\"DOG_TEST_ENV_PORT\") == \"9090\")

assert(time_format(0, \"%F %T\") == \"1970-01-01 00:00:00\")
let ck = cookie_parse(\"sid=xyz789; mode=dark\")
assert(map_get(ck, \"sid\") == \"xyz789\" and map_get(ck, \"mode\") == \"dark\")
";
    let mut io = ArgsMockIo {
        args: vec![
            json_path.display().to_string(),
            env_path.display().to_string(),
        ],
        output: String::new(),
    };
    run_source_with_io("data030.dg", src, &mut io).expect("0.3.0 data/env/time builtins pass");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn editor_std_intellisense_and_builtin_help_parity_hold() {
    for &b in builtin_names() {
        assert!(
            builtin_signature_help(b).is_some(),
            "built-in '{b}' is missing an arm in editor::builtin_signature_help"
        );
    }
    for m in STD_MODULES {
        for &(member, canonical) in m.exports {
            let resolved = resolve_std_call(m.name, member)
                .unwrap_or_else(|e| panic!("failed resolving {}.{}: {e}", m.name, member));
            assert_eq!(resolved, canonical);
            assert!(
                builtin_names().contains(&canonical),
                "std export {}.{} -> {} is not in BUILTIN_NAMES",
                m.name,
                member,
                canonical
            );
            assert!(
                builtin_signature_help(canonical).is_some(),
                "std export {}.{} -> {} has no signature help",
                m.name,
                member,
                canonical
            );
        }
    }

    let mut buf = EditorBuffer::empty("intellisense.dg");
    buf.lines = vec!["let s = std.".to_string()];
    buf.cursor_row = 0;
    buf.cursor_col = buf.lines[0].chars().count();
    let mod_sugs = buf.ranked_suggestions(true);
    assert!(mod_sugs.iter().any(|s| s.label == "std.io" && s.kind == "mod"));

    buf.lines = vec!["let s = std.io.as".to_string()];
    buf.cursor_col = buf.lines[0].chars().count();
    let fn_sugs = buf.ranked_suggestions(false);
    assert!(fn_sugs.iter().any(|s| s.label == "ask()" && s.kind == "fn"));

    buf.lines = vec!["std.text.upper(".to_string()];
    buf.cursor_col = buf.lines[0].chars().count();
    let hint = buf.contextual_hint().expect("contextual hint should resolve std.text.upper");
    assert!(hint.contains("upper("));
}

#[test]
fn devtools_debug_coverage_graph_explain_watch_and_docs_work() {
    let dir = env::temp_dir().join("dog_030_devtools_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let helper_path = dir.join("helper.dg");
    fs::write(
        &helper_path,
        "fn double_it(n: number) -> number\n    return n * 2\nend\n",
    )
    .unwrap();

    let main_path = dir.join("main_test.dg");
    let main_src = "\
use \"helper.dg\"
use \"std.math\"
var x = 10
let y = double_it(x)
let z = std.math.gcd(y, 12)
assert(z == 4)
say(z)
";
    fs::write(&main_path, main_src).unwrap();

    // Debugger: breakpoint + print vars + step + continue
    let dbg_cmds = b"b 5\nc\np x\nn\nq\n";
    let mut cmd_in = Cursor::new(&dbg_cmds[..]);
    let mut cmd_out = Vec::new();
    let mut io = MockIo::new(vec![]);
    run_debug_with_streams(
        &main_path.display().to_string(),
        main_src,
        Personality::Normal,
        &mut io,
        &mut cmd_in,
        &mut cmd_out,
    )
    .expect("debugger runs cleanly");
    let dbg_log = String::from_utf8(cmd_out).unwrap();
    assert!(dbg_log.contains("Set breakpoint at line 5"));
    assert!(dbg_log.contains("x = 10"));
    assert!(dbg_log.contains("Debugger aborted execution."));

    // Coverage report
    let cov = run_coverage(Some(&main_path.display().to_string()), Personality::Normal)
        .expect("coverage succeeds");
    assert_eq!(cov.tests_passed, 1);
    assert_eq!(cov.tests_failed, 0);
    assert!(cov.total_covered > 0 && cov.percent > 50.0);
    assert!(cov.report_text.contains("DOG Coverage Report"));

    // Dependency watcher & graph
    let watched = collect_watched_files(&main_path.display().to_string());
    assert_eq!(watched.len(), 2);

    let graph = handle_graph(Some(&main_path.display().to_string())).expect("graph succeeds");
    assert!(graph.contains("use \"helper.dg\""));
    assert!(graph.contains("std.math"));

    // Explain (error code & .dg file)
    let exp_err = handle_explain("E021").expect("explain E021 succeeds");
    assert!(exp_err.contains("Strict type mismatch"));
    let exp_file = handle_explain(&main_path.display().to_string()).expect("explain file succeeds");
    assert!(exp_file.contains("std.math"));

    // Docs std.*
    let docs_std = handle_docs(Some("std"));
    assert!(docs_std.contains("std.io") && docs_std.contains("std.net"));
    let docs_io = handle_docs(Some("std.io"));
    assert!(docs_io.contains("std.io.ask") && docs_io.contains("std.io.confirm"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn english_like_syntax_control_flow_loops_operators_and_transpile() {
    let src = "\
struct Counter
    total: number
end

define describe(n: number, suffix: text = \"!\") returns text
    if n is greater than 10 then
        return $\"big{suffix}\"
    elif n is at least 5 then
        return $\"mid{suffix}\"
    else if n is over 0 then
        return $\"small{suffix}\"
    else
        return $\"zero{suffix}\"
    end
end

function helper() returns number
    return 7
end

var c = Counter { total: 1 }
set c.total to 2
var nums: list<number> = [10, 20]
set nums[0] to 15

var score = 0
set score to 3
increase score by 4
decrease score by 2

until score is at least 7
    increase score by 1
end

var flag = false
unless flag then
    repeat 2 times
        increase score by 1
    end
end

for step from 1 to 3
    increase score by step
end

for each v in nums
    if v is equal to 15 or v is not equal to 99 then
        increase c.total by 1
    end
end

var word = \"\"
match score
    case 0
        set word to \"zero\"
    else
        set word to \"ready\"
end
let from = 1
let to = 2
let times = 3
let then = 4
assert(from + to + times + then is 10)
assert(score is 15 and c.total is 4 and word is \"ready\")
assert(helper() is 7 and 3 is under 4 and 4 is at most 4 and 2 is less than 3 and 5 equals 5)
print(describe(score))
std.io.print(describe(3, \".\"))
";
    let mut io = MockIo::new(vec![]);
    run_source_with_io("english_all.dg", src, &mut io).expect("English-style DOG syntax runs");
    assert_eq!(io.output, "big!\nsmall.\n");

    let prog = resolve_program("english_all.dg", src).unwrap();
    let rs = transpile_program_to_rust(&prog).expect("English AST lowers cleanly to Rust");
    assert!(rs.contains("fn describe("));
    assert!(rs.contains("println!"));

    let messy = "define f(x: number) returns text\nif x is 1 then\nprint(\"one\")\nelse\nprint(\"other\")\nend\nend\n";
    let formatted = format_source(messy);
    assert!(formatted.contains("    if x is 1 then\n        print(\"one\")\n    else\n        print(\"other\")\n    end"));

    let bad_bang = check_source("bang.dg", "let x = !true\n").expect_err("! gives English hint");
    assert_eq!(bad_bang.code, "E001");
    assert!(bad_bang.fix.contains("not"));

    let bad_amp = check_source("amp.dg", "let x = true && false\n").expect_err("&& gives English hint");
    assert_eq!(bad_amp.code, "E001");
    assert!(bad_amp.fix.contains("and"));

    let bad_pipe = check_source("pipe.dg", "let x = true || false\n").expect_err("|| gives English hint");
    assert_eq!(bad_pipe.code, "E001");
    assert!(bad_pipe.fix.contains("or"));

    let example_src =
        fs::read_to_string("examples/english_basics.dg").expect("read examples/english_basics.dg");
    let mut ex_io = MockIo::new(vec![]);
    run_source_with_io("examples/english_basics.dg", &example_src, &mut ex_io)
        .expect("examples/english_basics.dg executes cleanly");
    assert!(ex_io.output.contains("Hello, Friend Alex!"));
    assert!(ex_io.output.contains("Done after 2 retries."));
}

#[test]
fn dog_learn_lessons_and_english_docs_work() {
    let menu = handle_learn(None, false, Personality::Normal).expect("learn menu succeeds");
    assert!(menu.contains("basics") && menu.contains("english") && menu.contains("--run"));

    for lesson in ["basics", "english", "input", "functions", "data", "errors"] {
        let out = handle_learn(Some(lesson), true, Personality::Normal)
            .unwrap_or_else(|e| panic!("lesson '{lesson}' failed: {e}"));
        assert!(out.contains("Live Output:"), "lesson '{lesson}' should render Live Output");
    }

    let docs_eng = handle_docs(Some("english"));
    assert!(docs_eng.contains("unless") && docs_eng.contains("until") && docs_eng.contains("dog learn"));
    let docs_print = handle_docs(Some("print"));
    assert!(docs_print.contains("print(value)"));
    assert!(builtin_signature_help("print").is_some());
}




