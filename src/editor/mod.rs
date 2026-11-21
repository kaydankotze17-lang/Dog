use crate::config::Personality;
use crate::diagnostics::{DogDiagnostic, DOG_TIPS};
use crate::formatter::format_source;
use crate::interpreter::{check_source, run_source_with_io};
use crate::prompt::detect_git_status_tag;
use crate::stdlib::InterpreterIo;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
struct Snapshot {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
}

#[derive(Debug, Clone)]
pub struct EditorBuffer {
    pub path: Option<String>,
    pub title: String,
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub dirty: bool,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

const STATIC_CANDIDATES: &[&str] = &[
    "say(",
    "answer(",
    "lower(",
    "upper(",
    "trim(",
    "contains(",
    "starts_with(",
    "ends_with(",
    "replace(",
    "slice(",
    "split(",
    "join(",
    "len(",
    "text(",
    "number(",
    "decimal(",
    "push(",
    "pop(",
    "sort(",
    "reverse(",
    "list_contains(",
    "range(",
    "abs(",
    "min(",
    "max(",
    "round(",
    "floor(",
    "ceil(",
    "sqrt(",
    "pow(",
    "rand_int(",
    "assert(",
    "fail(",
    "map_get(",
    "map_set(",
    "map_has(",
    "map_keys(",
    "map_remove(",
    "file_read(",
    "file_write(",
    "file_append(",
    "file_exists(",
    "file_delete(",
    "dir_list(",
    "mkdir(",
    "cwd()",
    "args()",
    "env_get(",
    "env_set(",
    "time_now_ms()",
    "sleep_ms(",
    "json_parse(",
    "json_stringify(",
    "hex_encode(",
    "hex_decode(",
    "base64_encode(",
    "base64_decode(",
    "sha256(",
    "hmac_sha256(",
    "http_get(",
    "log_info(",
    "log_warn(",
    "log_error(",
    "let ",
    "var ",
    "const ",
    "if ",
    "otherwise",
    "match ",
    "case ",
    "try",
    "catch ",
    "while ",
    "repeat ",
    "for ",
    "fn ",
    "struct ",
    "enum ",
    "use ",
    "return ",
    "break",
    "continue",
    "end",
];

impl EditorBuffer {
    pub fn empty(title: impl Into<String>) -> Self {
        Self {
            path: None,
            title: title.into(),
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from_file(path: &str) -> Result<Self, String> {
        let content = if Path::new(path).exists() {
            fs::read_to_string(path).map_err(|e| format!("Failed to open '{path}': {e}"))?
        } else {
            String::new()
        };
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let title = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string();
        Ok(Self {
            path: Some(path.to_string()),
            title,
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }

    pub fn check(&self) -> Result<(), DogDiagnostic> {
        let name = self.path.as_deref().unwrap_or(&self.title);
        check_source(name, &self.text()).map(|_| ())
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(Snapshot {
            lines: self.lines.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        });
        if self.undo_stack.len() > 250 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(Snapshot {
            lines: self.lines.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        });
        self.lines = prev.lines;
        self.cursor_row = prev.cursor_row.min(self.lines.len().saturating_sub(1));
        self.cursor_col = prev
            .cursor_col
            .min(self.lines[self.cursor_row].chars().count());
        self.dirty = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(Snapshot {
            lines: self.lines.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        });
        self.lines = next.lines;
        self.cursor_row = next.cursor_row.min(self.lines.len().saturating_sub(1));
        self.cursor_col = next
            .cursor_col
            .min(self.lines[self.cursor_row].chars().count());
        self.dirty = true;
        true
    }

    pub fn insert_char(&mut self, ch: char) {
        self.push_undo();
        let line = &mut self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        line.insert(byte_idx, ch);
        self.cursor_col += 1;
        self.dirty = true;
    }

    /// Interactive smart insertion: auto-closes (), [], {}, "" and skips closing delimiters.
    pub fn insert_char_smart(&mut self, ch: char) {
        let next_char = self.lines[self.cursor_row].chars().nth(self.cursor_col);
        if matches!(ch, ')' | ']' | '}' | '"') && next_char == Some(ch) {
            self.cursor_col += 1;
            return;
        }
        let closing = match ch {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '"' => Some('"'),
            _ => None,
        };
        if let Some(close) = closing {
            let should_pair = match next_char {
                None => true,
                Some(c) if c.is_whitespace() || matches!(c, ')' | ']' | '}' | ',') => true,
                _ => false,
            };
            if should_pair {
                self.push_undo();
                let line = &mut self.lines[self.cursor_row];
                let byte_idx = char_to_byte_idx(line, self.cursor_col);
                line.insert(byte_idx, close);
                line.insert(byte_idx, ch);
                self.cursor_col += 1;
                self.dirty = true;
                return;
            }
        }
        self.insert_char(ch);
    }

    pub fn insert_tab(&mut self) {
        self.push_undo();
        let line = &mut self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        line.insert_str(byte_idx, "    ");
        self.cursor_col += 4;
        self.dirty = true;
    }

    pub fn insert_newline_auto_indent(&mut self) {
        self.push_undo();
        let current = self.lines[self.cursor_row].clone();
        let byte_idx = char_to_byte_idx(&current, self.cursor_col);
        let left = current[..byte_idx].to_string();
        let right = current[byte_idx..].to_string();

        let base_spaces = left.chars().take_while(|c| *c == ' ').count();
        let trimmed_left = left.trim();
        let first_word = trimmed_left
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        let extra = if matches!(
            first_word,
            "if" | "otherwise"
                | "while"
                | "repeat"
                | "for"
                | "fn"
                | "struct"
                | "enum"
                | "match"
                | "case"
                | "try"
                | "catch"
        ) || trimmed_left.ends_with('{')
        {
            4
        } else {
            0
        };
        let indent = " ".repeat(base_spaces + extra);

        self.lines[self.cursor_row] = left;
        self.cursor_row += 1;
        self.lines
            .insert(self.cursor_row, format!("{indent}{right}"));
        self.cursor_col = base_spaces + extra;
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col == 0 && self.cursor_row == 0 {
            return;
        }
        self.push_undo();
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let start = char_to_byte_idx(line, self.cursor_col - 1);
            let end = char_to_byte_idx(line, self.cursor_col);
            line.replace_range(start..end, "");
            self.cursor_col -= 1;
        } else {
            let removed = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&removed);
            self.cursor_col = prev_len;
        }
        self.dirty = true;
    }

    pub fn backspace_smart(&mut self) {
        if self.cursor_col > 0 {
            let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
            let prev_c = chars.get(self.cursor_col - 1).copied();
            let next_c = chars.get(self.cursor_col).copied();
            if matches!(
                (prev_c, next_c),
                (Some('('), Some(')'))
                    | (Some('['), Some(']'))
                    | (Some('{'), Some('}'))
                    | (Some('"'), Some('"'))
            ) {
                self.push_undo();
                let line = &mut self.lines[self.cursor_row];
                let start = char_to_byte_idx(line, self.cursor_col - 1);
                let end = char_to_byte_idx(line, self.cursor_col + 1);
                line.replace_range(start..end, "");
                self.cursor_col -= 1;
                self.dirty = true;
                return;
            }
        }
        self.backspace();
    }

    pub fn delete_forward(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            self.push_undo();
            let line = &mut self.lines[self.cursor_row];
            let start = char_to_byte_idx(line, self.cursor_col);
            let end = char_to_byte_idx(line, self.cursor_col + 1);
            line.replace_range(start..end, "");
            self.dirty = true;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.push_undo();
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
            self.dirty = true;
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
        }
    }

    pub fn move_word_left(&mut self) {
        if self.cursor_col == 0 {
            self.move_left();
            return;
        }
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let mut c = self.cursor_col;
        while c > 0 && !chars[c - 1].is_ascii_alphanumeric() && chars[c - 1] != '_' {
            c -= 1;
        }
        while c > 0 && (chars[c - 1].is_ascii_alphanumeric() || chars[c - 1] == '_') {
            c -= 1;
        }
        self.cursor_col = c;
    }

    pub fn move_word_right(&mut self) {
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let len = chars.len();
        if self.cursor_col >= len {
            self.move_right();
            return;
        }
        let mut c = self.cursor_col;
        while c < len && (chars[c].is_ascii_alphanumeric() || chars[c] == '_') {
            c += 1;
        }
        while c < len && !chars[c].is_ascii_alphanumeric() && chars[c] != '_' {
            c += 1;
        }
        self.cursor_col = c;
    }

    pub fn page_up(&mut self, page: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(page.max(1));
        let len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.cursor_col.min(len);
    }

    pub fn page_down(&mut self, page: usize) {
        self.cursor_row = (self.cursor_row + page.max(1)).min(self.lines.len().saturating_sub(1));
        let len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.cursor_col.min(len);
    }

    pub fn duplicate_line(&mut self) {
        self.push_undo();
        let copy = self.lines[self.cursor_row].clone();
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, copy);
        self.dirty = true;
    }

