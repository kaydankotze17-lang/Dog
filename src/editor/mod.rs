use crate::config::Personality;
use crate::diagnostics::DogDiagnostic;
use crate::formatter::format_source;
use crate::interpreter::{check_source, run_source_with_io};
use crate::stdlib::InterpreterIo;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

const DOG_TIPS: &[&str] = &[
    "Use 4 spaces per block; `end` always closes blocks.",
    "Prefer `let` by default and `var` only when mutation is intentional.",
    "Convert answer() text explicitly using number() or decimal().",
    "Press Ctrl+L anytime to auto-format your code to 4-space DOG style.",
    "Press Ctrl+R to run the current buffer without leaving the editor.",
];

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
        if self.undo_stack.len() > 200 {
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
        self.cursor_row = prev.cursor_row;
        self.cursor_col = prev.cursor_col;
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
        self.cursor_row = next.cursor_row;
        self.cursor_col = next.cursor_col;
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
            "if" | "otherwise" | "while" | "repeat" | "for" | "fn"
        ) {
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
            ')' => ('(', false),
            ']' => ('[', false),
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

    pub fn autocomplete_suggestions(&self) -> Vec<&'static str> {
        let line = &self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        let prefix: String = line[..byte_idx]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if prefix.is_empty() {
            return Vec::new();
        }
        const CANDIDATES: &[&str] = &[
            "say(",
            "answer(",
            "lower(",
            "upper(",
            "number(",
            "decimal(",
            "text(",
            "len(",
            "trim(",
            "contains(",
            "split(",
            "join(",
            "push(",
            "pop(",
            "assert(",
            "let ",
            "var ",
            "const ",
            "if ",
            "otherwise",
            "while ",
            "repeat ",
            "for ",
            "fn ",
            "return ",
            "end",
        ];
        CANDIDATES
            .iter()
            .copied()
            .filter(|c| c.starts_with(&prefix) && *c != prefix)
            .take(5)
            .collect()
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

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

struct TermGuard {
    saved_stty: Option<String>,
}

impl TermGuard {
    fn enter_raw() -> Result<Self, String> {
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

fn query_terminal_size() -> (usize, usize) {
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

enum KeyEvent {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Ctrl(char),
    Esc,
}

fn read_key(stdin: &mut io::StdinLock<'_>) -> io::Result<KeyEvent> {
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
            // Decode UTF-8 multi-byte character cleanly
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

struct BufferRunIo {
    output: String,
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
        let buf = &mut tabs[active_tab];
        let diag = buf.check().err();
        render_screen(
            &tabs,
            active_tab,
            diag.as_ref(),
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
                    status_msg = "Unsaved changes! Press Ctrl+S to save or Ctrl+X to discard & exit.".to_string();
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
                    prompt_line(&mut lock, rows, "Save as (.dg): ")
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
                if let Some(path) = prompt_line(&mut lock, rows, "Open file: ") {
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
                if let Some(needle) = prompt_line(&mut lock, rows, "Find: ") {
                    if !needle.is_empty() {
                        if let Some(repl) =
                            prompt_line(&mut lock, rows, "Replace with (blank = find next): ")
                        {
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
            KeyEvent::Ctrl('l') => {
                tabs[active_tab].format_in_place();
                status_msg = "Formatted buffer with 4-space DOG indentation.".to_string();
            }
            KeyEvent::Ctrl('r') => {
                let b = &tabs[active_tab];
                let name = b.path.as_deref().unwrap_or(&b.title);
                let src = b.text();
                let mut runner_io = BufferRunIo {
                    output: String::new(),
                };
                match run_source_with_io(name, &src, &mut runner_io) {
                    Ok(()) => {
                        let out = if runner_io.output.is_empty() {
                            "(Program finished with no output)".to_string()
                        } else {
                            runner_io.output
                        };
                        run_output = Some(out);
                        status_msg = "Executed buffer successfully.".to_string();
                    }
                    Err(d) => {
                        run_output = Some(d.format_with(personality, Some(&src)));
                        status_msg = format!("Execution stopped with {}.", d.code);
                    }
                }
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
            KeyEvent::Char(ch) => tabs[active_tab].insert_char(ch),
            KeyEvent::Enter => tabs[active_tab].insert_newline_auto_indent(),
            KeyEvent::Tab => tabs[active_tab].insert_tab(),
            KeyEvent::Backspace => tabs[active_tab].backspace(),
            KeyEvent::Left => tabs[active_tab].move_left(),
            KeyEvent::Right => tabs[active_tab].move_right(),
            KeyEvent::Up => tabs[active_tab].move_up(),
            KeyEvent::Down => tabs[active_tab].move_down(),
            KeyEvent::Home => tabs[active_tab].cursor_col = 0,
            KeyEvent::End => {
                let len = tabs[active_tab].lines[tabs[active_tab].cursor_row]
                    .chars()
                    .count();
                tabs[active_tab].cursor_col = len;
            }
            _ => {}
        }
    }

    Ok(())
}

fn prompt_line(stdin: &mut io::StdinLock<'_>, rows: usize, prompt: &str) -> Option<String> {
    let mut input = String::new();
    loop {
        print!("\x1b[{rows};1H\x1b[2K\x1b[44;37m {prompt}{input}\x1b[0m");
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
    status_msg: &str,
    show_help: bool,
    run_output: Option<&str>,
    rows: usize,
    cols: usize,
) {
    let buf = &tabs[active_tab];
    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[H");

    // Row 1: Kali navy/blue header + Tabs
    out.push_str("\x1b[44;97m DOG EDITOR ");
    for (idx, t) in tabs.iter().enumerate() {
        let marker = if t.dirty { "*" } else { "" };
        if idx == active_tab {
            out.push_str(&format!("\x1b[42;30m {}{} \x1b[44;97m", t.title, marker));
        } else {
            out.push_str(&format!(" {}{} ", t.title, marker));
        }
    }
    let check_badge = match diag {
        None => "\x1b[32m [OK] ",
        Some(d) => {
            out.push_str(&format!("\x1b[31m [{}] ", d.code));
            ""
        }
    };
    out.push_str(check_badge);
    out.push_str("\x1b[K\x1b[0m\r\n");

    let reserved_bottom = 3;
    let body_rows = rows.saturating_sub(1 + reserved_bottom).max(3);
    let mut scroll_row = buf.scroll_row;
    if buf.cursor_row < scroll_row {
        scroll_row = buf.cursor_row;
    } else if buf.cursor_row >= scroll_row + body_rows {
        scroll_row = buf.cursor_row + 1 - body_rows;
    }

    let bracket_match = buf.matching_bracket();
    let error_line = diag.map(|d| d.span.line);

    if show_help {
        let help_lines = [
            "DOG EDITOR — Keyboard Shortcuts & Syntax Reference",
            "---------------------------------------------------",
            "Ctrl+S  Save file         Ctrl+O  Open file",
            "Ctrl+R  Run buffer        Ctrl+L  Format (4-space)",
            "Ctrl+F  Find & Replace    Ctrl+Z/Y Undo / Redo",
            "Ctrl+T  New tab           Ctrl+N  Next tab",
            "Ctrl+G  Toggle help       Ctrl+Q  Quit (Ctrl+X force)",
            "",
            "DOG Syntax: let (immutable), var (mutable), const",
            "Built-ins : say(), answer(), lower(), upper(), number(), decimal()",
            "Blocks    : if ... otherwise ... end | while ... end | fn ... end",
            "Press any key to return to editor...",
        ];
        for r in 0..body_rows {
            let line = help_lines.get(r).copied().unwrap_or("");
            out.push_str(&format!("\x1b[36m{line}\x1b[K\x1b[0m\r\n"));
        }
    } else if let Some(output) = run_output {
        let mut lines: Vec<&str> = output.lines().collect();
        if lines.is_empty() {
            lines.push("");
        }
        out.push_str("\x1b[1;32m--- DOG Execution Output (press any key) ---\x1b[K\x1b[0m\r\n");
        for r in 1..body_rows {
            let line = lines.get(r - 1).copied().unwrap_or("");
            out.push_str(&format!("{line}\x1b[K\r\n"));
        }
    } else {
        let gutter_w = 5usize;
        let text_w = cols.saturating_sub(gutter_w + 1).max(10);
        for screen_r in 0..body_rows {
            let file_r = scroll_row + screen_r;
            if let Some(line) = buf.lines.get(file_r) {
                let line_num = file_r + 1;
                let is_err = error_line == Some(line_num);
                if is_err {
                    out.push_str(&format!("\x1b[31m{line_num:>4}!\x1b[0m "));
                } else {
                    out.push_str(&format!("\x1b[90m{line_num:>4} \x1b[0m "));
                }
                let clipped: String = line.chars().take(text_w).collect();
                out.push_str(&highlight_dog_line(&clipped));
                if let Some((mr, _)) = bracket_match {
                    if mr == file_r {
                        out.push_str(" \x1b[90m[match]\x1b[0m");
                    }
                }
                out.push_str("\x1b[K\r\n");
            } else {
                out.push_str("\x1b[90m   ~\x1b[0m\x1b[K\r\n");
            }
        }
    }

    // Bottom Diagnostic / Autocomplete Bar
    let suggestions = buf.autocomplete_suggestions();
    if let Some(d) = diag {
        let msg = format!(
            "{} ({}:{}): {} | FIX: {}",
            d.code, d.span.line, d.span.column, d.title, d.fix
        );
        let clipped: String = msg.chars().take(cols).collect();
        out.push_str(&format!("\x1b[41;97m{clipped}\x1b[K\x1b[0m\r\n"));
    } else if !suggestions.is_empty() {
        let hint = format!("Suggest: {}", suggestions.join("  "));
        let clipped: String = hint.chars().take(cols).collect();
        out.push_str(&format!("\x1b[44;37m{clipped}\x1b[K\x1b[0m\r\n"));
    } else {
        let clipped: String = status_msg.chars().take(cols).collect();
        out.push_str(&format!("\x1b[34m{clipped}\x1b[K\x1b[0m\r\n"));
    }

    // Status + Shortcut bar
    let pos_info = format!("Ln {}:{}", buf.cursor_row + 1, buf.cursor_col + 1);
    let shortcuts = "^S Save  ^R Run  ^L Fmt  ^F Find  ^G Help  ^Q Quit";
    let footer: String = format!("{pos_info} | {shortcuts}").chars().take(cols).collect();
    out.push_str(&format!("\x1b[7m{footer}\x1b[K\x1b[0m"));

    // Position cursor inside code pane
    let cursor_screen_r = buf.cursor_row.saturating_sub(scroll_row) + 2;
    let cursor_screen_c = 6 + buf.cursor_col + 1;
    out.push_str(&format!("\x1b[{cursor_screen_r};{cursor_screen_c}H\x1b[?25h"));

    print!("{out}");
    let _ = io::stdout().flush();
}

fn highlight_dog_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return format!("\x1b[90m{line}\x1b[0m");
    }

    let mut out = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            out.push_str("\x1b[32m\"");
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '"' && (i == 0 || chars[i - 1] != '\\') {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str("\x1b[0m");
        } else if ch.is_ascii_digit() {
            out.push_str("\x1b[36m");
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                out.push(chars[i]);
                i += 1;
            }
            out.push_str("\x1b[0m");
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
                    | "end"
                    | "or"
                    | "and"
                    | "not"
                    | "true"
                    | "false"
                    | "fn"
                    | "return"
                    | "while"
                    | "repeat"
                    | "for"
                    | "in"
                    | "break"
                    | "continue"
            ) {
                out.push_str(&format!("\x1b[1;34m{word}\x1b[0m"));
            } else if matches!(
                word.as_str(),
                "say"
                    | "answer"
                    | "lower"
                    | "upper"
                    | "number"
                    | "decimal"
                    | "text"
                    | "len"
                    | "trim"
                    | "contains"
                    | "split"
                    | "join"
                    | "push"
                    | "pop"
                    | "abs"
                    | "min"
                    | "max"
                    | "assert"
            ) {
                out.push_str(&format!("\x1b[33m{word}\x1b[0m"));
            } else {
                out.push_str(&word);
            }
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}
