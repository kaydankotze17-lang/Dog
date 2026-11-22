use crate::checker::builtin_names;
use crate::config::Personality;
use crate::diagnostics::{levenshtein_distance, DogDiagnostic, DOG_TIPS};
use crate::formatter::format_source;
use crate::interpreter::{check_source, run_source_with_io};
use crate::prompt::detect_git_status_tag;
use crate::stdlib::InterpreterIo;
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionItem {
    pub insert: String,
    pub label: String,
    pub kind: &'static str,
    pub detail: String,
    pub is_snippet: bool,
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

const KEYWORD_CANDIDATES: &[(&str, &str)] = &[
    ("let ", "Immutable binding"),
    ("var ", "Mutable binding"),
    ("const ", "Module constant"),
    ("if ", "Conditional block"),
    ("otherwise if ", "Chained conditional branch"),
    ("otherwise", "Fallback branch"),
    ("match ", "Pattern match block"),
    ("case ", "Match arm"),
    ("try", "Error-handling block"),
    ("catch ", "Catch runtime error"),
    ("while ", "Condition loop"),
    ("repeat ", "Fixed-count loop"),
    ("for ", "Iterator loop"),
    ("fn ", "Function declaration"),
    ("struct ", "Struct record definition"),
    ("enum ", "Enum variant definition"),
    ("use ", "Import sibling .dg module"),
    ("return ", "Return from function"),
    ("break", "Exit innermost loop"),
    ("continue", "Next loop iteration"),
    ("end", "Close block"),
    ("number", "64-bit integer type"),
    ("decimal", "64-bit float type"),
    ("text", "UTF-8 string type"),
    ("bool", "Boolean type"),
    ("void", "Unit return type"),
    ("list<", "Typed dynamic array"),
    ("map<", "Typed key-value dictionary"),
];

const SNIPPET_CATALOG: &[(&str, &str)] = &[
    ("fn", "fn my_fn(x: number) -> number ... end"),
    ("struct", "struct Item ... field: type ... end"),
    ("enum", "enum State ... Variant ... end"),
    ("if", "if cond ... otherwise ... end"),
    ("otherwise if", "otherwise if cond ..."),
    ("match", "match value ... case ... otherwise ... end"),
    ("try", "try ... catch err ... end"),
    ("for", "for i in range(0, 5) ... end"),
    ("while", "while cond ... end"),
    ("http_server", "Local HTTP web server loop"),
    ("ws_server", "Local WebSocket echo server loop"),
    ("discord_bot", "Discord bot REST/webhook command bridge"),
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
        if matches!(ch, 'd' | 'e' | 'h') {
            self.maybe_auto_dedent();
        }
    }

    pub fn maybe_auto_dedent(&mut self) {
        if self.cursor_row == 0 {
            return;
        }
        let line = &self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        if !line[byte_idx..].trim().is_empty() {
            return;
        }
        let left = &line[..byte_idx];
        let leading = left.chars().take_while(|c| *c == ' ').count();
        if leading < 4 {
            return;
        }
        let kw = left[leading..].trim();
        if !matches!(kw, "end" | "otherwise" | "catch") {
            return;
        }
        let prev_indent = (0..self.cursor_row)
            .rev()
            .find_map(|r| {
                let l = &self.lines[r];
                if l.trim().is_empty() {
                    None
                } else {
                    Some(l.chars().take_while(|c| *c == ' ').count())
                }
            })
            .unwrap_or(0);
        if leading > 0 && leading >= prev_indent {
            let trimmed_rest = line[leading..].to_string();
            let new_pad = " ".repeat(leading - 4);
            self.lines[self.cursor_row] = format!("{new_pad}{trimmed_rest}");
            self.cursor_col = self.cursor_col.saturating_sub(4);
        }
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
            "otherwise if" | "elseif" => vec![
                format!("{pad}otherwise if cond"),
                format!("{pad}    say(\"branch\")"),
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
            "while" => vec![
                format!("{pad}while true"),
                format!("{pad}    break"),
                format!("{pad}end"),
            ],
            "http_server" => vec![
                format!("{pad}let srv = http_listen(\"127.0.0.1:8080\")"),
                format!("{pad}cli_info($\"Listening on http://127.0.0.1:{{http_port(srv)}}\")"),
                format!("{pad}while true"),
                format!("{pad}    let req = http_accept(srv)"),
                format!("{pad}    let path = map_get(req, \"path\")"),
                format!("{pad}    if path == \"/\""),
                format!("{pad}        http_reply(req, 200, \"text/html; charset=utf-8\", \"<h1>DOG Web Server</h1>\")"),
                format!("{pad}    otherwise"),
                format!("{pad}        http_reply(req, 404, \"text/plain\", \"Not Found\")"),
                format!("{pad}    end"),
                format!("{pad}end"),
            ],
            "ws_server" => vec![
                format!("{pad}let srv = http_listen(\"127.0.0.1:8081\")"),
                format!("{pad}while true"),
                format!("{pad}    let req = http_accept(srv)"),
                format!("{pad}    let ws = ws_upgrade(req)"),
                format!("{pad}    let msg = ws_recv(ws)"),
                format!("{pad}    ws_send(ws, $\"echo: {{msg}}\")"),
                format!("{pad}    ws_close(ws)"),
                format!("{pad}end"),
            ],
            "discord_bot" => vec![
                format!("{pad}let webhook = env_get(\"DISCORD_WEBHOOK_URL\")"),
                format!("{pad}var payload: map<text, text> = {{}}"),
                format!("{pad}map_set(payload, \"content\", \"Hello from DOG!\")"),
                format!("{pad}if len(webhook) > 0"),
                format!("{pad}    say(http_post(webhook, json_stringify(payload), \"application/json\"))"),
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
        self.completion_context().1
    }

    /// Returns `(dot_base_ident, partial_ident_after_dot_or_at_cursor)`.
    pub(crate) fn completion_context(&self) -> (Option<String>, String) {
        let line = &self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        let before = &line[..byte_idx];
        let partial: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let before_partial = &before[..before.len() - partial.len()];
        if let Some(before_dot) = before_partial.strip_suffix('.') {
            let base: String = before_dot
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if !base.is_empty() {
                return (Some(base), partial);
            }
        }
        (None, partial)
    }

    fn collect_symbol_tables(&self) -> BufferSymbolTables {
        let mut tables = BufferSymbolTables::default();
        let mut all_sources = vec![self.lines.clone()];
        if let Some(ref cur_path) = self.path {
            if let Some(parent) = Path::new(cur_path).parent() {
                for line in &self.lines {
                    let trimmed = line.trim();
                    if let Some(rest) = trimmed.strip_prefix("use ") {
                        let rel = rest.trim().trim_matches('"');
                        if !rel.is_empty() {
                            let mod_path = parent.join(rel);
                            if let Ok(src) = fs::read_to_string(mod_path) {
                                all_sources.push(src.lines().map(String::from).collect());
                            }
                        }
                    }
                }
            }
        }

        for (src_idx, src_lines) in all_sources.iter().enumerate() {
            let mut in_struct: Option<String> = None;
            let mut in_enum: Option<String> = None;
            for line in src_lines {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    continue;
                }
                if trimmed == "end" {
                    in_struct = None;
                    in_enum = None;
                    continue;
                }
                if let Some(ref sname) = in_struct {
                    if let Some((f, ty)) = trimmed.split_once(':') {
                        let fname = f.trim();
                        let ftype = ty.trim();
                        if is_valid_ident(fname) {
                            tables
                                .structs
                                .entry(sname.clone())
                                .or_default()
                                .push((fname.to_string(), ftype.to_string()));
                        }
                    }
                    continue;
                }
                if let Some(ref ename) = in_enum {
                    let vname = trimmed.split_whitespace().next().unwrap_or("");
                    if is_valid_ident(vname) {
                        tables
                            .enums
                            .entry(ename.clone())
                            .or_default()
                            .push(vname.to_string());
                    }
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("fn ") {
                    if let Some((name_part, after_paren)) = rest.split_once('(') {
                        let clean = name_part.trim();
                        if is_valid_ident(clean) {
                            tables
                                .functions
                                .insert(clean.to_string(), format!("fn {rest}"));
                        }
                        if src_idx == 0 {
                            if let Some((params_part, _)) = after_paren.split_once(')') {
                                for param in params_part.split(',') {
                                    if let Some((pname, pty)) = param.split_once(':') {
                                        let pn = pname.trim();
                                        let pt = pty.trim();
                                        if is_valid_ident(pn) {
                                            tables.var_types.insert(pn.to_string(), pt.to_string());
                                            tables
                                                .vars
                                                .insert(pn.to_string(), format!("param: {pt}"));
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                    let clean = rest.split_whitespace().next().unwrap_or("");
                    if is_valid_ident(clean) {
                        tables.structs.entry(clean.to_string()).or_default();
                        in_struct = Some(clean.to_string());
                    }
                } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                    let clean = rest.split_whitespace().next().unwrap_or("");
                    if is_valid_ident(clean) {
                        tables.enums.entry(clean.to_string()).or_default();
                        in_enum = Some(clean.to_string());
                    }
                } else if src_idx == 0 || trimmed.starts_with("const ") {
                    if let Some(rest) = trimmed
                        .strip_prefix("let ")
                        .or_else(|| trimmed.strip_prefix("var "))
                        .or_else(|| trimmed.strip_prefix("const "))
                    {
                        if let Some((lhs, rhs)) = rest.split_once('=') {
                            let rhs_t = rhs.trim();
                            let (vname, vty) = if let Some((n, t)) = lhs.split_once(':') {
                                (n.trim(), Some(t.trim().to_string()))
                            } else {
                                let inferred = rhs_t
                                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                                    .next()
                                    .filter(|tok| {
                                        rhs_t.contains('{')
                                            && tok
                                                .chars()
                                                .next()
                                                .is_some_and(|c| c.is_ascii_uppercase())
                                    })
                                    .map(String::from);
                                (lhs.trim(), inferred)
                            };
                            if is_valid_ident(vname) {
                                let detail = vty
                                    .clone()
                                    .unwrap_or_else(|| "binding".to_string());
                                tables.vars.insert(vname.to_string(), detail);
                                if let Some(ty) = vty {
                                    tables.var_types.insert(vname.to_string(), ty);
                                }
                            }
                        }
                    } else if let Some(rest) = trimmed.strip_prefix("for ") {
                        let loop_var = rest.split_whitespace().next().unwrap_or("");
                        if is_valid_ident(loop_var) {
                            tables
                                .vars
                                .insert(loop_var.to_string(), "loop item".to_string());
                        }
                    }
                }
            }
        }
        tables
    }

    pub fn ranked_suggestions(&self, allow_empty_prefix: bool) -> Vec<SuggestionItem> {
        let (dot_base, partial) = self.completion_context();
        if !allow_empty_prefix && dot_base.is_none() && partial.is_empty() {
            return Vec::new();
        }
        let tables = self.collect_symbol_tables();
        let mut scored: Vec<(i32, SuggestionItem)> = Vec::new();

        if let Some(base) = dot_base {
            if let Some(variants) = tables.enums.get(&base) {
                for v in variants {
                    if let Some(score) = score_candidate(v, &partial, true) {
                        scored.push((
                            score + 50,
                            SuggestionItem {
                                insert: v.clone(),
                                label: v.clone(),
                                kind: "enum",
                                detail: format!("{base}.{v}"),
                                is_snippet: false,
                            },
                        ));
                    }
                }
            }
            if let Some(ty_name) = tables.var_types.get(&base) {
                let clean_ty = ty_name.split('<').next().unwrap_or(ty_name).trim();
                if let Some(fields) = tables.structs.get(clean_ty) {
                    for (fname, fty) in fields {
                        if let Some(score) = score_candidate(fname, &partial, true) {
                            scored.push((
                                score + 60,
                                SuggestionItem {
                                    insert: fname.clone(),
                                    label: fname.clone(),
                                    kind: "fld",
                                    detail: format!("{clean_ty}.{fname}: {fty}"),
                                    is_snippet: false,
                                },
                            ));
                        }
                    }
                }
            }
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
            return scored.into_iter().take(8).map(|(_, item)| item).collect();
        }

        if partial.is_empty() && allow_empty_prefix {
            let defaults = [
                "fn",
                "if",
                "try",
                "for",
                "http_server",
                "ws_server",
                "discord_bot",
            ];
            let mut out = Vec::new();
            for key in defaults {
                if let Some((_, doc)) = SNIPPET_CATALOG.iter().find(|(k, _)| *k == key) {
                    out.push(SuggestionItem {
                        insert: key.to_string(),
                        label: key.to_string(),
                        kind: "snip",
                        detail: (*doc).to_string(),
                        is_snippet: true,
                    });
                }
            }
            out.push(SuggestionItem {
                insert: "say(".to_string(),
                label: "say()".to_string(),
                kind: "fn",
                detail: builtin_signature_help("say").unwrap_or("").to_string(),
                is_snippet: false,
            });
            return out;
        }

        // Local & imported functions
        for (fname, sig) in &tables.functions {
            if let Some(score) = score_candidate(fname, &partial, false) {
                let no_arg = sig.contains("()");
                let ins = if no_arg {
                    format!("{fname}()")
                } else {
                    format!("{fname}(")
                };
                scored.push((
                    score + 40,
                    SuggestionItem {
                        insert: ins,
                        label: format!("{fname}()"),
                        kind: "fn",
                        detail: sig.clone(),
                        is_snippet: false,
                    },
                ));
            }
        }
        // Local variables / constants / params
        for (vname, detail) in &tables.vars {
            if let Some(score) = score_candidate(vname, &partial, false) {
                scored.push((
                    score + 35,
                    SuggestionItem {
                        insert: vname.clone(),
                        label: vname.clone(),
                        kind: "var",
                        detail: detail.clone(),
                        is_snippet: false,
                    },
                ));
            }
        }
        // User structs & enums
        for sname in tables.structs.keys() {
            if let Some(score) = score_candidate(sname, &partial, false) {
                scored.push((
                    score + 30,
                    SuggestionItem {
                        insert: sname.clone(),
                        label: sname.clone(),
                        kind: "type",
                        detail: format!("struct {sname}"),
                        is_snippet: false,
                    },
                ));
            }
        }
        for ename in tables.enums.keys() {
            if let Some(score) = score_candidate(ename, &partial, false) {
                scored.push((
                    score + 30,
                    SuggestionItem {
                        insert: ename.clone(),
                        label: ename.clone(),
                        kind: "enum",
                        detail: format!("enum {ename}"),
                        is_snippet: false,
                    },
                ));
            }
        }
        // Snippets
        for &(key, desc) in SNIPPET_CATALOG {
            if let Some(score) = score_candidate(key, &partial, false) {
                scored.push((
                    score + 20,
                    SuggestionItem {
                        insert: key.to_string(),
                        label: key.to_string(),
                        kind: "snip",
                        detail: desc.to_string(),
                        is_snippet: true,
                    },
                ));
            }
        }
        // Stdlib built-ins (always in sync with TypeChecker)
        for &bname in builtin_names() {
            if let Some(score) = score_candidate(bname, &partial, false) {
                let sig = builtin_signature_help(bname).unwrap_or("DOG built-in");
                let no_arg = matches!(
                    bname,
                    "cwd" | "args" | "time_now_ms" | "time_utc" | "os_name" | "os_arch"
                        | "uuid_v4" | "cli_supports_color" | "cli_is_tty" | "math_pi"
                        | "math_e" | "temp_dir" | "local_ip"
                );
                let ins = if no_arg {
                    format!("{bname}()")
                } else {
                    format!("{bname}(")
                };
                scored.push((
                    score + 10,
                    SuggestionItem {
                        insert: ins,
                        label: format!("{bname}()"),
                        kind: "fn",
                        detail: sig.to_string(),
                        is_snippet: false,
                    },
                ));
            }
        }
        // Keywords & types
        for &(kw, doc) in KEYWORD_CANDIDATES {
            let ident = kw.trim_end_matches(&[' ', '<'][..]);
            if let Some(score) = score_candidate(ident, &partial, false) {
                scored.push((
                    score,
                    SuggestionItem {
                        insert: kw.to_string(),
                        label: kw.trim().to_string(),
                        kind: "kw",
                        detail: doc.to_string(),
                        is_snippet: false,
                    },
                ));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
        let mut deduped = Vec::new();
        let mut seen: BTreeSet<(String, &'static str)> = BTreeSet::new();
        for (_, item) in scored {
            if seen.insert((item.label.clone(), item.kind)) {
                deduped.push(item);
                if deduped.len() >= 8 {
                    break;
                }
            }
        }
        deduped
    }

    pub fn ghost_completion_for(&self, item: &SuggestionItem) -> Option<String> {
        if item.is_snippet {
            return None;
        }
        let (_, partial) = self.completion_context();
        if !partial.is_empty()
            && item
                .insert
                .to_ascii_lowercase()
                .starts_with(&partial.to_ascii_lowercase())
            && item.insert.len() > partial.len()
        {
            let rest = &item.insert[partial.len()..];
            if rest.ends_with('(') {
                return Some(format!("{rest})"));
            }
            return Some(rest.to_string());
        }
        if partial.is_empty() && self.completion_context().0.is_some() {
            return Some(item.insert.clone());
        }
        None
    }

    pub fn accept_suggestion_item(&mut self, item: &SuggestionItem) -> bool {
        let (_, partial) = self.completion_context();
        if item.is_snippet {
            if !partial.is_empty() {
                self.push_undo();
                let line = &mut self.lines[self.cursor_row];
                let end_b = char_to_byte_idx(line, self.cursor_col);
                let start_col = self.cursor_col.saturating_sub(partial.chars().count());
                let start_b = char_to_byte_idx(line, start_col);
                line.replace_range(start_b..end_b, "");
                self.cursor_col = start_col;
            }
            return self.insert_snippet(&item.insert);
        }

        let start_col = self.cursor_col.saturating_sub(partial.chars().count());
        let line = &self.lines[self.cursor_row];
        let end_b = char_to_byte_idx(line, self.cursor_col);
        let start_b = char_to_byte_idx(line, start_col);
        let existing_prefix = &line[start_b..end_b];
        if existing_prefix == item.insert {
            return false;
        }
        self.push_undo();
        let line = &mut self.lines[self.cursor_row];
        let end_b = char_to_byte_idx(line, self.cursor_col);
        let start_b = char_to_byte_idx(line, start_col);
        let mut insertion = item.insert.clone();
        let move_inside_parens = insertion.ends_with('(');
        if move_inside_parens {
            insertion.push(')');
        }
        line.replace_range(start_b..end_b, &insertion);
        let inserted_chars = insertion.chars().count();
        self.cursor_col = if move_inside_parens {
            start_col + inserted_chars.saturating_sub(1)
        } else {
            start_col + inserted_chars
        };
        self.dirty = true;
        true
    }

    pub fn autocomplete_suggestions(&self) -> Vec<&'static str> {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            return Vec::new();
        }
        builtin_names()
            .iter()
            .copied()
            .filter(|c| c.starts_with(&prefix) && *c != prefix)
            .take(5)
            .collect()
    }

    pub fn autocomplete_suggestions_dynamic(&self) -> Vec<String> {
        let (dot_base, prefix) = self.completion_context();
        if dot_base.is_none() && prefix.is_empty() {
            return Vec::new();
        }
        self.ranked_suggestions(false)
            .into_iter()
            .filter(|item| !item.is_snippet)
            .map(|item| item.insert)
            .take(5)
            .collect()
    }

    pub fn accept_autocomplete(&mut self) -> bool {
        let (dot_base, prefix) = self.completion_context();
        if dot_base.is_none() && prefix.is_empty() {
            return false;
        }
        let suggestions = self.ranked_suggestions(false);
        let Some(top) = suggestions.into_iter().find(|s| !s.is_snippet) else {
            return false;
        };
        self.accept_suggestion_item(&top)
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
        self.ensure_cursor_visible_wrapped(body_rows, code_w, false);
    }

    pub fn ensure_cursor_visible_wrapped(
        &mut self,
        body_rows: usize,
        code_w: usize,
        wrap_mode: bool,
    ) {
        if wrap_mode {
            self.scroll_col = 0;
            let vrows = build_visual_rows(&self.lines, code_w, true);
            let target_vr = visual_row_for_cursor(&vrows, &self.lines, self.cursor_row, self.cursor_col);
            if target_vr < self.scroll_row {
                self.scroll_row = target_vr;
            } else if target_vr >= self.scroll_row + body_rows {
                self.scroll_row = target_vr + 1 - body_rows;
            }
        } else {
            if self.cursor_row < self.scroll_row {
                self.scroll_row = self.cursor_row;
            } else if self.cursor_row >= self.scroll_row + body_rows {
                self.scroll_row = self.cursor_row + 1 - body_rows;
            }
            let vcol = logical_to_visual_col(&self.lines[self.cursor_row], self.cursor_col);
            if vcol < self.scroll_col {
                self.scroll_col = vcol;
            } else if vcol >= self.scroll_col + code_w {
                self.scroll_col = vcol + 1 - code_w;
            }
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

pub fn builtin_signature_help(name: &str) -> Option<&'static str> {
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
        "index_of" => Some("index_of(haystack: text, needle: text) -> number  | -1 if absent"),
        "glob_match" => Some("glob_match(pattern: text, text: text) -> bool  | * and ? wildcards"),
        "str_repeat" => Some("str_repeat(s: text, count: number) -> text"),
        "pad_left" => Some("pad_left(s: text, width: number, fill: text) -> text"),
        "pad_right" => Some("pad_right(s: text, width: number, fill: text) -> text"),
        "html_escape" => Some("html_escape(s: text) -> text  | Escape & < > \" '"),
        "len" => Some("len(x: text | list<T> | map<K,V>) -> number"),
        "text" => Some("text(value) -> text  | Explicit conversion to text"),
        "number" => Some("number(x: text | decimal | number) -> number"),
        "decimal" => Some("decimal(x: text | number | decimal) -> decimal"),
        "push" => Some("push(var list: list<T>, item: T) -> void"),
        "pop" => Some("pop(var list: list<T>) -> T"),
        "insert_at" => Some("insert_at(var list: list<T>, idx: number, item: T) -> void"),
        "remove_at" => Some("remove_at(var list: list<T>, idx: number) -> T"),
        "clear" => Some("clear(var list: list<T>) -> void"),
        "sort" => Some("sort(var list: list<T>) -> void  | In-place ascending sort"),
        "reverse" => Some("reverse(var list: list<T>) -> void  | Reverse elements in-place"),
        "list_contains" => Some("list_contains(list: list<T>, item: T) -> bool"),
        "list_index_of" => Some("list_index_of(list: list<T>, item: T) -> number  | -1 if absent"),
        "list_slice" => Some("list_slice(list: list<T>, start: number, end: number) -> list<T>"),
        "range" => Some("range(start: number, end: number) -> list<number>"),
        "abs" => Some("abs(x: number | decimal) -> number | decimal"),
        "min" => Some("min(a: T, b: T) -> T  | Smaller of two numbers/decimals"),
        "max" => Some("max(a: T, b: T) -> T  | Larger of two numbers/decimals"),
        "clamp" => Some("clamp(x: T, lo: T, hi: T) -> T  | Bound x between lo and hi"),
        "round" => Some("round(x: decimal) -> number  | Nearest integer"),
        "floor" => Some("floor(x: decimal) -> number  | Round down"),
        "ceil" => Some("ceil(x: decimal) -> number  | Round up"),
        "sqrt" => Some("sqrt(x: decimal) -> decimal  | Square root (x >= 0)"),
        "pow" => Some("pow(base: decimal, exp: decimal) -> decimal"),
        "sin" => Some("sin(rad: decimal) -> decimal"),
        "cos" => Some("cos(rad: decimal) -> decimal"),
        "log" => Some("log(x: decimal) -> decimal  | Natural logarithm ln(x)"),
        "rand_int" => Some("rand_int(min: number, max: number) -> number  | Inclusive RNG"),
        "assert" => Some("assert(cond: bool, [msg: text]) -> void  | Fail with E033 if false"),
        "fail" => Some("fail(reason: text) -> void  | Throw catchable E035 error"),
        "map_get" => Some("map_get(m: map<K,V>, key: K) -> V"),
        "map_set" => Some("map_set(var m: map<K,V>, key: K, val: V) -> void"),
        "map_has" => Some("map_has(m: map<K,V>, key: K) -> bool"),
        "map_keys" => Some("map_keys(m: map<K,V>) -> list<K>"),
        "map_values" => Some("map_values(m: map<K,V>) -> list<V>"),
        "map_remove" => Some("map_remove(var m: map<K,V>, key: K) -> bool"),
        "map_clear" => Some("map_clear(var m: map<K,V>) -> void"),
        "map_merge" => Some("map_merge(var dst: map<K,V>, src: map<K,V>) -> void"),
        "file_read" => Some("file_read(path: text) -> text"),
        "file_write" => Some("file_write(path: text, content: text) -> void"),
        "file_append" => Some("file_append(path: text, content: text) -> void"),
        "file_exists" => Some("file_exists(path: text) -> bool"),
        "file_delete" => Some("file_delete(path: text) -> bool"),
        "file_copy" => Some("file_copy(src: text, dst: text) -> void"),
        "file_size" => Some("file_size(path: text) -> number  | Size in bytes"),
        "is_file" => Some("is_file(path: text) -> bool"),
        "is_dir" => Some("is_dir(path: text) -> bool"),
        "path_join" => Some("path_join(a: text, b: text) -> text"),
        "dir_list" => Some("dir_list(path: text) -> list<text>"),
        "mkdir" => Some("mkdir(path: text) -> void  | Create directories recursively"),
        "cwd" => Some("cwd() -> text  | Current working directory"),
        "args" => Some("args() -> list<text>  | Script CLI arguments"),
        "env_get" => Some("env_get(name: text) -> text"),
        "env_set" => Some("env_set(name: text, val: text) -> void"),
        "time_now_ms" => Some("time_now_ms() -> number  | Epoch milliseconds"),
        "time_utc" => Some("time_utc() -> text  | ISO-8601 UTC timestamp"),
        "sleep_ms" => Some("sleep_ms(ms: number) -> void"),
        "json_parse" => Some("json_parse(raw: text) -> map<text, text>"),
        "json_stringify" => Some("json_stringify(val) -> text"),
        "json_pretty" => Some("json_pretty(raw_or_val) -> text  | Pretty-printed JSON"),
        "hex_encode" => Some("hex_encode(s: text) -> text"),
        "hex_decode" => Some("hex_decode(hex: text) -> text"),
        "base64_encode" => Some("base64_encode(s: text) -> text"),
        "base64_decode" => Some("base64_decode(b64: text) -> text"),
        "sha1" => Some("sha1(s: text) -> text  | FIPS 180-1 SHA-1 hex digest"),
        "sha256" => Some("sha256(s: text) -> text  | FIPS 180-4 SHA-256 hex digest"),
        "hmac_sha256" => Some("hmac_sha256(key: text, msg: text) -> text  | RFC 2104 HMAC"),
        "http_get" => Some("http_get(url: text) -> text  | HTTP/HTTPS GET body"),
        "http_post" => Some("http_post(url: text, body: text, ctype: text) -> text  | HTTP/HTTPS POST"),
        "http_put" => Some("http_put(url: text, body: text, ctype: text) -> text  | HTTP/HTTPS PUT"),
        "http_delete" => Some("http_delete(url: text) -> text  | HTTP/HTTPS DELETE"),
        "http_request" => Some("http_request(method, url, headers, body) -> map<text,text>  | status, body, headers, final_url, ok"),
        "http_request_timeout" => Some("http_request_timeout(method, url, headers, body, timeout_ms) -> map<text,text>"),
        "http_listen" => Some("http_listen(addr: text) -> number  | Bind HTTP server e.g. \"127.0.0.1:8080\""),
        "http_port" => Some("http_port(server_id: number) -> number  | Query bound local port"),
        "http_accept" => Some("http_accept(server_id: number) -> map<text,text>  | Block until next HTTP req"),
        "http_try_accept" => Some("http_try_accept(server_id: number, timeout_ms: number) -> map<text,text>  | _ok=\"true\"|\"false\""),
        "http_reply" => Some("http_reply(req: map<text,text>, status: number, ctype, body) -> void"),
        "http_reply_headers" => Some("http_reply_headers(req, status, headers: map<text,text>, body) -> void"),
        "http_redirect" => Some("http_redirect(req, status: number, location: text) -> void"),
        "http_serve_file" => Some("http_serve_file(req: map<text,text>, path: text) -> void  | Serve static asset"),
        "http_close" => Some("http_close(server_id: number) -> void  | Stop HTTP server"),
        "http_route_match" => Some("http_route_match(pattern: text, path: text) -> map<text,text>  | /users/:id/*rest (_matched)"),
        "ws_upgrade" => Some("ws_upgrade(req: map<text,text>) -> number  | Upgrade HTTP request to WebSocket"),
        "ws_connect" => Some("ws_connect(url: text) -> number  | Connect ws:// client socket"),
        "ws_send" => Some("ws_send(ws_id: number, msg: text) -> void  | Send WebSocket text frame"),
        "ws_recv" => Some("ws_recv(ws_id: number) -> text  | Read next WebSocket text frame"),
        "ws_close" => Some("ws_close(ws_id: number) -> void  | Close WebSocket connection"),
        "tcp_listen" => Some("tcp_listen(addr: text) -> number  | Bind raw TCP listener"),
        "tcp_port" => Some("tcp_port(listener_id: number) -> number  | Query TCP listener port"),
        "tcp_accept" => Some("tcp_accept(listener_id: number) -> number  | Accept raw TCP client"),
        "tcp_connect" => Some("tcp_connect(addr: text) -> number  | Connect to TCP host:port"),
        "tcp_send" => Some("tcp_send(conn_id: number, data: text) -> void  | Write UTF-8 to TCP socket"),
        "tcp_recv" => Some("tcp_recv(conn_id: number, max_bytes: number) -> text  | Read from TCP socket"),
        "tcp_set_timeout" => Some("tcp_set_timeout(conn_id: number, timeout_ms: number) -> void"),
        "tcp_remote_addr" => Some("tcp_remote_addr(conn_id: number) -> text"),
        "tcp_close" => Some("tcp_close(handle_id: number) -> void  | Close TCP/WS/UDP handle"),
        "udp_bind" => Some("udp_bind(addr: text) -> number  | Bind UDP socket"),
        "udp_send_to" => Some("udp_send_to(sock_id: number, addr: text, data: text) -> void"),
        "udp_recv_from" => Some("udp_recv_from(sock_id: number, max_bytes: number) -> map<text,text>  | data, from"),
        "udp_close" => Some("udp_close(sock_id: number) -> void"),
        "dns_lookup" => Some("dns_lookup(host: text) -> list<text>  | Resolve IP addresses"),
        "port_is_open" => Some("port_is_open(addr: text, timeout_ms: number) -> bool"),
        "url_parse" => Some("url_parse(url: text) -> map<text,text>  | scheme, host, port, path, query"),
        "query_parse" => Some("query_parse(query: text) -> map<text,text>"),
        "query_stringify" => Some("query_stringify(params: map<text,text>) -> text"),
        "url_encode" => Some("url_encode(s: text) -> text  | Percent-encode query strings"),
        "url_decode" => Some("url_decode(s: text) -> text  | Decode percent-encoded text"),
        "csv_parse" => Some("csv_parse(raw: text) -> list<list<text>>  | Parse CSV table rows"),
        "kv_set" => Some("kv_set(db_path: text, key: text, val: text) -> void  | Persist key in JSON store"),
        "kv_get" => Some("kv_get(db_path: text, key: text) -> text  | Read persisted key"),
        "kv_has" => Some("kv_has(db_path: text, key: text) -> bool  | Check if key exists in store"),
        "kv_delete" => Some("kv_delete(db_path: text, key: text) -> bool  | Delete key from store"),
        "kv_keys" => Some("kv_keys(db_path: text) -> list<text>  | List all stored keys"),
        "exec_cmd" => Some("exec_cmd(cmd: text) -> map<text,text>  | Run shell command (code, stdout, stderr)"),
        "os_name" => Some("os_name() -> text  | Host OS name (e.g. linux, android, windows)"),
        "os_arch" => Some("os_arch() -> text  | Host CPU arch (e.g. aarch64, x86_64)"),
        "uuid_v4" => Some("uuid_v4() -> text  | Generate random RFC 4122 UUID v4"),
        "log_info" => Some("log_info(msg: text) -> void"),
        "log_warn" => Some("log_warn(msg: text) -> void"),
        "log_error" => Some("log_error(msg: text) -> void"),
        "cli_info" => Some("cli_info(msg: text) -> void  | Print themed info message"),
        "cli_success" => Some("cli_success(msg: text) -> void  | Print themed success message"),
        "cli_warn" => Some("cli_warn(msg: text) -> void  | Print themed warning message"),
        "cli_error" => Some("cli_error(msg: text) -> void  | Print themed error message"),
        "cli_debug" => Some("cli_debug(msg: text) -> void  | Print themed debug message"),
        "cli_progress" => Some("cli_progress(label: text, current: number, total: number) -> void  | Render progress bar"),
        "cli_spinner" => Some("cli_spinner(step: number, label: text) -> void  | Render step spinner frame"),
        "cli_theme" => Some("cli_theme(name: text) -> void  | Switch CLI theme (default|kali|dracula|solar|mono)"),
        "cli_prefix" => Some("cli_prefix(prefix: text) -> void  | Set custom CLI message prefix"),
        "cli_timestamps" => Some("cli_timestamps(enabled: bool) -> void  | Toggle [HH:MM:SS] timestamps"),
        "cli_supports_color" => Some("cli_supports_color() -> bool  | Detect if ANSI colors are active"),
        "cli_is_tty" => Some("cli_is_tty() -> bool  | Detect interactive terminal TTY"),
        "char_at" => Some("char_at(s: text, idx: number) -> text  | UTF-8 char at idx"),
        "last_index_of" => Some("last_index_of(haystack: text, needle: text) -> number  | -1 if absent"),
        "count_matches" => Some("count_matches(haystack: text, needle: text) -> number"),
        "remove_prefix" => Some("remove_prefix(s: text, prefix: text) -> text"),
        "remove_suffix" => Some("remove_suffix(s: text, suffix: text) -> text"),
        "title_case" => Some("title_case(s: text) -> text  | Capitalize words"),
        "reverse_text" => Some("reverse_text(s: text) -> text  | Reverse Unicode chars"),
        "lines" => Some("lines(s: text) -> list<text>  | Split text into lines"),
        "tan" => Some("tan(rad: decimal) -> decimal"),
        "atan2" => Some("atan2(y: decimal, x: decimal) -> decimal"),
        "exp" => Some("exp(x: decimal) -> decimal  | e^x"),
        "log10" => Some("log10(x: decimal) -> decimal  | Base-10 logarithm"),
        "pow_int" => Some("pow_int(base: number, exp: number) -> number  | Checked integer power"),
        "sign" => Some("sign(n: number | decimal) -> number  | -1, 0, or 1"),
        "gcd" => Some("gcd(a: number, b: number) -> number  | Greatest common divisor"),
        "lcm" => Some("lcm(a: number, b: number) -> number  | Least common multiple"),
        "math_pi" => Some("math_pi() -> decimal  | 3.141592653589793"),
        "math_e" => Some("math_e() -> decimal  | 2.718281828459045"),
        "list_unique" => Some("list_unique(list: list<T>) -> list<T>  | Preserve first occurrence"),
        "list_take" => Some("list_take(list: list<T>, n: number) -> list<T>"),
        "list_skip" => Some("list_skip(list: list<T>, n: number) -> list<T>"),
        "list_flatten" => Some("list_flatten(lists: list<list<T>>) -> list<T>"),
        "list_sum" => Some("list_sum(list: list<T>) -> T  | Sum of number or decimal list"),
        "list_avg" => Some("list_avg(list: list<T>) -> decimal  | Arithmetic mean"),
        "list_concat" => Some("list_concat(a: list<T>, b: list<T>) -> list<T>"),
        "map_from_lists" => Some("map_from_lists(keys: list<K>, vals: list<V>) -> map<K,V>"),
        "map_get_or" => Some("map_get_or(m: map<K,V>, key: K, fallback: V) -> V"),
        "csv_stringify" => Some("csv_stringify(rows: list<list<text>>) -> text"),
        "json_has_key" => Some("json_has_key(raw_json: text, key: text) -> bool"),
        "path_dirname" => Some("path_dirname(path: text) -> text"),
        "path_basename" => Some("path_basename(path: text) -> text"),
        "path_ext" => Some("path_ext(path: text) -> text"),
        "file_move" => Some("file_move(src: text, dst: text) -> void"),
        "rmdir" => Some("rmdir(path: text) -> void  | Remove directory recursively"),
        "temp_dir" => Some("temp_dir() -> text  | OS temporary directory path"),
        "sha512" => Some("sha512(s: text) -> text  | FIPS 180-4 SHA-512 hex digest"),
        "hmac_sha256_verify" => Some("hmac_sha256_verify(key: text, msg: text, sig_hex: text) -> bool"),
        "base64_url_encode" => Some("base64_url_encode(s: text) -> text  | URL-safe unpadded"),
        "base64_url_decode" => Some("base64_url_decode(b64u: text) -> text"),
        "rand_bytes_hex" => Some("rand_bytes_hex(count: number) -> text  | Random hex token"),
        "ws_ping" => Some("ws_ping(ws_id: number, msg: text) -> void  | Send WS ping frame"),
        "local_ip" => Some("local_ip() -> text  | Best-effort LAN IPv4 address"),
        "http_status_text" => Some("http_status_text(code: number) -> text  | Standard HTTP reason"),
        _ => None,
    }
}

#[derive(Default)]
struct BufferSymbolTables {
    functions: BTreeMap<String, String>,
    vars: BTreeMap<String, String>,
    var_types: BTreeMap<String, String>,
    structs: BTreeMap<String, Vec<(String, String)>>,
    enums: BTreeMap<String, Vec<String>>,
}

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn score_candidate(candidate: &str, query: &str, allow_empty_query: bool) -> Option<i32> {
    if query.is_empty() {
        return if allow_empty_query {
            Some(500 - candidate.len() as i32)
        } else {
            None
        };
    }
    if candidate == query {
        return None;
    }
    if candidate.starts_with(query) {
        return Some(1000 - candidate.len() as i32);
    }
    let cand_l = candidate.to_ascii_lowercase();
    let query_l = query.to_ascii_lowercase();
    if cand_l.starts_with(&query_l) {
        return Some(850 - candidate.len() as i32);
    }
    if query_l.len() >= 2 {
        // Subsequence match with word-boundary bonus (e.g. "htl" -> "http_listen")
        let q_bytes = query_l.as_bytes();
        let c_bytes = cand_l.as_bytes();
        let mut qi = 0usize;
        let mut boundary_hits = 0i32;
        let mut contiguous = 0i32;
        let mut last_match: Option<usize> = None;
        for (ci, &cb) in c_bytes.iter().enumerate() {
            if qi < q_bytes.len() && cb == q_bytes[qi] {
                if ci == 0 || c_bytes[ci - 1] == b'_' {
                    boundary_hits += 1;
                }
                if last_match.is_some_and(|prev| prev + 1 == ci) {
                    contiguous += 1;
                }
                last_match = Some(ci);
                qi += 1;
            }
        }
        if qi == q_bytes.len() {
            return Some(400 + boundary_hits * 40 + contiguous * 20 - candidate.len() as i32);
        }
        if query_l.len() >= 3 && levenshtein_distance(&cand_l, &query_l) <= 1 {
            return Some(250 - candidate.len() as i32);
        }
    }
    None
}

fn keyword_help(kw: &str) -> Option<&'static str> {
    match kw {
        "let" => Some("let name = expr  | Immutable binding (cannot be reassigned)"),
        "var" => Some("var name = expr  | Mutable binding (allows reassignment, +=, push/sort)"),
        "const" => Some("const NAME = expr  | Module-level constant"),
        "fn" => Some("fn name(arg: type) -> ret_type ... end  | Typed function"),
        "struct" => Some("struct Name ... field: type ... end  | Product record type"),
        "enum" => Some("enum Name ... Variant ... end  | Sum/variant type"),
        "if" | "otherwise" => Some("if cond ... otherwise if cond2 ... otherwise ... end  | Strict bool"),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualRow {
    pub file_row: usize,
    pub vis_start: usize,
    pub vis_end: usize,
    pub is_cont: bool,
}

pub fn visual_expand(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + 8);
    for ch in line.chars() {
        if ch == '\t' {
            out.push_str("    ");
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn visual_char_len(line: &str) -> usize {
    line.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

pub fn logical_to_visual_col(line: &str, logical_col: usize) -> usize {
    line.chars()
        .take(logical_col)
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

pub fn build_visual_rows(lines: &[String], text_w: usize, wrap_mode: bool) -> Vec<VisualRow> {
    let w = text_w.max(1);
    let mut rows = Vec::with_capacity(lines.len());
    for (file_row, line) in lines.iter().enumerate() {
        let vlen = visual_char_len(line);
        if !wrap_mode || vlen <= w {
            rows.push(VisualRow {
                file_row,
                vis_start: 0,
                vis_end: vlen,
                is_cont: false,
            });
        } else {
            let mut start = 0usize;
            while start < vlen {
                let end = (start + w).min(vlen);
                rows.push(VisualRow {
                    file_row,
                    vis_start: start,
                    vis_end: end,
                    is_cont: start > 0,
                });
                start = end;
            }
        }
    }
    rows
}

pub fn visual_row_for_cursor(
    vrows: &[VisualRow],
    lines: &[String],
    cursor_row: usize,
    cursor_col: usize,
) -> usize {
    let vcol = lines
        .get(cursor_row)
        .map(|l| logical_to_visual_col(l, cursor_col))
        .unwrap_or(0);
    for (idx, vr) in vrows.iter().enumerate() {
        if vr.file_row != cursor_row {
            continue;
        }
        let is_last_of_line = vrows
            .get(idx + 1)
            .map(|n| n.file_row != cursor_row)
            .unwrap_or(true);
        if is_last_of_line {
            if vcol >= vr.vis_start && vcol <= vr.vis_end {
                return idx;
            }
        } else if vcol >= vr.vis_start && vcol < vr.vis_end {
            return idx;
        }
    }
    vrows
        .iter()
        .rposition(|vr| vr.file_row == cursor_row)
        .unwrap_or(0)
}

pub fn wrap_plain_lines(lines: &[&str], width: usize) -> Vec<String> {
    let w = width.max(1);
    let mut out = Vec::new();
    for &line in lines {
        let expanded = visual_expand(line);
        let chars: Vec<char> = expanded.chars().collect();
        if chars.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut i = 0usize;
        while i < chars.len() {
            let max_end = (i + w).min(chars.len());
            let cut = if max_end == chars.len() {
                max_end
            } else {
                let mut space_cut = None;
                for j in (i + 1..=max_end).rev() {
                    if chars[j - 1] == ' ' {
                        space_cut = Some(j);
                        break;
                    }
                }
                space_cut.unwrap_or(max_end)
            };
            let chunk: String = chars[i..cut].iter().collect();
            out.push(chunk.trim_end_matches(' ').to_string());
            i = cut;
            while i < chars.len() && chars[i] == ' ' && cut < max_end {
                i += 1;
            }
        }
    }
    out
}

pub fn visible_ansi_len(s: &str) -> usize {
    let mut count = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for seq in chars.by_ref() {
                    if seq.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' && c != '\n' {
            count += 1;
        }
    }
    count
}

pub(crate) fn scroll_prompt_tail(prompt: &str, input: &str, cols: usize) -> String {
    let full = format!(" {prompt}{input}");
    let full_chars: Vec<char> = full.chars().collect();
    if full_chars.len() < cols {
        return full;
    }
    let avail = cols.saturating_sub(1).max(1);
    let p_prefix: String = format!(" {prompt}")
        .chars()
        .take(avail.min(18))
        .collect();
    let p_len = p_prefix.chars().count();
    let rem = avail.saturating_sub(p_len + 1);
    if rem == 0 {
        return full_chars.into_iter().take(avail).collect();
    }
    let in_chars: Vec<char> = input.chars().collect();
    let tail_start = in_chars.len().saturating_sub(rem);
    let tail: String = in_chars[tail_start..].iter().collect();
    if tail_start > 0 {
        format!("{p_prefix}…{tail}")
    } else {
        format!("{p_prefix}{tail}")
    }
}

pub(crate) fn format_ln_fixed(n: usize, width: usize) -> String {
    let modulo = 10usize.pow(width as u32);
    let val = if n >= modulo { n % modulo } else { n };
    format!("{val:>width$}", width = width)
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
                    if rows > 0 && cols > 0 {
                        return (rows.max(8), cols.max(24));
                    }
                }
            }
        }
    }
    let env_r = std::env::var("LINES")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0);
    let env_c = std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0);
    if let (Some(r), Some(c)) = (env_r, env_c) {
        return (r.max(8), c.max(24));
    }
    let tput_r = Command::new("tput")
        .arg("lines")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<usize>().ok())
        .filter(|&n| n > 0);
    let tput_c = Command::new("tput")
        .arg("cols")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<usize>().ok())
        .filter(|&n| n > 0);
    (
        env_r.or(tput_r).unwrap_or(24).max(8),
        env_c.or(tput_c).unwrap_or(80).max(24),
    )
}

pub(crate) enum KeyEvent {
    Char(char),
    Enter,
    Tab,
    BackTab,
    CtrlSpace,
    Alt(char),
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
        0 => Ok(KeyEvent::CtrlSpace),
        0x1b => {
            let mut b1 = [0u8; 1];
            if stdin.read_exact(&mut b1).is_err() {
                return Ok(KeyEvent::Esc);
            }
            if b1[0] == b'[' {
                let mut b2 = [0u8; 1];
                if stdin.read_exact(&mut b2).is_err() {
                    return Ok(KeyEvent::Esc);
                }
                match b2[0] {
                    b'A' => Ok(KeyEvent::Up),
                    b'B' => Ok(KeyEvent::Down),
                    b'C' => Ok(KeyEvent::Right),
                    b'D' => Ok(KeyEvent::Left),
                    b'H' => Ok(KeyEvent::Home),
                    b'F' => Ok(KeyEvent::End),
                    b'Z' => Ok(KeyEvent::BackTab),
                    b'1' => read_tilde_or_mod(stdin, b'1'),
                    b'3' => read_tilde_or_mod(stdin, b'3'),
                    b'4' => read_tilde_or_mod(stdin, b'4'),
                    b'5' => read_tilde_or_mod(stdin, b'5'),
                    b'6' => read_tilde_or_mod(stdin, b'6'),
                    _ => Ok(KeyEvent::Esc),
                }
            } else if b1[0] == b'O' {
                let mut b2 = [0u8; 1];
                if stdin.read_exact(&mut b2).is_err() {
                    return Ok(KeyEvent::Esc);
                }
                match b2[0] {
                    b'H' => Ok(KeyEvent::Home),
                    b'F' => Ok(KeyEvent::End),
                    _ => Ok(KeyEvent::Esc),
                }
            } else if b1[0].is_ascii_graphic() {
                Ok(KeyEvent::Alt(b1[0] as char))
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
    let mut output_scroll = 0usize;
    let mut wrap_override: Option<bool> = None;
    let mut manual_suggest = false;
    let mut popup_selected = 0usize;
    let mut suppressed_at: Option<(usize, usize)> = None;

    loop {
        let (rows, cols) = query_terminal_size();
        let wrap_mode = wrap_override.unwrap_or(cols < 72);
        let body_rows = rows.saturating_sub(4).max(3);
        let gutter_w = 6usize;
        let code_w = cols.saturating_sub(gutter_w + 1).max(1);

        tabs[active_tab].ensure_cursor_visible_wrapped(body_rows, code_w, wrap_mode);
        let diag = tabs[active_tab].check().err();
        let git_tag = detect_git_status_tag(Path::new("."));

        let suggestions = tabs[active_tab].ranked_suggestions(manual_suggest);
        let (dot_base, partial) = tabs[active_tab].completion_context();
        let cur_pos = (tabs[active_tab].cursor_row, tabs[active_tab].cursor_col);
        let show_popup = !show_help
            && run_output.is_none()
            && !suggestions.is_empty()
            && (manual_suggest
                || (suppressed_at != Some(cur_pos)
                    && (dot_base.is_some() || partial.len() >= 2)));
        if popup_selected >= suggestions.len() {
            popup_selected = 0;
        }

        if let Some(ref output) = run_output {
            let raw: Vec<&str> = output.lines().collect();
            let wrapped = wrap_plain_lines(&raw, cols.saturating_sub(2).max(1));
            let visible_out = body_rows.saturating_sub(1).max(1);
            let max_scroll = wrapped.len().saturating_sub(visible_out);
            if output_scroll > max_scroll {
                output_scroll = max_scroll;
            }
        } else {
            output_scroll = 0;
        }

        render_screen(
            &tabs,
            active_tab,
            diag.as_ref(),
            git_tag.as_deref(),
            &status_msg,
            show_help,
            run_output.as_deref(),
            output_scroll,
            wrap_mode,
            if show_popup { &suggestions } else { &[] },
            popup_selected,
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
            match key {
                KeyEvent::Up => output_scroll = output_scroll.saturating_sub(1),
                KeyEvent::Down => output_scroll = output_scroll.saturating_add(1),
                KeyEvent::PageUp => {
                    output_scroll = output_scroll.saturating_sub(body_rows.saturating_sub(2).max(1))
                }
                KeyEvent::PageDown => {
                    output_scroll = output_scroll.saturating_add(body_rows.saturating_sub(2).max(1))
                }
                KeyEvent::Home => output_scroll = 0,
                _ => {
                    run_output = None;
                    output_scroll = 0;
                }
            }
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
                    manual_suggest = false;
                    popup_selected = 0;
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
                                manual_suggest = false;
                                popup_selected = 0;
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
                    "Palette (:<ln>|fmt|run|wrap|comment|dup|del|snippet fn/if/try/http_server/ws_server): ",
                ) {
                    if cmd.trim() == "wrap" {
                        let next_wrap = !wrap_mode;
                        wrap_override = Some(next_wrap);
                        tabs[active_tab].scroll_col = 0;
                        status_msg = format!(
                            "Soft wrap {}.",
                            if next_wrap { "enabled" } else { "disabled" }
                        );
                    } else {
                        status_msg = run_editor_palette_cmd(
                            &cmd,
                            &mut tabs[active_tab],
                            &mut run_output,
                            personality,
                        );
                        output_scroll = 0;
                    }
                }
            }
            KeyEvent::Alt('w') | KeyEvent::Alt('W') => {
                let next_wrap = !wrap_mode;
                wrap_override = Some(next_wrap);
                tabs[active_tab].scroll_col = 0;
                status_msg = format!(
                    "Soft wrap {}.",
                    if next_wrap { "enabled" } else { "disabled" }
                );
            }
            KeyEvent::Ctrl('l') => {
                tabs[active_tab].format_in_place();
                manual_suggest = false;
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
                output_scroll = 0;
                status_msg = msg;
            }
            KeyEvent::Ctrl('z') => {
                if tabs[active_tab].undo() {
                    manual_suggest = false;
                    status_msg = "Undo.".to_string();
                }
            }
            KeyEvent::Ctrl('y') => {
                if tabs[active_tab].redo() {
                    manual_suggest = false;
                    status_msg = "Redo.".to_string();
                }
            }
            KeyEvent::Ctrl('t') => {
                let n = tabs.len() + 1;
                tabs.push(EditorBuffer::empty(format!("scratch_{n}.dg")));
                active_tab = tabs.len() - 1;
                manual_suggest = false;
                popup_selected = 0;
                status_msg = "Opened new tab.".to_string();
            }
            KeyEvent::Ctrl('n') => {
                active_tab = (active_tab + 1) % tabs.len();
                manual_suggest = false;
                popup_selected = 0;
            }
            KeyEvent::Ctrl('g') => {
                show_help = true;
                tip_idx = (tip_idx + 1) % DOG_TIPS.len();
                status_msg = DOG_TIPS[tip_idx].to_string();
            }
            KeyEvent::CtrlSpace | KeyEvent::Alt('/') => {
                manual_suggest = true;
                suppressed_at = None;
                popup_selected = 0;
            }
            KeyEvent::Esc => {
                if show_popup {
                    manual_suggest = false;
                    suppressed_at = Some(cur_pos);
                }
            }
            KeyEvent::Char(ch) => {
                tabs[active_tab].insert_char_smart(ch);
                suppressed_at = None;
                popup_selected = 0;
                if ch == '.' {
                    manual_suggest = false;
                }
            }
            KeyEvent::Enter => {
                tabs[active_tab].insert_newline_auto_indent();
                manual_suggest = false;
                suppressed_at = None;
                popup_selected = 0;
            }
            KeyEvent::Tab => {
                if show_popup {
                    if let Some(chosen) = suggestions.get(popup_selected) {
                        tabs[active_tab].accept_suggestion_item(chosen);
                        manual_suggest = false;
                        popup_selected = 0;
                        suppressed_at = None;
                    }
                } else if !tabs[active_tab].accept_autocomplete() {
                    tabs[active_tab].insert_tab();
                }
            }
            KeyEvent::BackTab => {
                if show_popup && !suggestions.is_empty() {
                    popup_selected = (popup_selected + 1) % suggestions.len();
                } else {
                    manual_suggest = true;
                    suppressed_at = None;
                    popup_selected = 0;
                }
            }
            KeyEvent::Backspace => {
                tabs[active_tab].backspace_smart();
                suppressed_at = None;
                popup_selected = 0;
            }
            KeyEvent::Delete => {
                tabs[active_tab].delete_forward();
                suppressed_at = None;
                popup_selected = 0;
            }
            KeyEvent::Left => {
                tabs[active_tab].move_left();
                manual_suggest = false;
                popup_selected = 0;
            }
            KeyEvent::Right => {
                tabs[active_tab].move_right();
                manual_suggest = false;
                popup_selected = 0;
            }
            KeyEvent::WordLeft => {
                tabs[active_tab].move_word_left();
                manual_suggest = false;
            }
            KeyEvent::WordRight => {
                tabs[active_tab].move_word_right();
                manual_suggest = false;
            }
            KeyEvent::Up => {
                if show_popup && !suggestions.is_empty() {
                    popup_selected = if popup_selected == 0 {
                        suggestions.len() - 1
                    } else {
                        popup_selected - 1
                    };
                } else {
                    tabs[active_tab].move_up();
                    manual_suggest = false;
                    popup_selected = 0;
                }
            }
            KeyEvent::Down => {
                if show_popup && !suggestions.is_empty() {
                    popup_selected = (popup_selected + 1) % suggestions.len();
                } else {
                    tabs[active_tab].move_down();
                    manual_suggest = false;
                    popup_selected = 0;
                }
            }
            KeyEvent::PageUp => {
                tabs[active_tab].page_up(body_rows);
                manual_suggest = false;
            }
            KeyEvent::PageDown => {
                tabs[active_tab].page_down(body_rows);
                manual_suggest = false;
            }
            KeyEvent::Home => {
                tabs[active_tab].cursor_col = 0;
                manual_suggest = false;
            }
            KeyEvent::End => {
                let len = tabs[active_tab].lines[tabs[active_tab].cursor_row]
                    .chars()
                    .count();
                tabs[active_tab].cursor_col = len;
                manual_suggest = false;
            }
            KeyEvent::Alt(_) | KeyEvent::Ctrl(_) => {}
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
        return "Unknown snippet. Try: fn, struct, enum, if, match, try, for, while, http_server, ws_server, discord_bot".to_string();
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
        let shown = scroll_prompt_tail(prompt, &input, cols);
        let cur_c = (shown.chars().count() + 1).min(cols.max(1));
        print!("\x1b[{rows};1H\x1b[2K\x1b[48;5;24;38;5;255m{shown}\x1b[K\x1b[0m\x1b[{rows};{cur_c}H");
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

pub(crate) fn render_suggestion_popup_overlay(
    out: &mut String,
    suggestions: &[SuggestionItem],
    selected: usize,
    cursor_screen_r: usize,
    cursor_screen_c: usize,
    max_body_bottom_row: usize,
    rows: usize,
    cols: usize,
) {
    if suggestions.is_empty() || cols < 22 || rows < 8 {
        return;
    }
    let shown_items = suggestions
        .len()
        .min(6)
        .min(max_body_bottom_row.saturating_sub(3).max(1));
    let box_h = shown_items + 2;
    let box_w = cols.saturating_sub(2).min(64).max(20).min(cols);
    let inner_w = box_w.saturating_sub(2);

    let start_col = cursor_screen_c
        .min(cols.saturating_sub(box_w) + 1)
        .max(1);
    let place_below = cursor_screen_r + box_h <= max_body_bottom_row;
    let start_row = if place_below {
        cursor_screen_r + 1
    } else {
        cursor_screen_r.saturating_sub(box_h).max(2)
    };

    let hdr_txt = " Suggestions (Tab accept · ↑↓ · Esc) ";
    let hdr_chars: String = hdr_txt.chars().take(inner_w).collect();
    let hdr_fill = "─".repeat(inner_w.saturating_sub(hdr_chars.chars().count()));
    out.push_str(&format!(
        "\x1b[{start_row};{start_col}H\x1b[48;5;235;38;5;117m┌{hdr_chars}{hdr_fill}┐\x1b[0m"
    ));

    for idx in 0..shown_items {
        let item = &suggestions[idx];
        let r = start_row + 1 + idx;
        let is_sel = idx == selected;
        let row_bg = if is_sel {
            "\x1b[48;5;24;38;5;255m"
        } else {
            "\x1b[48;5;235;38;5;250m"
        };
        let badge = format!("[{:<4}]", item.kind);
        let label_w = if inner_w < 34 { 12 } else { 18 };
        let label_clipped: String = item.label.chars().take(label_w).collect();
        let prefix = format!(" {badge} {label_clipped:<width$} ", width = label_w);
        let rem = inner_w.saturating_sub(prefix.chars().count());
        let detail: String = item.detail.chars().take(rem).collect();
        let line_txt = format!("{prefix}{detail}");
        let clipped: String = line_txt.chars().take(inner_w).collect();
        let pad = " ".repeat(inner_w.saturating_sub(clipped.chars().count()));
        out.push_str(&format!(
            "\x1b[{r};{start_col}H\x1b[38;5;117m│{row_bg}{clipped}{pad}\x1b[48;5;235;38;5;117m│\x1b[0m"
        ));
    }

    let bot_row = start_row + 1 + shown_items;
    let bot_fill = "─".repeat(inner_w);
    out.push_str(&format!(
        "\x1b[{bot_row};{start_col}H\x1b[48;5;235;38;5;117m└{bot_fill}┘\x1b[0m"
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn render_editor_frame_rows(
    tabs: &[EditorBuffer],
    active_tab: usize,
    diag: Option<&DogDiagnostic>,
    git_tag: Option<&str>,
    status_msg: &str,
    show_help: bool,
    run_output: Option<&str>,
    output_scroll: usize,
    wrap_mode: bool,
    suggestions: &[SuggestionItem],
    popup_selected: usize,
    rows: usize,
    cols: usize,
) -> (Vec<String>, usize, usize) {
    let buf = &tabs[active_tab];
    let mut frame_rows: Vec<String> = Vec::with_capacity(rows);

    // Row 1: Strict width-bounded header so narrow Termux screens never wrap row 1
    let mut header = String::new();
    let mut h_used = 0usize;
    let badge_plain = match diag {
        None => "[✓ OK]".to_string(),
        Some(d) => format!("[✗ {}]", d.code),
    };
    let badge_ansi = match diag {
        None => "\x1b[38;5;78;1m[✓ OK]\x1b[22m".to_string(),
        Some(d) => format!("\x1b[38;5;203;1m[✗ {}]\x1b[22m", d.code),
    };
    let wrap_plain = if wrap_mode && cols >= 44 { " [WRAP]" } else { "" };
    let right_plain_len = 1 + wrap_plain.chars().count() + badge_plain.chars().count();
    let brand = if cols >= 28 { " DOG " } else { " D " };
    let brand_len = brand.chars().count();
    header.push_str("\x1b[48;5;33;38;5;255;1m");
    let b_take: String = brand.chars().take(cols).collect();
    h_used += b_take.chars().count();
    header.push_str(&b_take);
    header.push_str("\x1b[22;48;5;235;38;5;250m");

    let left_limit = cols.saturating_sub(right_plain_len).max(brand_len);
    // Active tab first so mobile users always see the active filename
    if h_used < left_limit {
        let act = &tabs[active_tab];
        let dot = if act.dirty { "*" } else { "" };
        let max_t = left_limit.saturating_sub(h_used);
        let raw_lbl = format!(" {}{} ", act.title, dot);
        let clip: String = raw_lbl.chars().take(max_t).collect();
        h_used += clip.chars().count();
        header.push_str(&format!(
            "\x1b[48;5;238;38;5;255;1m{clip}\x1b[22;48;5;235;38;5;246m"
        ));
    }
    for (idx, t) in tabs.iter().enumerate() {
        if idx == active_tab {
            continue;
        }
        let dot = if t.dirty { "*" } else { "" };
        let lbl = format!(" {}{} ", t.title, dot);
        if h_used + lbl.chars().count() <= left_limit {
            h_used += lbl.chars().count();
            header.push_str(&lbl);
        }
    }
    if let Some(branch) = git_tag {
        let g = format!(" git:{branch}");
        if h_used + g.chars().count() <= left_limit {
            h_used += g.chars().count();
            header.push_str(&format!("\x1b[38;5;117m{g}\x1b[38;5;250m"));
        }
    }
    if h_used + right_plain_len <= cols {
        let gap = cols.saturating_sub(h_used + right_plain_len);
        if gap > 0 && gap < cols {
            header.push_str(&" ".repeat(gap.min(1)));
        }
        if !wrap_plain.is_empty() {
            header.push_str("\x1b[38;5;117m [WRAP]\x1b[38;5;250m");
        }
        header.push(' ');
        header.push_str(&badge_ansi);
    }
    frame_rows.push(header);

    let reserved_bottom = 3;
    let body_rows = rows.saturating_sub(1 + reserved_bottom).max(3);
    let gutter_w = 6usize;
    let text_w = cols.saturating_sub(gutter_w + 1).max(1);
    let bracket_match = buf.matching_bracket();
    let error_line = diag.map(|d| d.span.line);
    let ghost = if !show_help && run_output.is_none() {
        suggestions
            .get(popup_selected)
            .and_then(|item| buf.ghost_completion_for(item))
    } else {
        None
    };

    let mut cursor_screen_r = 2usize;
    let mut cursor_screen_c = (gutter_w + 1).min(cols.max(1));

    if show_help {
        let help_raw = [
            "DOG EDITOR — Mobile & Desktop Reference",
            "Ctrl+S Save  Ctrl+O Open  Ctrl+R Run",
            "Ctrl+L Fmt   Ctrl+F Find  Ctrl+P Palette",
            "Ctrl+Space / Alt+/ : Live IntelliSense",
            "Tab Accept   Shift+Tab Cycle  Esc Dismiss",
            "Alt+W : Toggle Soft Line Wrap",
            "Ctrl+D Dup  Ctrl+K Del  Ctrl+U Comment",
            "Ctrl+Z/Y Undo/Redo  Ctrl+T/N Tabs  ^Q Quit",
            "Syntax: let, var, const, use \"mod.dg\"",
            "Strings: \"lit\", $\"interp\", \"\"\"multi\"\"\"",
            "Flow: if / otherwise if / otherwise / end",
            "Press any key to return...",
        ];
        let wrapped = wrap_plain_lines(&help_raw, cols.saturating_sub(2).max(1));
        for r in 0..body_rows {
            let line = wrapped.get(r).map(|s| s.as_str()).unwrap_or("");
            let clipped: String = line.chars().take(cols.saturating_sub(2)).collect();
            frame_rows.push(format!("\x1b[48;5;234;38;5;117m  {clipped}"));
        }
    } else if let Some(output) = run_output {
        let raw_lines: Vec<&str> = if output.is_empty() {
            vec![""]
        } else {
            output.lines().collect()
        };
        let wrapped = wrap_plain_lines(&raw_lines, cols.saturating_sub(2).max(1));
        let out_hdr: String = "  ▶ Output (↑↓ scroll · Esc/Enter back)"
            .chars()
            .take(cols)
            .collect();
        frame_rows.push(format!("\x1b[48;5;235;38;5;78;1m{out_hdr}"));
        for r in 1..body_rows {
            let idx = output_scroll + r - 1;
            let line = wrapped.get(idx).map(|s| s.as_str()).unwrap_or("");
            let clipped: String = line.chars().take(cols.saturating_sub(2)).collect();
            frame_rows.push(format!("\x1b[48;5;234;38;5;252m  {clipped}"));
        }
    } else {
        let vrows = build_visual_rows(&buf.lines, text_w, wrap_mode);
        let cur_vr =
            visual_row_for_cursor(&vrows, &buf.lines, buf.cursor_row, buf.cursor_col);
        if let Some(vr) = vrows.get(cur_vr) {
            let vcol = buf
                .lines
                .get(buf.cursor_row)
                .map(|l| logical_to_visual_col(l, buf.cursor_col))
                .unwrap_or(0);
            cursor_screen_r = cur_vr.saturating_sub(buf.scroll_row) + 2;
            let rel_c = if wrap_mode {
                vcol.saturating_sub(vr.vis_start)
            } else {
                vcol.saturating_sub(buf.scroll_col)
            };
            cursor_screen_c = (gutter_w + rel_c + 1).min(cols.max(1));
        }

        for screen_r in 0..body_rows {
            let vr_idx = buf.scroll_row + screen_r;
            if let Some(vr) = vrows.get(vr_idx) {
                let file_r = vr.file_row;
                let line = &buf.lines[file_r];
                let line_num = file_r + 1;
                let is_active = file_r == buf.cursor_row;
                let is_err = error_line == Some(line_num);
                let trimmed = line.trim_start();
                let row_bg = if is_active {
                    "\x1b[48;5;236m"
                } else {
                    "\x1b[48;5;234m"
                };

                let mut row_s = String::new();
                if vr.is_cont {
                    row_s.push_str(&format!(
                        "{row_bg}\x1b[38;5;240m   ↳ \x1b[38;5;239m│\x1b[38;5;252m"
                    ));
                } else {
                    let symbol_mark = if is_err {
                        "\x1b[38;5;203m✗\x1b[39m"
                    } else if trimmed.starts_with("fn ") {
                        "\x1b[38;5;221mƒ\x1b[39m"
                    } else if trimmed.starts_with("struct ") || trimmed.starts_with("enum ") {
                        "\x1b[38;5;81m◆\x1b[39m"
                    } else {
                        " "
                    };
                    let ln_fg = if is_err {
                        "\x1b[38;5;203;1m"
                    } else if is_active {
                        "\x1b[38;5;221;1m"
                    } else {
                        "\x1b[38;5;242m"
                    };
                    let ln_str = format_ln_fixed(line_num, 4);
                    row_s.push_str(&format!(
                        "{row_bg}{ln_fg}{ln_str}\x1b[22m{symbol_mark}\x1b[38;5;239m│\x1b[38;5;252m"
                    ));
                }

                let expanded = visual_expand(line);
                let vis_slice: String = if wrap_mode {
                    expanded
                        .chars()
                        .skip(vr.vis_start)
                        .take(vr.vis_end.saturating_sub(vr.vis_start).min(text_w))
                        .collect()
                } else {
                    expanded.chars().skip(buf.scroll_col).take(text_w).collect()
                };
                let vis_len = vis_slice.chars().count();
                row_s.push_str(&highlight_dog_line(&vis_slice));

                let mut extra_len = 0usize;
                let is_last_seg_of_line = vrows
                    .get(vr_idx + 1)
                    .map(|n| n.file_row != file_r)
                    .unwrap_or(true);

                if is_active
                    && is_last_seg_of_line
                    && buf.cursor_col == line.chars().count()
                {
                    if let Some(ref g) = ghost {
                        let avail = text_w.saturating_sub(vis_len);
                        if avail > 0 {
                            let clipped_g: String = g.chars().take(avail).collect();
                            extra_len += clipped_g.chars().count();
                            row_s.push_str(&format!(
                                "\x1b[38;5;242m{clipped_g}\x1b[38;5;252m"
                            ));
                        }
                    }
                }

                if let Some((mr, mc)) = bracket_match {
                    if mr == file_r && is_last_seg_of_line {
                        let tag = format!(" [match @{}]", mc + 1);
                        let avail = cols.saturating_sub(gutter_w + vis_len + extra_len);
                        if tag.chars().count() <= avail {
                            row_s.push_str(&format!("\x1b[38;5;214m{tag}\x1b[39m"));
                        }
                    }
                }
                frame_rows.push(row_s);
            } else {
                let empty_gutter: String = "   ~ │".chars().take(cols).collect();
                frame_rows.push(format!("\x1b[48;5;234;38;5;238m{empty_gutter}"));
            }
        }
    }

    // Row rows-1: Contextual Diagnostic / Autocomplete / Signature Tooltip Bar
    if let Some(d) = diag {
        let msg = format!(
            " ✗ {} ({}:{}): {}  →  {}",
            d.code, d.span.line, d.span.column, d.title, d.fix
        );
        let clipped: String = msg.chars().take(cols).collect();
        frame_rows.push(format!("\x1b[48;5;52;38;5;224m{clipped}"));
    } else if let Some(chosen) = suggestions.get(popup_selected) {
        let hint = format!(" ▸ [{}] {}  —  {}", chosen.kind, chosen.label, chosen.detail);
        let clipped: String = hint.chars().take(cols).collect();
        frame_rows.push(format!("\x1b[48;5;24;38;5;159m{clipped}"));
    } else if let Some(sig_hint) = buf.contextual_hint() {
        let clipped: String = format!(" ⓘ {sig_hint}").chars().take(cols).collect();
        frame_rows.push(format!("\x1b[48;5;235;38;5;117m{clipped}"));
    } else {
        let clipped: String = format!(" ▸ {status_msg}").chars().take(cols).collect();
        frame_rows.push(format!("\x1b[48;5;235;38;5;110m{clipped}"));
    }

    // Row rows: Responsive Status & Shortcut Footer
    let footer_raw = if cols < 56 {
        format!(
            " L{}:{} │ {}L │ ^R Run ^S Save ^G Help ^Q Quit",
            buf.cursor_row + 1,
            buf.cursor_col + 1,
            buf.lines.len()
        )
    } else {
        format!(
            " Ln {}, Col {} │ {}L │ {} │ ^Space ^P ^R Run ^S Save Alt+W Wrap ^G Help ^Q Quit",
            buf.cursor_row + 1,
            buf.cursor_col + 1,
            buf.lines.len(),
            if wrap_mode { "WRAP" } else { "NOWRAP" }
        )
    };
    let footer: String = footer_raw.chars().take(cols).collect();
    frame_rows.push(format!("\x1b[48;5;235;38;5;250m{footer}"));

    (frame_rows, cursor_screen_r, cursor_screen_c)
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
    output_scroll: usize,
    wrap_mode: bool,
    suggestions: &[SuggestionItem],
    popup_selected: usize,
    rows: usize,
    cols: usize,
) {
    let (frame_rows, cursor_screen_r, cursor_screen_c) = render_editor_frame_rows(
        tabs,
        active_tab,
        diag,
        git_tag,
        status_msg,
        show_help,
        run_output,
        output_scroll,
        wrap_mode,
        suggestions,
        popup_selected,
        rows,
        cols,
    );

    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[H");
    let total = frame_rows.len();
    for (idx, row_str) in frame_rows.into_iter().enumerate() {
        out.push_str(&row_str);
        out.push_str("\x1b[K\x1b[0m");
        if idx + 1 < total {
            out.push_str("\r\n");
        }
    }

    let body_rows = rows.saturating_sub(4).max(3);
    if !suggestions.is_empty() && !show_help && run_output.is_none() {
        render_suggestion_popup_overlay(
            &mut out,
            suggestions,
            popup_selected,
            cursor_screen_r,
            cursor_screen_c,
            1 + body_rows,
            rows,
            cols,
        );
    }

    let clamped_r = cursor_screen_r.clamp(2, (1 + body_rows).max(2));
    let clamped_c = cursor_screen_c.clamp(1, cols.max(1));
    out.push_str(&format!("\x1b[{clamped_r};{clamped_c}H\x1b[?25h"));

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
        if ch == '#' || (ch == '/' && chars.get(i + 1) == Some(&'/')) {
            let rest: String = chars[i..].iter().collect();
            out.push_str(&format!("\x1b[3;38;5;244m{rest}\x1b[23;39m"));
            break;
        } else if ch == '$' && chars.get(i + 1) == Some(&'"') {
            out.push_str("\x1b[38;5;214m$\x1b[38;5;114m\"");
            i += 2;
            while i < chars.len() {
                let c = chars[i];
                if c == '"' {
                    out.push('"');
                    i += 1;
                    break;
                } else if c == '\\' && i + 1 < chars.len() {
                    out.push(c);
                    out.push(chars[i + 1]);
                    i += 2;
                } else if c == '{' && chars.get(i + 1) == Some(&'{') {
                    out.push_str("{{");
                    i += 2;
                } else if c == '{' {
                    out.push_str("\x1b[38;5;214m{\x1b[39m");
                    i += 1;
                    let expr_start = i;
                    let mut depth = 1usize;
                    let mut in_inner_str = false;
                    while i < chars.len() && depth > 0 {
                        let ic = chars[i];
                        if in_inner_str {
                            if ic == '\\' && i + 1 < chars.len() {
                                i += 2;
                                continue;
                            } else if ic == '"' {
                                in_inner_str = false;
                            }
                        } else if ic == '"' {
                            in_inner_str = true;
                        } else if ic == '{' {
                            depth += 1;
                        } else if ic == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        i += 1;
                    }
                    let expr_str: String = chars[expr_start..i].iter().collect();
                    out.push_str(&highlight_dog_line(&expr_str));
                    if i < chars.len() && chars[i] == '}' {
                        out.push_str("\x1b[38;5;214m}\x1b[38;5;114m");
                        i += 1;
                    } else {
                        out.push_str("\x1b[38;5;114m");
                    }
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            out.push_str("\x1b[39m");
        } else if ch == '"' && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'"') {
            out.push_str("\x1b[38;5;114m\"\"\"");
            i += 3;
            while i < chars.len() {
                if chars[i] == '"'
                    && chars.get(i + 1) == Some(&'"')
                    && chars.get(i + 2) == Some(&'"')
                {
                    out.push_str("\"\"\"");
                    i += 3;
                    break;
                }
                out.push(chars[i]);
                i += 1;
            }
            out.push_str("\x1b[39m");
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
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
            {
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
            } else if builtin_names().contains(&word.as_str()) {
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