    pub fn delete_line(&mut self) {
        self.push_undo();
        if self.lines.len() == 1 {
            self.lines[0].clear();
            self.cursor_col = 0;
        } else {
            self.lines.remove(self.cursor_row);
            if self.cursor_row >= self.lines.len() {
                self.cursor_row = self.lines.len() - 1;
            }
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
        }
        self.dirty = true;
    }

    pub fn toggle_comment(&mut self) {
        self.push_undo();
        let line = &mut self.lines[self.cursor_row];
        let indent_len = line.chars().take_while(|c| *c == ' ').count();
        let indent: String = line.chars().take(indent_len).collect();
        let rest: String = line.chars().skip(indent_len).collect();
        if let Some(after) = rest.strip_prefix("# ") {
            *line = format!("{indent}{after}");
            self.cursor_col = self.cursor_col.saturating_sub(2);
        } else if let Some(after) = rest.strip_prefix('#') {
            *line = format!("{indent}{after}");
            self.cursor_col = self.cursor_col.saturating_sub(1);
        } else {
            *line = format!("{indent}# {rest}");
            self.cursor_col += 2;
        }
        self.dirty = true;
    }

    pub fn insert_snippet(&mut self, kind: &str) -> bool {
        let indent_len = self.lines[self.cursor_row]
            .chars()
            .take_while(|c| *c == ' ')
            .count();
        let pad = " ".repeat(indent_len);
        let snippet_lines: Vec<String> = match kind.trim().to_ascii_lowercase().as_str() {
            "fn" => vec![
                format!("{pad}fn my_fn(x: number) -> number"),
                format!("{pad}    return x"),
                format!("{pad}end"),
            ],
            "struct" => vec![
                format!("{pad}struct Item"),
                format!("{pad}    name: text"),
                format!("{pad}    value: number"),
                format!("{pad}end"),
            ],
            "enum" => vec![
                format!("{pad}enum State"),
                format!("{pad}    Ready"),
                format!("{pad}    Done"),
                format!("{pad}end"),
            ],
            "if" => vec![
                format!("{pad}if true"),
                format!("{pad}    say(\"ok\")"),
                format!("{pad}otherwise"),
                format!("{pad}    say(\"fallback\")"),
                format!("{pad}end"),
            ],
            "match" => vec![
                format!("{pad}match value"),
                format!("{pad}    case 1"),
                format!("{pad}        say(\"one\")"),
                format!("{pad}    otherwise"),
                format!("{pad}        say(\"other\")"),
                format!("{pad}end"),
            ],
            "try" => vec![
                format!("{pad}try"),
                format!("{pad}    say(\"working...\")"),
                format!("{pad}catch err"),
                format!("{pad}    log_error(err)"),
                format!("{pad}end"),
            ],
            "for" => vec![
                format!("{pad}for i in range(0, 5)"),
                format!("{pad}    say(text(i))"),
                format!("{pad}end"),
            ],
            _ => return false,
        };
        self.push_undo();
        let insert_at = if self.lines[self.cursor_row].trim().is_empty() {
            self.lines.remove(self.cursor_row);
            self.cursor_row
        } else {
            self.cursor_row + 1
        };
        for (offset, line) in snippet_lines.into_iter().enumerate() {
            self.lines.insert(insert_at + offset, line);
        }
        self.cursor_row = insert_at;
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.dirty = true;
        true
    }

    pub fn format_in_place(&mut self) {
        self.push_undo();
        let formatted = format_source(&self.text());
        let mut lines: Vec<String> = formatted.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        self.lines = lines;
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);
        let len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.cursor_col.min(len);
        self.dirty = true;
    }

    pub fn replace_all(&mut self, needle: &str, replacement: &str) -> usize {
        if needle.is_empty() {
            return 0;
        }
        self.push_undo();
        let mut count = 0usize;
        for line in &mut self.lines {
            let matches = line.matches(needle).count();
            if matches > 0 {
                *line = line.replace(needle, replacement);
                count += matches;
            }
        }
        if count > 0 {
            self.dirty = true;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
        } else {
            self.undo_stack.pop();
        }
        count
    }

