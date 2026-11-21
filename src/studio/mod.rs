use crate::checker::TypeChecker;
use crate::config::Personality;
use crate::diagnostics::DogDiagnostic;
use crate::editor::{
    highlight_dog_line, query_terminal_size, read_key, BufferRunIo, EditorBuffer, KeyEvent,
    TermGuard,
};
use crate::interpreter::{resolve_program, run_source_with_io};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Explorer,
    Editor,
}

pub fn launch_studio(root_dir: Option<&str>, personality: Personality) -> Result<(), String> {
    let base = PathBuf::from(root_dir.unwrap_or("."));
    if !base.exists() {
        return Err(format!("Studio directory '{}' does not exist.", base.display()));
    }

    let mut files = discover_workspace_files(&base);
    let mut selected_idx = 0usize;

    let mut buf = if let Some(first_dg) = files.iter().find(|p| p.extension().and_then(|s| s.to_str()) == Some("dg")) {
        EditorBuffer::from_file(&first_dg.display().to_string())?
    } else {
        EditorBuffer::empty("main.dg")
    };

    let _guard = TermGuard::enter_raw()?;
    let stdin = io::stdin();
    let mut lock = stdin.lock();

    let mut focus = FocusPane::Editor;
    let mut status_msg = String::from("DOG Studio | Ctrl+W Switch Pane | Ctrl+P Palette | Ctrl+R Run | Ctrl+Q Quit");
    let mut run_output: Option<String> = None;
    let git_branch = detect_git_branch(&base);

    loop {
        let (rows, cols) = query_terminal_size();
        let (diag, symbols) = inspect_buffer(&buf);

        render_studio(
            &files,
            selected_idx,
            &buf,
            focus,
            diag.as_ref(),
            &symbols,
            &git_branch,
            &status_msg,
            run_output.as_deref(),
            rows,
            cols,
        );

        let key = match read_key(&mut lock) {
            Ok(k) => k,
            Err(_) => break,
        };

        if run_output.is_some() {
            run_output = None;
            continue;
        }

        match key {
            KeyEvent::Ctrl('q') => {
                if buf.dirty {
                    status_msg = "Unsaved changes! Press Ctrl+S to save or Ctrl+X to discard & quit.".to_string();
                } else {
                    break;
                }
            }
            KeyEvent::Ctrl('x') => break,
            KeyEvent::Ctrl('w') => {
                focus = match focus {
                    FocusPane::Explorer => FocusPane::Editor,
                    FocusPane::Editor => FocusPane::Explorer,
                };
                status_msg = match focus {
                    FocusPane::Explorer => "Focus: Project Explorer (Up/Down + Enter to open)".to_string(),
                    FocusPane::Editor => "Focus: Code Editor".to_string(),
                };
            }
            KeyEvent::Ctrl('p') => {
                if let Some(cmd) = prompt_studio_line(&mut lock, rows, "Command (run|save|fmt|refresh|open <file>): ") {
                    let trimmed = cmd.trim();
                    if trimmed == "run" {
                        run_output = Some(execute_studio_buffer(&buf, personality));
                    } else if trimmed == "save" {
                        status_msg = save_studio_buffer(&mut lock, rows, &mut buf);
                    } else if trimmed == "fmt" {
                        buf.format_in_place();
                        status_msg = "Formatted active buffer.".to_string();
                    } else if trimmed == "refresh" {
                        files = discover_workspace_files(&base);
                        selected_idx = selected_idx.min(files.len().saturating_sub(1));
                        status_msg = format!("Refreshed workspace ({} files).", files.len());
                    } else if let Some(rest) = trimmed.strip_prefix("open ") {
                        let p = rest.trim();
                        match EditorBuffer::from_file(p) {
                            Ok(new_b) => {
                                buf = new_b;
                                status_msg = format!("Opened '{p}'.");
                            }
                            Err(e) => status_msg = e,
                        }
                    } else if !trimmed.is_empty() {
                        status_msg = format!("Unknown palette command '{trimmed}'.");
                    }
                }
            }
            KeyEvent::Ctrl('s') => {
                status_msg = save_studio_buffer(&mut lock, rows, &mut buf);
                files = discover_workspace_files(&base);
            }
            KeyEvent::Ctrl('l') => {
                buf.format_in_place();
                status_msg = "Formatted with 4-space DOG style.".to_string();
            }
            KeyEvent::Ctrl('r') => {
                run_output = Some(execute_studio_buffer(&buf, personality));
                status_msg = "Executed active buffer.".to_string();
            }
            KeyEvent::Ctrl('z') => {
                if buf.undo() {
                    status_msg = "Undo.".to_string();
                }
            }
            KeyEvent::Ctrl('y') => {
                if buf.redo() {
                    status_msg = "Redo.".to_string();
                }
            }
            _ => match focus {
                FocusPane::Explorer => match key {
                    KeyEvent::Up => {
                        selected_idx = selected_idx.saturating_sub(1);
                    }
                    KeyEvent::Down => {
                        if selected_idx + 1 < files.len() {
                            selected_idx += 1;
                        }
                    }
                    KeyEvent::Enter => {
                        if let Some(target) = files.get(selected_idx) {
                            let path_str = target.display().to_string();
                            match EditorBuffer::from_file(&path_str) {
                                Ok(new_buf) => {
                                    buf = new_buf;
                                    focus = FocusPane::Editor;
                                    status_msg = format!("Loaded '{path_str}'.");
                                }
                                Err(e) => status_msg = e,
                            }
                        }
                    }
                    _ => {}
                },
                FocusPane::Editor => match key {
                    KeyEvent::Char(ch) => buf.insert_char(ch),
                    KeyEvent::Enter => buf.insert_newline_auto_indent(),
                    KeyEvent::Tab => buf.insert_tab(),
                    KeyEvent::Backspace => buf.backspace(),
                    KeyEvent::Left => buf.move_left(),
                    KeyEvent::Right => buf.move_right(),
                    KeyEvent::Up => buf.move_up(),
                    KeyEvent::Down => buf.move_down(),
                    KeyEvent::Home => buf.cursor_col = 0,
                    KeyEvent::End => {
                        buf.cursor_col = buf.lines[buf.cursor_row].chars().count();
                    }
                    _ => {}
                },
            },
        }
    }

    Ok(())
}

