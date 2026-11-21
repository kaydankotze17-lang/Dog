use dog_lang::bytecode::{build_bundle, decode_bundle, run_bundle_with_io};
use dog_lang::crypto::{
    base64_decode, base64_encode, hex_decode, hex_encode, hmac_sha256_hex, sha256_hex,
};
use dog_lang::editor::EditorBuffer;
use dog_lang::formatter::format_source;
use dog_lang::hub::DogPack;
use dog_lang::interpreter::{check_source, run_source_with_io};
use dog_lang::json::parse_json;
use dog_lang::prompt::{handle_completions, handle_prompt};
use dog_lang::sdu::handle_sdu;
use dog_lang::stdlib::InterpreterIo;
use std::collections::VecDeque;
use std::env;
use std::fs;

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