    pub fn find_next(&mut self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let total = self.lines.len();
        for offset in 0..total {
            let row = (self.cursor_row + offset) % total;
            let start_char = if offset == 0 { self.cursor_col + 1 } else { 0 };
            let line = &self.lines[row];
            let byte_start = char_to_byte_idx(line, start_char.min(line.chars().count()));
            if let Some(found_byte) = line[byte_start..].find(needle) {
                let col = line[..byte_start + found_byte].chars().count();
                self.cursor_row = row;
                self.cursor_col = col;
                return true;
            }
        }
        false
    }

    pub fn matching_bracket(&self) -> Option<(usize, usize)> {
        let line_chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let ch = line_chars.get(self.cursor_col).copied()?;
        let (target, forward) = match ch {
            '(' => (')', true),
            '[' => (']', true),
            '{' => ('}', true),
            ')' => ('(', false),
            ']' => ('[', false),
            '}' => ('{', false),
            _ => return None,
        };

        if forward {
            let mut depth = 0i32;
            for r in self.cursor_row..self.lines.len() {
                let chars: Vec<char> = self.lines[r].chars().collect();
                let start_c = if r == self.cursor_row { self.cursor_col } else { 0 };
                for c in start_c..chars.len() {
                    if chars[c] == ch {
                        depth += 1;
                    } else if chars[c] == target {
                        depth -= 1;
                        if depth == 0 {
                            return Some((r, c));
                        }
                    }
                }
            }
        } else {
            let mut depth = 0i32;
            for r in (0..=self.cursor_row).rev() {
                let chars: Vec<char> = self.lines[r].chars().collect();
                let end_c = if r == self.cursor_row {
                    self.cursor_col
                } else {
                    chars.len().saturating_sub(1)
                };
                if chars.is_empty() {
                    continue;
                }
                for c in (0..=end_c.min(chars.len() - 1)).rev() {
                    if chars[c] == ch {
                        depth += 1;
                    } else if chars[c] == target {
                        depth -= 1;
                        if depth == 0 {
                            return Some((r, c));
                        }
                    }
                }
            }
        }
        None
    }

    fn current_prefix(&self) -> String {
        let line = &self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        line[..byte_idx]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    }

    pub fn autocomplete_suggestions(&self) -> Vec<&'static str> {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            return Vec::new();
        }
        STATIC_CANDIDATES
            .iter()
            .copied()
            .filter(|c| c.starts_with(&prefix) && *c != prefix)
            .take(5)
            .collect()
    }

    pub fn autocomplete_suggestions_dynamic(&self) -> Vec<String> {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut dynamic: BTreeSet<String> = BTreeSet::new();
        for line in &self.lines {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                if let Some(name) = rest.split('(').next() {
                    let clean = name.trim();
                    if !clean.is_empty() {
                        dynamic.insert(format!("{clean}("));
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                let clean = rest.split_whitespace().next().unwrap_or("");
                if !clean.is_empty() {
                    dynamic.insert(clean.to_string());
                }
            } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                let clean = rest.split_whitespace().next().unwrap_or("");
                if !clean.is_empty() {
                    dynamic.insert(clean.to_string());
                }
            } else if let Some(rest) = trimmed
                .strip_prefix("let ")
                .or_else(|| trimmed.strip_prefix("var "))
                .or_else(|| trimmed.strip_prefix("const "))
            {
                let clean = rest
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if !clean.is_empty() {
                    dynamic.insert(clean.to_string());
                }
            }
        }

        let mut results: Vec<String> = Vec::new();
        for cand in &dynamic {
            if cand.starts_with(&prefix) && cand != &prefix && results.len() < 5 {
                results.push(cand.clone());
            }
        }
        for &cand in STATIC_CANDIDATES {
            if cand.starts_with(&prefix)
                && cand != prefix
                && !results.iter().any(|r| r == cand)
                && results.len() < 5
            {
                results.push(cand.to_string());
            }
        }
        results
    }

    pub fn accept_autocomplete(&mut self) -> bool {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            return false;
        }
        let suggestions = self.autocomplete_suggestions_dynamic();
        let Some(top) = suggestions.first() else {
            return false;
        };
        let suffix = &top[prefix.len()..];
        if suffix.is_empty() {
            return false;
        }
        self.push_undo();
        let line = &mut self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        if suffix.ends_with('(') {
            line.insert_str(byte_idx, &format!("{suffix})"));
            self.cursor_col += suffix.chars().count();
        } else {
            line.insert_str(byte_idx, suffix);
            self.cursor_col += suffix.chars().count();
        }
        self.dirty = true;
        true
    }

    pub fn contextual_hint(&self) -> Option<String> {
        let line = &self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        let before = &line[..byte_idx];

        if let Some(open_pos) = before.rfind('(') {
            let fn_slice = &before[..open_pos];
            let callee: String = fn_slice
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if let Some(sig) = builtin_signature_help(&callee) {
                return Some(sig.to_string());
            }
        }

        let lead = line
            .trim_start()
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        keyword_help(lead).map(String::from)
    }

    pub fn extract_outline(&self) -> Vec<(usize, String)> {
        let mut items = Vec::new();
        for (idx, line) in self.lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("fn ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("enum ")
            {
                items.push((idx + 1, trimmed.to_string()));
            }
        }
        items
    }

    pub fn ensure_cursor_visible(&mut self, body_rows: usize, code_w: usize) {
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + body_rows {
            self.scroll_row = self.cursor_row + 1 - body_rows;
        }
        if self.cursor_col < self.scroll_col {
            self.scroll_col = self.cursor_col;
        } else if self.cursor_col >= self.scroll_col + code_w {
            self.scroll_col = self.cursor_col + 1 - code_w;
        }
    }

    pub fn save_to(&mut self, path: &str) -> Result<(), String> {
        fs::write(path, self.text()).map_err(|e| format!("Failed to save '{path}': {e}"))?;
        self.path = Some(path.to_string());
        self.title = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string();
        self.dirty = false;
        Ok(())
    }
}

