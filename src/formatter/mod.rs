#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Simple,
    If,
    Match { in_arm: bool },
    Try,
    Brace,
}

/// Formats DOG source code with canonical 4-space indentation per block level.
/// Preserves comments, blank lines (collapsed to at most one consecutive blank line),
/// and ignores inconsistent input indentation because block keywords & `end` determine boundaries.
pub fn format_source(source: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut stack: Vec<BlockKind> = Vec::new();
    let mut prev_blank = false;
    let mut in_triple_str = false;

    for raw_line in source.lines() {
        if in_triple_str {
            out.push(raw_line.to_string());
            if count_triple_quotes(raw_line) % 2 == 1 {
                in_triple_str = false;
            }
            continue;
        }

        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            if !out.is_empty() && !prev_blank {
                out.push(String::new());
                prev_blank = true;
            }
            continue;
        }
        prev_blank = false;

        let first_word = leading_keyword(trimmed);
        let starts_rbrace = trimmed.starts_with('}');
        let ends_lbrace = ends_with_open_brace(trimmed);

        if starts_rbrace && stack.last() == Some(&BlockKind::Brace) {
            stack.pop();
        }

        let indent = match first_word {
            "case" => match stack.last() {
                Some(BlockKind::Match { in_arm }) => {
                    let level = stack_indent(&stack);
                    if *in_arm {
                        level.saturating_sub(1)
                    } else {
                        level
                    }
                }
                _ => stack_indent(&stack),
            },
            "otherwise" | "else" => match stack.last() {
                Some(BlockKind::If) => stack_indent(&stack).saturating_sub(1),
                Some(BlockKind::Match { in_arm }) => {
                    let level = stack_indent(&stack);
                    if *in_arm {
                        level.saturating_sub(1)
                    } else {
                        level
                    }
                }
                _ => stack_indent(&stack),
            },
            "elif" => match stack.last() {
                Some(BlockKind::If) => stack_indent(&stack).saturating_sub(1),
                _ => stack_indent(&stack),
            },
            "catch" => {
                if stack.last() == Some(&BlockKind::Try) {
                    stack_indent(&stack).saturating_sub(1)
                } else {
                    stack_indent(&stack)
                }
            }
            "end" => {
                while stack.last() == Some(&BlockKind::Brace) {
                    stack.pop();
                }
                if !stack.is_empty() {
                    stack.pop();
                }
                stack_indent(&stack)
            }
            _ => stack_indent(&stack),
        };

        let pad = "    ".repeat(indent);
        out.push(format!("{pad}{trimmed}"));

        if count_triple_quotes(trimmed) % 2 == 1 {
            in_triple_str = true;
        }

        // Update block stack after printing the line
        match first_word {
            "if" | "unless" => stack.push(BlockKind::If),
            "match" => stack.push(BlockKind::Match { in_arm: false }),
            "try" => stack.push(BlockKind::Try),
            "case" => {
                if let Some(BlockKind::Match { in_arm }) = stack.last_mut() {
                    *in_arm = true;
                }
            }
            "otherwise" | "else" => {
                if let Some(BlockKind::Match { in_arm }) = stack.last_mut() {
                    *in_arm = true;
                }
            }
            "while" | "until" | "repeat" | "for" | "fn" | "define" | "function" | "struct"
            | "enum" => {
                stack.push(BlockKind::Simple);
            }
            _ => {
                if ends_lbrace && !starts_rbrace {
                    stack.push(BlockKind::Brace);
                }
            }
        }
    }

    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }

    if out.is_empty() {
        String::new()
    } else {
        let mut joined = out.join("\n");
        joined.push('\n');
        joined
    }
}

fn count_triple_quotes(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut count = 0usize;
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if &bytes[i..i + 3] == b"\"\"\"" {
            count += 1;
            i += 3;
            continue;
        }
        i += 1;
    }
    count
}

fn stack_indent(stack: &[BlockKind]) -> usize {
    let mut level = 0usize;
    for b in stack {
        match b {
            BlockKind::Match { in_arm: true } => level += 2,
            _ => level += 1,
        }
    }
    level
}

fn ends_with_open_brace(trimmed: &str) -> bool {
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return false;
    }
    trimmed.ends_with('{')
}

fn leading_keyword(trimmed: &str) -> &str {
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return "";
    }
    trimmed
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
}