fn save_studio_buffer(lock: &mut io::StdinLock<'_>, rows: usize, buf: &mut EditorBuffer) -> String {
    let target_path = if let Some(ref p) = buf.path {
        Some(p.clone())
    } else {
        prompt_studio_line(lock, rows, "Save as (.dg): ")
    };
    if let Some(p) = target_path {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return match buf.save_to(trimmed) {
                Ok(()) => format!("Saved '{trimmed}'."),
                Err(e) => e,
            };
        }
    }
    "Save cancelled.".to_string()
}

fn execute_studio_buffer(buf: &EditorBuffer, personality: Personality) -> String {
    let name = buf.path.as_deref().unwrap_or(&buf.title);
    let src = buf.text();
    let mut runner_io = BufferRunIo {
        output: String::new(),
    };
    match run_source_with_io(name, &src, &mut runner_io) {
        Ok(()) => {
            if runner_io.output.is_empty() {
                "(Finished with no stdout output)".to_string()
            } else {
                runner_io.output
            }
        }
        Err(d) => d.format_with(personality, Some(&src)),
    }
}

fn inspect_buffer(buf: &EditorBuffer) -> (Option<DogDiagnostic>, Vec<(String, String)>) {
    let name = buf.path.as_deref().unwrap_or(&buf.title);
    let text = buf.text();
    match resolve_program(name, &text) {
        Ok(prog) => {
            let mut checker = TypeChecker::new();
            match checker.check(&prog) {
                Ok(()) => (None, checker.list_symbols()),
                Err(d) => (Some(d), checker.list_symbols()),
            }
        }
        Err(d) => (Some(d), Vec::new()),
    }
}

fn discover_workspace_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_files(root, &mut out);
    out.sort();
    out
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if name == "target" || name == "node_modules" || name.starts_with('.') {
                continue;
            }
        }
        if p.is_dir() {
            collect_files(&p, out);
        } else if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            if matches!(ext, "dg" | "pack" | "lock" | "md") {
                out.push(p);
            }
        }
    }
}

fn detect_git_branch(root: &Path) -> String {
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
    {
        if out.status.success() {
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !b.is_empty() {
                return b;
            }
        }
    }
    "no-git".to_string()
}

