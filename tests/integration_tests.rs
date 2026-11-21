use dog_lang::editor::EditorBuffer;
use dog_lang::formatter::format_source;
use dog_lang::interpreter::{check_source, run_source_with_io};
use dog_lang::stdlib::InterpreterIo;
use std::collections::VecDeque;

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
fn formats_blocks_with_four_spaces() {
    let messy = "if true\nsay(\"a\")\n   otherwise\n         say(\"b\")\n  end\n";
    let expected = "if true\n    say(\"a\")\notherwise\n    say(\"b\")\nend\n";
    assert_eq!(format_source(messy), expected);
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