fn builtin_signature_help(name: &str) -> Option<&'static str> {
    match name {
        "say" => Some("say(value) -> void  | Print a value followed by a newline"),
        "answer" => Some("answer([prompt: text]) -> text  | Read a line from stdin"),
        "lower" => Some("lower(s: text) -> text  | Convert text to lowercase"),
        "upper" => Some("upper(s: text) -> text  | Convert text to uppercase"),
        "trim" => Some("trim(s: text) -> text  | Strip leading/trailing whitespace"),
        "contains" => Some("contains(haystack: text, needle: text) -> bool"),
        "starts_with" => Some("starts_with(s: text, prefix: text) -> bool"),
        "ends_with" => Some("ends_with(s: text, suffix: text) -> bool"),
        "replace" => Some("replace(s: text, from: text, to: text) -> text"),
        "slice" => Some("slice(s: text, start: number, end: number) -> text"),
        "split" => Some("split(s: text, sep: text) -> list<text>"),
        "join" => Some("join(items: list<text>, sep: text) -> text"),
        "len" => Some("len(x: text | list<T> | map<K,V>) -> number"),
        "text" => Some("text(value) -> text  | Explicit conversion to text"),
        "number" => Some("number(x: text | decimal | number) -> number"),
        "decimal" => Some("decimal(x: text | number | decimal) -> decimal"),
        "push" => Some("push(var list: list<T>, item: T) -> void"),
        "pop" => Some("pop(var list: list<T>) -> T"),
        "sort" => Some("sort(var list: list<T>) -> void  | In-place ascending sort"),
        "reverse" => Some("reverse(var list: list<T>) -> void  | Reverse elements in-place"),
        "list_contains" => Some("list_contains(list: list<T>, item: T) -> bool"),
        "range" => Some("range(start: number, end: number) -> list<number>"),
        "abs" => Some("abs(x: number | decimal) -> number | decimal"),
        "min" => Some("min(a: T, b: T) -> T  | Smaller of two numbers/decimals"),
        "max" => Some("max(a: T, b: T) -> T  | Larger of two numbers/decimals"),
        "round" => Some("round(x: decimal) -> number  | Nearest integer"),
        "floor" => Some("floor(x: decimal) -> number  | Round down"),
        "ceil" => Some("ceil(x: decimal) -> number  | Round up"),
        "sqrt" => Some("sqrt(x: decimal) -> decimal  | Square root (x >= 0)"),
        "pow" => Some("pow(base: decimal, exp: decimal) -> decimal"),
        "rand_int" => Some("rand_int(min: number, max: number) -> number  | Inclusive RNG"),
        "assert" => Some("assert(cond: bool, [msg: text]) -> void  | Fail with E033 if false"),
        "fail" => Some("fail(reason: text) -> void  | Throw catchable E035 error"),
        "map_get" => Some("map_get(m: map<K,V>, key: K) -> V"),
        "map_set" => Some("map_set(var m: map<K,V>, key: K, val: V) -> void"),
        "map_has" => Some("map_has(m: map<K,V>, key: K) -> bool"),
        "map_keys" => Some("map_keys(m: map<K,V>) -> list<K>"),
        "map_remove" => Some("map_remove(var m: map<K,V>, key: K) -> bool"),
        "file_read" => Some("file_read(path: text) -> text"),
        "file_write" => Some("file_write(path: text, content: text) -> void"),
        "file_append" => Some("file_append(path: text, content: text) -> void"),
        "file_exists" => Some("file_exists(path: text) -> bool"),
        "file_delete" => Some("file_delete(path: text) -> bool"),
        "dir_list" => Some("dir_list(path: text) -> list<text>"),
        "mkdir" => Some("mkdir(path: text) -> void  | Create directories recursively"),
        "cwd" => Some("cwd() -> text  | Current working directory"),
        "args" => Some("args() -> list<text>  | Script CLI arguments"),
        "env_get" => Some("env_get(name: text) -> text"),
        "env_set" => Some("env_set(name: text, val: text) -> void"),
        "time_now_ms" => Some("time_now_ms() -> number  | Epoch milliseconds"),
        "sleep_ms" => Some("sleep_ms(ms: number) -> void"),
        "json_parse" => Some("json_parse(raw: text) -> map<text, text>"),
        "json_stringify" => Some("json_stringify(val) -> text"),
        "hex_encode" => Some("hex_encode(s: text) -> text"),
        "hex_decode" => Some("hex_decode(hex: text) -> text"),
        "base64_encode" => Some("base64_encode(s: text) -> text"),
        "base64_decode" => Some("base64_decode(b64: text) -> text"),
        "sha256" => Some("sha256(s: text) -> text  | FIPS 180-4 SHA-256 hex digest"),
        "hmac_sha256" => Some("hmac_sha256(key: text, msg: text) -> text  | RFC 2104 HMAC"),
        "http_get" => Some("http_get(url: text) -> text  | Plain HTTP GET request"),
        "log_info" => Some("log_info(msg: text) -> void"),
        "log_warn" => Some("log_warn(msg: text) -> void"),
        "log_error" => Some("log_error(msg: text) -> void"),
        _ => None,
    }
}

fn keyword_help(kw: &str) -> Option<&'static str> {
    match kw {
        "let" => Some("let name = expr  | Immutable binding (cannot be reassigned)"),
        "var" => Some("var name = expr  | Mutable binding (allows reassignment, push/sort)"),
        "const" => Some("const NAME = expr  | Module-level constant"),
        "fn" => Some("fn name(arg: type) -> ret_type ... end  | Typed function"),
        "struct" => Some("struct Name ... field: type ... end  | Product record type"),
        "enum" => Some("enum Name ... Variant ... end  | Sum/variant type"),
        "if" | "otherwise" => Some("if cond ... otherwise ... end  | Strict bool conditional"),
        "match" | "case" => Some("match val ... case pat ... otherwise ... end"),
        "try" | "catch" => Some("try ... catch err ... end  | Safe runtime error recovery"),
        "for" => Some("for item in list_expr ... end  | Try `for i in range(0, 10)`"),
        "while" => Some("while bool_cond ... end  | Loop while condition is true"),
        "repeat" => Some("repeat count ... end  | Repeat body N times"),
        "use" => Some("use \"other.dg\"  | Import functions/types from local .dg file"),
        _ => None,
    }
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub(crate) struct TermGuard {
    saved_stty: Option<String>,
}

