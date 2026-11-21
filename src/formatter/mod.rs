/// Formats DOG source code with canonical 4-space indentation per block level.
/// Preserves comments, blank lines (collapsed to at most one consecutive blank line),
/// and ignores inconsistent input indentation because `end` determines block boundaries.
pub fn format_source(source: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut indent: usize = 0;
    let mut prev_blank = false;

    for raw_line in source.lines() {
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

        if first_word == "otherwise" || first_word == "end" {
            indent = indent.saturating_sub(1);
        }

        let pad = "    ".repeat(indent);
        out.push(format!("{pad}{trimmed}"));

        if matches!(
            first_word,
            "if" | "otherwise" | "while" | "repeat" | "for" | "fn"
        ) {
            indent += 1;
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

fn leading_keyword(trimmed: &str) -> &str {
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return "";
    }
    trimmed
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
}