fn prompt_studio_line(stdin: &mut io::StdinLock<'_>, rows: usize, prompt: &str) -> Option<String> {
    let mut input = String::new();
    loop {
        print!("\x1b[{rows};1H\x1b[2K\x1b[44;97m {prompt}{input}\x1b[0m");
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
fn render_studio(
    files: &[PathBuf],
    selected_idx: usize,
    buf: &EditorBuffer,
    focus: FocusPane,
    diag: Option<&DogDiagnostic>,
    symbols: &[(String, String)],
    git_branch: &str,
    status_msg: &str,
    run_output: Option<&str>,
    rows: usize,
    cols: usize,
) {
    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[H");

    let dirty_mark = if buf.dirty { "*" } else { "" };
    let diag_badge = match diag {
        None => "\x1b[32m[OK]\x1b[44;97m".to_string(),
        Some(d) => format!("\x1b[31m[{}]\x1b[44;97m", d.code),
    };
    let header = format!(
        " DOG STUDIO | {}{} {} | git:{} ",
        buf.title, dirty_mark, diag_badge, git_branch
    );
    out.push_str(&format!("\x1b[44;97m{header}\x1b[K\x1b[0m\r\n"));

    let body_rows = rows.saturating_sub(3).max(4);
    let show_inspector = cols >= 96;
    let left_w = if cols >= 70 { 22usize } else { 0usize };
    let right_w = if show_inspector { 26usize } else { 0usize };
    let center_w = cols
        .saturating_sub(left_w + right_w + if left_w > 0 { 2 } else { 0 } + if right_w > 0 { 2 } else { 0 })
        .max(20);

    let mut scroll_row = buf.scroll_row;
    if buf.cursor_row < scroll_row {
        scroll_row = buf.cursor_row;
    } else if buf.cursor_row >= scroll_row + body_rows {
        scroll_row = buf.cursor_row + 1 - body_rows;
    }

    if let Some(output) = run_output {
        let lines: Vec<&str> = output.lines().collect();
        out.push_str("\x1b[1;32m--- DOG Studio Execution Output (press any key) ---\x1b[K\x1b[0m\r\n");
        for r in 1..body_rows {
            let l = lines.get(r - 1).copied().unwrap_or("");
            out.push_str(&format!("{l}\x1b[K\r\n"));
        }
    } else {
        let file_scroll = selected_idx.saturating_sub(body_rows.saturating_sub(2));
        for r in 0..body_rows {
            // Left pane: Project Explorer
            if left_w > 0 {
                let title_style = if focus == FocusPane::Explorer {
                    "\x1b[1;36m"
                } else {
                    "\x1b[90m"
                };
                if r == 0 {
                    let hdr = format!("{:<width$}", "FILES", width = left_w);
                    out.push_str(&format!("{title_style}{hdr}\x1b[0m │ "));
                } else {
                    let f_idx = file_scroll + r - 1;
                    if let Some(p) = files.get(f_idx) {
                        let name = p
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("?");
                        let clipped: String = name.chars().take(left_w).collect();
                        let padded = format!("{:<width$}", clipped, width = left_w);
                        if f_idx == selected_idx {
                            out.push_str(&format!("\x1b[7m{padded}\x1b[0m │ "));
                        } else {
                            out.push_str(&format!("{padded} │ "));
                        }
                    } else {
                        out.push_str(&format!("{:<width$} │ ", "", width = left_w));
                    }
                }
            }

            // Center pane: Code Editor
            let file_r = scroll_row + r;
            let gutter_w = 5usize;
            let code_w = center_w.saturating_sub(gutter_w).max(8);
            if let Some(line) = buf.lines.get(file_r) {
                let ln = file_r + 1;
                let is_err = diag.is_some_and(|d| d.span.line == ln);
                if is_err {
                    out.push_str(&format!("\x1b[31m{ln:>3}!\x1b[0m "));
                } else {
                    out.push_str(&format!("\x1b[90m{ln:>3} \x1b[0m "));
                }
                let clipped: String = line.chars().take(code_w).collect();
                let pad_spaces = code_w.saturating_sub(clipped.chars().count());
                out.push_str(&highlight_dog_line(&clipped));
                out.push_str(&" ".repeat(pad_spaces));
            } else {
                let empty_code = format!("~{:<width$}", "", width = code_w.saturating_sub(1));
                out.push_str(&format!("\x1b[90m    {empty_code}\x1b[0m"));
            }

            // Right pane: Live Symbols
            if right_w > 0 {
                out.push_str(" │ ");
                if r == 0 {
                    out.push_str(&format!("\x1b[1;33m{:<width$}\x1b[0m", "SYMBOLS", width = right_w));
                } else if let Some((sym, ty)) = symbols.get(r - 1) {
                    let entry = format!("{sym}: {ty}");
                    let clipped: String = entry.chars().take(right_w).collect();
                    out.push_str(&format!("{:<width$}", clipped, width = right_w));
                } else {
                    out.push_str(&format!("{:<width$}", "", width = right_w));
                }
            }

            out.push_str("\x1b[K\r\n");
        }
    }

    if let Some(d) = diag {
        let msg = format!("{} ({}:{}): {}", d.code, d.span.line, d.span.column, d.title);
        let clipped: String = msg.chars().take(cols).collect();
        out.push_str(&format!("\x1b[41;97m{clipped}\x1b[K\x1b[0m\r\n"));
    } else {
        let clipped: String = status_msg.chars().take(cols).collect();
        out.push_str(&format!("\x1b[34m{clipped}\x1b[K\x1b[0m\r\n"));
    }

    let footer = format!(
        "Ln {}:{} | ^W Pane  ^P Palette  ^R Run  ^S Save  ^L Fmt  ^Q Quit",
        buf.cursor_row + 1,
        buf.cursor_col + 1
    );
    let clipped_footer: String = footer.chars().take(cols).collect();
    out.push_str(&format!("\x1b[7m{clipped_footer}\x1b[K\x1b[0m"));

    let cursor_r = buf.cursor_row.saturating_sub(scroll_row) + 2;
    let left_offset = if left_w > 0 { left_w + 3 } else { 0 };
    let cursor_c = left_offset + 5 + buf.cursor_col + 1;
    out.push_str(&format!("\x1b[{cursor_r};{cursor_c}H\x1b[?25h"));

    print!("{out}");
    let _ = io::stdout().flush();
}