impl TermGuard {
    pub(crate) fn enter_raw() -> Result<Self, String> {
        let output = Command::new("stty")
            .arg("-g")
            .stdin(std::process::Stdio::inherit())
            .output()
            .map_err(|e| format!("DOG Editor requires a Unix TTY (`stty` unavailable): {e}"))?;
        if !output.status.success() {
            return Err("DOG Editor must be run in an interactive terminal.".to_string());
        }
        let saved = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let status = Command::new("stty")
            .args(["raw", "-echo", "-ixon"])
            .stdin(std::process::Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to switch terminal to raw mode: {e}"))?;
        if !status.success() {
            return Err("Failed to initialize raw terminal mode.".to_string());
        }
        print!("\x1b[?1049h\x1b[?25h");
        let _ = io::stdout().flush();
        Ok(Self {
            saved_stty: Some(saved),
        })
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
        if let Some(ref saved) = self.saved_stty {
            let _ = Command::new("stty")
                .arg(saved)
                .stdin(std::process::Stdio::inherit())
                .status();
        }
    }
}

pub(crate) fn query_terminal_size() -> (usize, usize) {
    if let Ok(out) = Command::new("stty")
        .arg("size")
        .stdin(std::process::Stdio::inherit())
        .output()
    {
        if out.status.success() {
            let txt = String::from_utf8_lossy(&out.stdout);
            let mut parts = txt.split_whitespace();
            if let (Some(r), Some(c)) = (parts.next(), parts.next()) {
                if let (Ok(rows), Ok(cols)) = (r.parse::<usize>(), c.parse::<usize>()) {
                    return (rows.max(10), cols.max(30));
                }
            }
        }
    }
    (24, 80)
}

pub(crate) enum KeyEvent {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
    PageUp,
    PageDown,
    Ctrl(char),
    Esc,
}

pub(crate) fn read_key(stdin: &mut io::StdinLock<'_>) -> io::Result<KeyEvent> {
    let mut buf = [0u8; 1];
    stdin.read_exact(&mut buf)?;
    let b = buf[0];
    match b {
        0x1b => {
            let mut seq = [0u8; 2];
            if stdin.read_exact(&mut seq).is_err() {
                return Ok(KeyEvent::Esc);
            }
            if seq[0] == b'[' {
                match seq[1] {
                    b'A' => Ok(KeyEvent::Up),
                    b'B' => Ok(KeyEvent::Down),
                    b'C' => Ok(KeyEvent::Right),
                    b'D' => Ok(KeyEvent::Left),
                    b'H' => Ok(KeyEvent::Home),
                    b'F' => Ok(KeyEvent::End),
                    b'1' => read_tilde_or_mod(stdin, b'1'),
                    b'3' => read_tilde_or_mod(stdin, b'3'),
                    b'4' => read_tilde_or_mod(stdin, b'4'),
                    b'5' => read_tilde_or_mod(stdin, b'5'),
                    b'6' => read_tilde_or_mod(stdin, b'6'),
                    _ => Ok(KeyEvent::Esc),
                }
            } else if seq[0] == b'O' {
                match seq[1] {
                    b'H' => Ok(KeyEvent::Home),
                    b'F' => Ok(KeyEvent::End),
                    _ => Ok(KeyEvent::Esc),
                }
            } else {
                Ok(KeyEvent::Esc)
            }
        }
        b'\r' | b'\n' => Ok(KeyEvent::Enter),
        b'\t' => Ok(KeyEvent::Tab),
        127 | 8 => Ok(KeyEvent::Backspace),
        1..=26 => {
            let ch = (b + b'a' - 1) as char;
            Ok(KeyEvent::Ctrl(ch))
        }
        _ => {
            let width = if b & 0b1000_0000 == 0 {
                1
            } else if b & 0b1110_0000 == 0b1100_0000 {
                2
            } else if b & 0b1111_0000 == 0b1110_0000 {
                3
            } else {
                4
            };
            let mut bytes = vec![b];
            for _ in 1..width {
                let mut next = [0u8; 1];
                if stdin.read_exact(&mut next).is_ok() {
                    bytes.push(next[0]);
                }
            }
            let s = String::from_utf8_lossy(&bytes);
            Ok(KeyEvent::Char(s.chars().next().unwrap_or(' ')))
        }
    }
}

fn read_tilde_or_mod(stdin: &mut io::StdinLock<'_>, first: u8) -> io::Result<KeyEvent> {
    let mut b3 = [0u8; 1];
    if stdin.read_exact(&mut b3).is_err() {
        return Ok(KeyEvent::Esc);
    }
    if b3[0] == b'~' {
        return Ok(match first {
            b'1' => KeyEvent::Home,
            b'3' => KeyEvent::Delete,
            b'4' => KeyEvent::End,
            b'5' => KeyEvent::PageUp,
            b'6' => KeyEvent::PageDown,
            _ => KeyEvent::Esc,
        });
    }
    if first == b'1' && b3[0] == b';' {
        let mut rest = [0u8; 2];
        if stdin.read_exact(&mut rest).is_ok() {
            return Ok(match rest[1] {
                b'C' => KeyEvent::WordRight,
                b'D' => KeyEvent::WordLeft,
                b'H' => KeyEvent::Home,
                b'F' => KeyEvent::End,
                _ => KeyEvent::Esc,
            });
        }
    }
    Ok(KeyEvent::Esc)
}

pub(crate) struct BufferRunIo {
    pub(crate) output: String,
}

impl InterpreterIo for BufferRunIo {
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
}

pub fn launch_editor(initial_file: Option<&str>, personality: Personality) -> Result<(), String> {
    let mut tabs: Vec<EditorBuffer> = Vec::new();
    if let Some(path) = initial_file {
        tabs.push(EditorBuffer::from_file(path)?);
    } else {
        tabs.push(EditorBuffer::empty("untitled.dg"));
    }
    let mut active_tab = 0usize;

    let _guard = TermGuard::enter_raw()?;
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    let mut status_msg = String::from(DOG_TIPS[0]);
    let mut tip_idx = 0usize;
    let mut show_help = false;
    let mut run_output: Option<String> = None;

    loop {
        let (rows, cols) = query_terminal_size();
        let body_rows = rows.saturating_sub(4).max(3);
        let gutter_w = 6usize;
        let code_w = cols.saturating_sub(gutter_w + 1).max(10);

        tabs[active_tab].ensure_cursor_visible(body_rows, code_w);
        let diag = tabs[active_tab].check().err();
        let git_tag = detect_git_status_tag(Path::new("."));

        render_screen(
            &tabs,
            active_tab,
            diag.as_ref(),
            git_tag.as_deref(),
            &status_msg,
            show_help,
            run_output.as_deref(),
            rows,
            cols,
        );

        let key = match read_key(&mut lock) {
            Ok(k) => k,
            Err(_) => break,
        };

        if show_help {
            show_help = false;
            continue;
        }
        if run_output.is_some() {
            run_output = None;
            continue;
        }

        match key {
            KeyEvent::Ctrl('q') => {
                if tabs[active_tab].dirty {
                    status_msg = "Unsaved changes! Press Ctrl+S to save or Ctrl+X to discard & quit.".to_string();
                } else if tabs.len() > 1 {
                    tabs.remove(active_tab);
                    if active_tab >= tabs.len() {
                        active_tab = tabs.len() - 1;
                    }
                } else {
                    break;
                }
            }
            KeyEvent::Ctrl('x') => break,
            KeyEvent::Ctrl('s') => {
                let target_path = if let Some(ref p) = tabs[active_tab].path {
                    Some(p.clone())
                } else {
                    prompt_line(&mut lock, rows, cols, "Save as (.dg): ")
                };
                if let Some(path) = target_path {
                    if !path.trim().is_empty() {
                        match tabs[active_tab].save_to(path.trim()) {
                            Ok(()) => status_msg = format!("Saved '{}'.", path.trim()),
                            Err(e) => status_msg = e,
                        }
                    }
                }
            }
            KeyEvent::Ctrl('o') => {
                if let Some(path) = prompt_line(&mut lock, rows, cols, "Open file: ") {
                    let trimmed = path.trim();
                    if !trimmed.is_empty() {
                        match EditorBuffer::from_file(trimmed) {
                            Ok(new_buf) => {
                                tabs.push(new_buf);
                                active_tab = tabs.len() - 1;
                                status_msg = format!("Opened '{trimmed}'.");
                            }
                            Err(e) => status_msg = e,
                        }
                    }
                }
            }
            KeyEvent::Ctrl('f') => {
                if let Some(needle) = prompt_line(&mut lock, rows, cols, "Find: ") {
                    if !needle.is_empty() {
                        if let Some(repl) = prompt_line(
                            &mut lock,
                            rows,
                            cols,
                            "Replace with (blank = find next): ",
                        ) {
                            if repl.is_empty() {
                                if tabs[active_tab].find_next(&needle) {
                                    status_msg = format!("Found '{needle}'.");
                                } else {
                                    status_msg = format!("'{needle}' not found.");
                                }
                            } else {
                                let n = tabs[active_tab].replace_all(&needle, &repl);
                                status_msg = format!("Replaced {n} occurrence(s).");
                            }
                        }
                    }
                }
            }
            KeyEvent::Ctrl('p') => {
                if let Some(cmd) = prompt_line(
                    &mut lock,
                    rows,
                    cols,
                    "Palette (:<ln>|fmt|run|comment|dup|del|snippet fn/if/try/match/struct/for): ",
                ) {
                    status_msg = run_editor_palette_cmd(
                        &cmd,
                        &mut tabs[active_tab],
                        &mut run_output,
                        personality,
                    );
                }
            }
            KeyEvent::Ctrl('l') => {
                tabs[active_tab].format_in_place();
                status_msg = "Formatted buffer with 4-space DOG style.".to_string();
            }
            KeyEvent::Ctrl('d') => {
                tabs[active_tab].duplicate_line();
                status_msg = "Duplicated current line.".to_string();
            }
            KeyEvent::Ctrl('k') => {
                tabs[active_tab].delete_line();
                status_msg = "Deleted line.".to_string();
            }
            KeyEvent::Ctrl('u') => {
                tabs[active_tab].toggle_comment();
                status_msg = "Toggled comment.".to_string();
            }
            KeyEvent::Ctrl('r') => {
                let (out, msg) = execute_buffer_captured(&tabs[active_tab], personality);
                run_output = Some(out);
                status_msg = msg;
            }
            KeyEvent::Ctrl('z') => {
                if tabs[active_tab].undo() {
                    status_msg = "Undo.".to_string();
                }
            }
            KeyEvent::Ctrl('y') => {
                if tabs[active_tab].redo() {
                    status_msg = "Redo.".to_string();
                }
            }
            KeyEvent::Ctrl('t') => {
                let n = tabs.len() + 1;
                tabs.push(EditorBuffer::empty(format!("scratch_{n}.dg")));
                active_tab = tabs.len() - 1;
                status_msg = "Opened new tab.".to_string();
            }
            KeyEvent::Ctrl('n') => {
                active_tab = (active_tab + 1) % tabs.len();
            }
            KeyEvent::Ctrl('g') => {
                show_help = true;
                tip_idx = (tip_idx + 1) % DOG_TIPS.len();
                status_msg = DOG_TIPS[tip_idx].to_string();
            }
            KeyEvent::Char(ch) => tabs[active_tab].insert_char_smart(ch),
            KeyEvent::Enter => tabs[active_tab].insert_newline_auto_indent(),
            KeyEvent::Tab => {
                if !tabs[active_tab].accept_autocomplete() {
                    tabs[active_tab].insert_tab();
                }
            }
            KeyEvent::Backspace => tabs[active_tab].backspace_smart(),
            KeyEvent::Delete => tabs[active_tab].delete_forward(),
            KeyEvent::Left => tabs[active_tab].move_left(),
            KeyEvent::Right => tabs[active_tab].move_right(),
            KeyEvent::WordLeft => tabs[active_tab].move_word_left(),
            KeyEvent::WordRight => tabs[active_tab].move_word_right(),
            KeyEvent::Up => tabs[active_tab].move_up(),
            KeyEvent::Down => tabs[active_tab].move_down(),
            KeyEvent::PageUp => tabs[active_tab].page_up(body_rows),
            KeyEvent::PageDown => tabs[active_tab].page_down(body_rows),
            KeyEvent::Home => tabs[active_tab].cursor_col = 0,
            KeyEvent::End => {
                let len = tabs[active_tab].lines[tabs[active_tab].cursor_row]
                    .chars()
                    .count();
                tabs[active_tab].cursor_col = len;
            }
            KeyEvent::Esc | KeyEvent::Ctrl(_) => {}
        }
    }

    Ok(())
}

fn execute_buffer_captured(buf: &EditorBuffer, personality: Personality) -> (String, String) {
    let name = buf.path.as_deref().unwrap_or(&buf.title);
    let src = buf.text();
    let mut runner_io = BufferRunIo {
        output: String::new(),
    };
    match run_source_with_io(name, &src, &mut runner_io) {
        Ok(()) => {
            let out = if runner_io.output.is_empty() {
                "(Program finished with no stdout output)".to_string()
            } else {
                runner_io.output
            };
            (out, "Executed buffer successfully.".to_string())
        }
        Err(d) => (
            d.format_with(personality, Some(&src)),
            format!("Execution stopped with {}.", d.code),
        ),
    }
}

fn run_editor_palette_cmd(
    raw: &str,
    buf: &mut EditorBuffer,
    run_output: &mut Option<String>,
    personality: Personality,
) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Palette cancelled.".to_string();
    }
    if let Some(num_str) = trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix("goto ")) {
        if let Ok(line_num) = num_str.trim().parse::<usize>() {
            let target = line_num.saturating_sub(1).min(buf.lines.len().saturating_sub(1));
            buf.cursor_row = target;
            buf.cursor_col = 0;
            return format!("Jumped to line {}.", target + 1);
        }
        return "Usage: :<line-number> or goto <line-number>".to_string();
    }
    if trimmed == "fmt" || trimmed == "format" {
        buf.format_in_place();
        return "Formatted buffer.".to_string();
    }
    if trimmed == "run" {
        let (out, msg) = execute_buffer_captured(buf, personality);
        *run_output = Some(out);
        return msg;
    }
    if trimmed == "comment" {
        buf.toggle_comment();
        return "Toggled line comment.".to_string();
    }
    if trimmed == "dup" {
        buf.duplicate_line();
        return "Duplicated line.".to_string();
    }
    if trimmed == "del" {
        buf.delete_line();
        return "Deleted line.".to_string();
    }
    if trimmed == "outline" {
        let outline = buf.extract_outline();
        if outline.is_empty() {
            return "No fn/struct/enum symbols in buffer.".to_string();
        }
        let summary: Vec<String> = outline
            .iter()
            .take(5)
            .map(|(ln, label)| format!("L{ln}: {label}"))
            .collect();
        return summary.join(" | ");
    }
    if let Some(kind) = trimmed.strip_prefix("snippet ") {
        if buf.insert_snippet(kind) {
            return format!("Inserted '{kind}' snippet.");
        }
        return "Unknown snippet. Try: fn, struct, enum, if, match, try, for".to_string();
    }
    format!("Unknown palette command '{trimmed}'.")
}

fn prompt_line(
    stdin: &mut io::StdinLock<'_>,
    rows: usize,
    cols: usize,
    prompt: &str,
) -> Option<String> {
    let mut input = String::new();
    loop {
        let shown: String = format!(" {prompt}{input}").chars().take(cols).collect();
        print!("\x1b[{rows};1H\x1b[2K\x1b[48;5;24;38;5;255m{shown}\x1b[K\x1b[0m");
        let _ = io::stdout().flush();
        match read_key(stdin) {
            Ok(KeyEvent::Enter) => return Some(input),
            Ok(KeyEvent::Esc) | Ok(KeyEvent::Ctrl('q')) => return None,
            Ok(KeyEvent::Backspace) => {
                input.pop();
            }
            Ok(KeyEvent::Char(c)) => input.push(c),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_screen(
    tabs: &[EditorBuffer],
    active_tab: usize,
    diag: Option<&DogDiagnostic>,
    git_tag: Option<&str>,
    status_msg: &str,
    show_help: bool,
    run_output: Option<&str>,
    rows: usize,
    cols: usize,
) {
    let buf = &tabs[active_tab];
    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[H");

    // Row 1: 256-color Dark Slate Header + Tabs + Status Badge
    out.push_str("\x1b[48;5;33;38;5;255;1m DOG \x1b[22;48;5;235;38;5;250m ");
    for (idx, t) in tabs.iter().enumerate() {
        let marker = if t.dirty {
            "\x1b[38;5;214m●\x1b[38;5;255m"
        } else {
            ""
        };
        if idx == active_tab {
            out.push_str(&format!(
                "\x1b[48;5;238;38;5;255;1m {}{} \x1b[22;48;5;235;38;5;246m",
                t.title, marker
            ));
        } else {
            out.push_str(&format!(" {}{} ", t.title, marker));
        }
    }
    if let Some(branch) = git_tag {
        out.push_str(&format!(" \x1b[38;5;117mgit:{branch}\x1b[38;5;250m"));
    }
    match diag {
        None => out.push_str(" \x1b[38;5;78;1m[✓ OK]\x1b[22m"),
        Some(d) => out.push_str(&format!(" \x1b[38;5;203;1m[✗ {}]\x1b[22m", d.code)),
    }
    out.push_str("\x1b[K\x1b[0m\r\n");

    let reserved_bottom = 3;
    let body_rows = rows.saturating_sub(1 + reserved_bottom).max(3);
    let bracket_match = buf.matching_bracket();
    let error_line = diag.map(|d| d.span.line);

    if show_help {
        let help_lines = [
            "DOG EDITOR — Shortcuts, Navigation & Quick Reference",
            "─────────────────────────────────────────────────────",
            "Ctrl+S  Save file         Ctrl+O  Open in new tab",
            "Ctrl+R  Run buffer        Ctrl+L  Format (4-space style)",
            "Ctrl+F  Find & Replace    Ctrl+P  Command Palette (:line, snippet)",
            "Ctrl+D  Duplicate line    Ctrl+K  Delete line",
            "Ctrl+U  Toggle # comment  Tab     Accept autocomplete / Indent",
            "Ctrl+Z/Y Undo / Redo      Ctrl+T/N New tab / Next tab",
            "Ctrl+G  Toggle help       Ctrl+Q  Quit (Ctrl+X force quit)",
            "",
            "Syntax   : let (immutable), var (mutable), const, use \"mod.dg\"",
            "Types    : number, decimal, text, bool, void, list<T>, map<K,V>, struct, enum",
            "Stdlib   : say(), answer(), range(), sort(), slice(), json_parse(), sha256()",
            "Press any key to return to editor...",
        ];
        for r in 0..body_rows {
            let line = help_lines.get(r).copied().unwrap_or("");
            let clipped: String = line.chars().take(cols).collect();
            out.push_str(&format!(
                "\x1b[48;5;234;38;5;117m  {clipped}\x1b[K\x1b[0m\r\n"
            ));
        }
    } else if let Some(output) = run_output {
        let mut lines: Vec<&str> = output.lines().collect();
        if lines.is_empty() {
            lines.push("");
        }
        out.push_str(
            "\x1b[48;5;235;38;5;78;1m  ▶ DOG Execution Output (press any key to return)\x1b[K\x1b[0m\r\n",
        );
        for r in 1..body_rows {
            let line = lines.get(r - 1).copied().unwrap_or("");
            let clipped: String = line.chars().take(cols.saturating_sub(2)).collect();
            out.push_str(&format!(
                "\x1b[48;5;234;38;5;252m  {clipped}\x1b[K\x1b[0m\r\n"
            ));
        }
    } else {
        let gutter_w = 6usize;
        let text_w = cols.saturating_sub(gutter_w + 1).max(10);
        for screen_r in 0..body_rows {
            let file_r = buf.scroll_row + screen_r;
            if let Some(line) = buf.lines.get(file_r) {
                let line_num = file_r + 1;
                let is_active = file_r == buf.cursor_row;
                let is_err = error_line == Some(line_num);
                let trimmed = line.trim_start();
                let symbol_mark = if is_err {
                    "\x1b[38;5;203m✗\x1b[39m"
                } else if trimmed.starts_with("fn ") {
                    "\x1b[38;5;221mƒ\x1b[39m"
                } else if trimmed.starts_with("struct ") || trimmed.starts_with("enum ") {
                    "\x1b[38;5;81m◆\x1b[39m"
                } else {
                    " "
                };

                let row_bg = if is_active {
                    "\x1b[48;5;236m"
                } else {
                    "\x1b[48;5;234m"
                };
                let ln_fg = if is_err {
                    "\x1b[38;5;203;1m"
                } else if is_active {
                    "\x1b[38;5;221;1m"
                } else {
                    "\x1b[38;5;242m"
                };

                out.push_str(&format!(
                    "{row_bg}{ln_fg}{line_num:>4}\x1b[22m{symbol_mark}\x1b[38;5;239m│\x1b[38;5;252m"
                ));

                let visible: String = line.chars().skip(buf.scroll_col).take(text_w).collect();
                out.push_str(&highlight_dog_line(&visible));

                if let Some((mr, mc)) = bracket_match {
                    if mr == file_r {
                        out.push_str(&format!(" \x1b[38;5;214m[match @col {}]\x1b[39m", mc + 1));
                    }
                }
                out.push_str("\x1b[K\x1b[0m\r\n");
            } else {
                out.push_str("\x1b[48;5;234;38;5;238m   ~ │\x1b[K\x1b[0m\r\n");
            }
        }
    }

    // Row rows-1: Contextual Diagnostic / Autocomplete / Signature Tooltip Bar
    let suggestions = buf.autocomplete_suggestions_dynamic();
    if let Some(d) = diag {
        let msg = format!(
            " ✗ {} ({}:{}): {}  →  {}",
            d.code, d.span.line, d.span.column, d.title, d.fix
        );
        let clipped: String = msg.chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;52;38;5;224m{clipped}\x1b[K\x1b[0m\r\n"));
    } else if !suggestions.is_empty() {
        let hint = format!(" ▸ Tab to accept: {}", suggestions.join("   "));
        let clipped: String = hint.chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;24;38;5;159m{clipped}\x1b[K\x1b[0m\r\n"));
    } else if let Some(sig_hint) = buf.contextual_hint() {
        let clipped: String = format!(" ⓘ {sig_hint}").chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;235;38;5;117m{clipped}\x1b[K\x1b[0m\r\n"));
    } else {
        let clipped: String = format!(" ▸ {status_msg}").chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;235;38;5;110m{clipped}\x1b[K\x1b[0m\r\n"));
    }

    // Row rows: Status & Shortcut Footer
    let pos_info = format!(
        " Ln {}, Col {} │ {}L │ UTF-8",
        buf.cursor_row + 1,
        buf.cursor_col + 1,
        buf.lines.len()
    );
    let shortcuts = "^P Palette  ^R Run  ^S Save  ^L Fmt  ^F Find  ^D Dup  ^G Help  ^Q Quit";
    let footer: String = format!("{pos_info} │ {shortcuts}").chars().take(cols).collect();
    out.push_str(&format!("\x1b[48;5;235;38;5;250m{footer}\x1b[K\x1b[0m"));

    // Position cursor inside code pane cleanly accounting for horizontal scroll
    let cursor_screen_r = buf.cursor_row.saturating_sub(buf.scroll_row) + 2;
    let cursor_screen_c = 6 + buf.cursor_col.saturating_sub(buf.scroll_col) + 1;
    out.push_str(&format!("\x1b[{cursor_screen_r};{cursor_screen_c}H\x1b[?25h"));

    print!("{out}");
    let _ = io::stdout().flush();
}

/// Highlights a line of DOG source code using foreground-only ANSI 256-color escapes
/// so any caller row background shading stays continuous across tokens.
pub(crate) fn highlight_dog_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return format!("\x1b[3;38;5;244m{line}\x1b[23;39m");
    }

    let mut out = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '#' {
            let rest: String = chars[i..].iter().collect();
            out.push_str(&format!("\x1b[3;38;5;244m{rest}\x1b[23;39m"));
            break;
        } else if ch == '"' {
            out.push_str("\x1b[38;5;114m\"");
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '"' && (i == 0 || chars[i - 1] != '\\') {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str("\x1b[39m");
        } else if ch.is_ascii_digit() {
            out.push_str("\x1b[38;5;209m");
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                out.push(chars[i]);
                i += 1;
            }
            out.push_str("\x1b[39m");
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if matches!(
                word.as_str(),
                "let"
                    | "var"
                    | "const"
                    | "if"
                    | "otherwise"
                    | "match"
                    | "case"
                    | "try"
                    | "catch"
                    | "struct"
                    | "enum"
                    | "use"
                    | "end"
                    | "or"
                    | "and"
                    | "not"
                    | "fn"
                    | "return"
                    | "while"
                    | "repeat"
                    | "for"
                    | "in"
                    | "break"
                    | "continue"
            ) {
                out.push_str(&format!("\x1b[38;5;141;1m{word}\x1b[22;39m"));
            } else if matches!(word.as_str(), "true" | "false") {
                out.push_str(&format!("\x1b[38;5;209m{word}\x1b[39m"));
            } else if matches!(
                word.as_str(),
                "number" | "decimal" | "text" | "bool" | "void" | "list" | "map"
            ) {
                out.push_str(&format!("\x1b[38;5;81m{word}\x1b[39m"));
            } else if matches!(
                word.as_str(),
                "say"
                    | "answer"
                    | "lower"
                    | "upper"
                    | "trim"
                    | "contains"
                    | "starts_with"
                    | "ends_with"
                    | "replace"
                    | "slice"
                    | "split"
                    | "join"
                    | "len"
                    | "push"
                    | "pop"
                    | "sort"
                    | "reverse"
                    | "list_contains"
                    | "range"
                    | "abs"
                    | "min"
                    | "max"
                    | "round"
                    | "floor"
                    | "ceil"
                    | "sqrt"
                    | "pow"
                    | "rand_int"
                    | "assert"
                    | "fail"
                    | "map_get"
                    | "map_set"
                    | "map_has"
                    | "map_keys"
                    | "map_remove"
                    | "file_read"
                    | "file_write"
                    | "file_append"
                    | "file_exists"
                    | "file_delete"
                    | "dir_list"
                    | "mkdir"
                    | "cwd"
                    | "args"
                    | "env_get"
                    | "env_set"
                    | "time_now_ms"
                    | "sleep_ms"
                    | "json_parse"
                    | "json_stringify"
                    | "hex_encode"
                    | "hex_decode"
                    | "base64_encode"
                    | "base64_decode"
                    | "sha256"
                    | "hmac_sha256"
                    | "http_get"
                    | "log_info"
                    | "log_warn"
                    | "log_error"
            ) {
                out.push_str(&format!("\x1b[38;5;221m{word}\x1b[39m"));
            } else if word.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                out.push_str(&format!("\x1b[38;5;117m{word}\x1b[39m"));
            } else {
                out.push_str(&word);
            }
        } else if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}') {
            out.push_str(&format!("\x1b[38;5;214m{ch}\x1b[39m"));
            i += 1;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}
