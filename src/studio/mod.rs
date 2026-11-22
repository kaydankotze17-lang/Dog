use crate::checker::TypeChecker;
use crate::config::Personality;
use crate::diagnostics::DogDiagnostic;
use crate::editor::{
    build_visual_rows, format_ln_fixed, highlight_dog_line, logical_to_visual_col,
    query_terminal_size, read_key, render_suggestion_popup_overlay, scroll_prompt_tail,
    visual_expand, visual_row_for_cursor, wrap_plain_lines, BufferRunIo, EditorBuffer, KeyEvent,
    SuggestionItem, TermGuard,
};
use crate::interpreter::{resolve_program, run_source_with_io};
use crate::prompt::detect_git_status_tag;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

    let mut buf = if let Some(first_dg) = files
        .iter()
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("dg"))
    {
        EditorBuffer::from_file(&first_dg.display().to_string())?
    } else {
        EditorBuffer::empty("main.dg")
    };

    let _guard = TermGuard::enter_raw()?;
    let stdin = io::stdin();
    let mut lock = stdin.lock();

    let mut focus = FocusPane::Editor;
    let mut status_msg = String::from(
        "DOG Studio ready | Ctrl+W Switch Pane | Ctrl+Space Suggest | Ctrl+P Palette | Ctrl+R Run",
    );
    let mut run_output: Option<String> = None;
    let mut output_scroll = 0usize;
    let mut wrap_override: Option<bool> = None;
    let mut manual_suggest = false;
    let mut popup_selected = 0usize;
    let mut suppressed_at: Option<(usize, usize)> = None;

    loop {
        let (rows, cols) = query_terminal_size();
        let body_rows = rows.saturating_sub(3).max(4);
        let show_inspector = cols >= 96;
        let left_w = if cols >= 70 { 24usize } else { 0usize };
        let right_w = if show_inspector { 26usize } else { 0usize };
        let left_sep = if left_w > 0 { 2usize } else { 0usize };
        let right_sep = if right_w > 0 { 3usize } else { 0usize };
        let center_w = cols
            .saturating_sub(left_w + left_sep + right_w + right_sep)
            .max(1);
        let gutter_w = 5usize;
        let code_w = center_w.saturating_sub(gutter_w).max(1);
        let wrap_mode = wrap_override.unwrap_or(code_w < 64);

        buf.ensure_cursor_visible_wrapped(body_rows, code_w, wrap_mode);
        let (diag, symbols) = inspect_buffer(&buf);
        let git_branch = detect_git_status_tag(&base).unwrap_or_else(|| "no-git".to_string());

        let suggestions = buf.ranked_suggestions(manual_suggest);
        let (dot_base, partial) = buf.completion_context();
        let cur_pos = (buf.cursor_row, buf.cursor_col);
        let show_popup = focus == FocusPane::Editor
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
                if let Some(cmd) = prompt_studio_line(
                    &mut lock,
                    rows,
                    cols,
                    "Palette (:<ln>|run|wrap|save|fmt|refresh|comment|dup|snippet fn/if/try|open <file>): ",
                ) {
                    let trimmed = cmd.trim();
                    if let Some(num_str) =
                        trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix("goto "))
                    {
                        if let Ok(line_num) = num_str.trim().parse::<usize>() {
                            let target = line_num
                                .saturating_sub(1)
                                .min(buf.lines.len().saturating_sub(1));
                            buf.cursor_row = target;
                            buf.cursor_col = 0;
                            status_msg = format!("Jumped to line {}.", target + 1);
                        } else {
                            status_msg = "Usage: :<line-number>".to_string();
                        }
                    } else if trimmed == "run" {
                        run_output = Some(execute_studio_buffer(&buf, personality));
                        output_scroll = 0;
                        status_msg = "Executed active buffer.".to_string();
                    } else if trimmed == "wrap" {
                        let next_wrap = !wrap_mode;
                        wrap_override = Some(next_wrap);
                        buf.scroll_col = 0;
                        status_msg = format!(
                            "Soft wrap {}.",
                            if next_wrap { "enabled" } else { "disabled" }
                        );
                    } else if trimmed == "save" {
                        status_msg = save_studio_buffer(&mut lock, rows, cols, &mut buf);
                        files = discover_workspace_files(&base);
                    } else if trimmed == "fmt" || trimmed == "format" {
                        buf.format_in_place();
                        status_msg = "Formatted active buffer.".to_string();
                    } else if trimmed == "comment" {
                        buf.toggle_comment();
                        status_msg = "Toggled comment.".to_string();
                    } else if trimmed == "dup" {
                        buf.duplicate_line();
                        status_msg = "Duplicated line.".to_string();
                    } else if let Some(kind) = trimmed.strip_prefix("snippet ") {
                        if buf.insert_snippet(kind) {
                            status_msg = format!("Inserted '{kind}' snippet.");
                        } else {
                            status_msg = "Unknown snippet. Try fn|struct|enum|if|match|try|for".to_string();
                        }
                    } else if trimmed == "refresh" {
                        files = discover_workspace_files(&base);
                        selected_idx = selected_idx.min(files.len().saturating_sub(1));
                        status_msg = format!("Refreshed workspace ({} files).", files.len());
                    } else if let Some(rest) = trimmed.strip_prefix("open ") {
                        let p = rest.trim();
                        match EditorBuffer::from_file(p) {
                            Ok(new_b) => {
                                buf = new_b;
                                focus = FocusPane::Editor;
                                status_msg = format!("Opened '{p}'.");
                            }
                            Err(e) => status_msg = e,
                        }
                    } else if !trimmed.is_empty() {
                        status_msg = format!("Unknown palette command '{trimmed}'.");
                    }
                }
            }
            KeyEvent::Alt('w') | KeyEvent::Alt('W') => {
                let next_wrap = !wrap_mode;
                wrap_override = Some(next_wrap);
                buf.scroll_col = 0;
                status_msg = format!(
                    "Soft wrap {}.",
                    if next_wrap { "enabled" } else { "disabled" }
                );
            }
            KeyEvent::Ctrl('s') => {
                status_msg = save_studio_buffer(&mut lock, rows, cols, &mut buf);
                files = discover_workspace_files(&base);
            }
            KeyEvent::Ctrl('l') => {
                buf.format_in_place();
                status_msg = "Formatted with 4-space DOG style.".to_string();
            }
            KeyEvent::Ctrl('d') => {
                buf.duplicate_line();
                status_msg = "Duplicated line.".to_string();
            }
            KeyEvent::Ctrl('k') => {
                buf.delete_line();
                status_msg = "Deleted line.".to_string();
            }
            KeyEvent::Ctrl('u') => {
                buf.toggle_comment();
                status_msg = "Toggled comment.".to_string();
            }
            KeyEvent::Ctrl('r') => {
                run_output = Some(execute_studio_buffer(&buf, personality));
                output_scroll = 0;
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
                    KeyEvent::Char('r') => {
                        files = discover_workspace_files(&base);
                        selected_idx = selected_idx.min(files.len().saturating_sub(1));
                        status_msg = format!("Refreshed explorer ({} files).", files.len());
                    }
                    KeyEvent::Char('n') => {
                        if let Some(rel) =
                            prompt_studio_line(&mut lock, rows, cols, "New file (.dg): ")
                        {
                            let trimmed = rel.trim();
                            if !trimmed.is_empty() {
                                let with_ext = if trimmed.ends_with(".dg") {
                                    trimmed.to_string()
                                } else {
                                    format!("{trimmed}.dg")
                                };
                                let target = base.join(&with_ext);
                                if let Some(parent) = target.parent() {
                                    let _ = fs::create_dir_all(parent);
                                }
                                if !target.exists() {
                                    let _ = fs::write(&target, "say(\"Hello from DOG!\")\n");
                                }
                                match EditorBuffer::from_file(&target.display().to_string()) {
                                    Ok(new_buf) => {
                                        buf = new_buf;
                                        files = discover_workspace_files(&base);
                                        if let Some(pos) =
                                            files.iter().position(|p| p == &target)
                                        {
                                            selected_idx = pos;
                                        }
                                        focus = FocusPane::Editor;
                                        status_msg = format!("Created '{}'.", target.display());
                                    }
                                    Err(e) => status_msg = e,
                                }
                            }
                        }
                    }
                    KeyEvent::Enter => {
                        if let Some(target) = files.get(selected_idx) {
                            let path_str = target.display().to_string();
                            match EditorBuffer::from_file(&path_str) {
                                Ok(new_buf) => {
                                    buf = new_buf;
                                    focus = FocusPane::Editor;
                                    manual_suggest = false;
                                    popup_selected = 0;
                                    status_msg = format!("Loaded '{path_str}'.");
                                }
                                Err(e) => status_msg = e,
                            }
                        }
                    }
                    _ => {}
                },
                FocusPane::Editor => match key {
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
                        buf.insert_char_smart(ch);
                        suppressed_at = None;
                        popup_selected = 0;
                        if ch == '.' {
                            manual_suggest = false;
                        }
                    }
                    KeyEvent::Enter => {
                        buf.insert_newline_auto_indent();
                        manual_suggest = false;
                        suppressed_at = None;
                        popup_selected = 0;
                    }
                    KeyEvent::Tab => {
                        if show_popup {
                            if let Some(chosen) = suggestions.get(popup_selected) {
                                buf.accept_suggestion_item(chosen);
                                manual_suggest = false;
                                popup_selected = 0;
                                suppressed_at = None;
                            }
                        } else if !buf.accept_autocomplete() {
                            buf.insert_tab();
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
                        buf.backspace_smart();
                        suppressed_at = None;
                        popup_selected = 0;
                    }
                    KeyEvent::Delete => {
                        buf.delete_forward();
                        suppressed_at = None;
                        popup_selected = 0;
                    }
                    KeyEvent::Left => {
                        buf.move_left();
                        manual_suggest = false;
                        popup_selected = 0;
                    }
                    KeyEvent::Right => {
                        buf.move_right();
                        manual_suggest = false;
                        popup_selected = 0;
                    }
                    KeyEvent::WordLeft => {
                        buf.move_word_left();
                        manual_suggest = false;
                    }
                    KeyEvent::WordRight => {
                        buf.move_word_right();
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
                            buf.move_up();
                            manual_suggest = false;
                            popup_selected = 0;
                        }
                    }
                    KeyEvent::Down => {
                        if show_popup && !suggestions.is_empty() {
                            popup_selected = (popup_selected + 1) % suggestions.len();
                        } else {
                            buf.move_down();
                            manual_suggest = false;
                            popup_selected = 0;
                        }
                    }
                    KeyEvent::PageUp => {
                        buf.page_up(body_rows);
                        manual_suggest = false;
                    }
                    KeyEvent::PageDown => {
                        buf.page_down(body_rows);
                        manual_suggest = false;
                    }
                    KeyEvent::Home => {
                        buf.cursor_col = 0;
                        manual_suggest = false;
                    }
                    KeyEvent::End => {
                        buf.cursor_col = buf.lines[buf.cursor_row].chars().count();
                        manual_suggest = false;
                    }
                    _ => {}
                },
            },
        }
    }

    Ok(())
}

fn save_studio_buffer(
    lock: &mut io::StdinLock<'_>,
    rows: usize,
    cols: usize,
    buf: &mut EditorBuffer,
) -> String {
    let target_path = if let Some(ref p) = buf.path {
        Some(p.clone())
    } else {
        prompt_studio_line(lock, rows, cols, "Save as (.dg): ")
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

fn prompt_studio_line(
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
    output_scroll: usize,
    wrap_mode: bool,
    suggestions: &[SuggestionItem],
    popup_selected: usize,
    rows: usize,
    cols: usize,
) {
    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[H");

    // Width-bounded Row 1 header so phones never wrap row 1
    let badge_plain = match diag {
        None => "[✓ OK]".to_string(),
        Some(d) => format!("[✗ {}]", d.code),
    };
    let badge_ansi = match diag {
        None => "\x1b[38;5;78;1m[✓ OK]\x1b[22;38;5;250m".to_string(),
        Some(d) => format!("\x1b[38;5;203;1m[✗ {}]\x1b[22;38;5;250m", d.code),
    };
    let brand = if cols >= 40 { " DOG STUDIO " } else { " STUDIO " };
    let brand_len = brand.chars().count();
    let dirty_plain = if buf.dirty { "*" } else { "" };
    let git_plain = format!(" │ git:{git_branch}");
    let right_len = 1 + badge_plain.chars().count();
    let title_budget = cols.saturating_sub(brand_len + right_len + 1);
    let raw_title = format!(" {}{}", buf.title, dirty_plain);
    let title_clip: String = raw_title.chars().take(title_budget).collect();
    let used_so_far = brand_len + title_clip.chars().count() + right_len;
    let git_part = if used_so_far + git_plain.chars().count() <= cols {
        format!("\x1b[38;5;239m│\x1b[38;5;117m git:{git_branch}")
    } else {
        String::new()
    };
    out.push_str(&format!(
        "\x1b[48;5;33;38;5;255;1m{brand}\x1b[22;48;5;235;38;5;255m{title_clip} {badge_ansi}{git_part}\x1b[K\x1b[0m\r\n"
    ));

    let body_rows = rows.saturating_sub(3).max(4);
    let show_inspector = cols >= 96;
    let left_w = if cols >= 70 { 24usize } else { 0usize };
    let right_w = if show_inspector { 26usize } else { 0usize };
    let left_sep = if left_w > 0 { 2usize } else { 0usize };
    let right_sep = if right_w > 0 { 3usize } else { 0usize };
    let center_w = cols
        .saturating_sub(left_w + left_sep + right_w + right_sep)
        .max(1);
    let gutter_w = 5usize;
    let code_w = center_w.saturating_sub(gutter_w).max(1);

    let ghost = if focus == FocusPane::Editor && run_output.is_none() {
        suggestions
            .get(popup_selected)
            .and_then(|item| buf.ghost_completion_for(item))
    } else {
        None
    };

    let mut cursor_r = 2usize;
    let left_offset = if left_w > 0 { left_w + 2 } else { 0 };
    let mut cursor_c = (left_offset + gutter_w + 1).min(cols.max(1));

    if let Some(output) = run_output {
        let raw: Vec<&str> = if output.is_empty() {
            vec![""]
        } else {
            output.lines().collect()
        };
        let wrapped = wrap_plain_lines(&raw, cols.saturating_sub(2).max(1));
        let out_hdr: String = "  ▶ Studio Output (↑↓ scroll · Esc/Enter back)"
            .chars()
            .take(cols)
            .collect();
        out.push_str(&format!("\x1b[48;5;235;38;5;78;1m{out_hdr}\x1b[K\x1b[0m\r\n"));
        for r in 1..body_rows {
            let l = wrapped
                .get(output_scroll + r - 1)
                .map(|s| s.as_str())
                .unwrap_or("");
            let clipped: String = l.chars().take(cols.saturating_sub(2)).collect();
            out.push_str(&format!(
                "\x1b[48;5;234;38;5;252m  {clipped}\x1b[K\x1b[0m\r\n"
            ));
        }
    } else {
        let vrows = build_visual_rows(&buf.lines, code_w, wrap_mode);
        let cur_vr = visual_row_for_cursor(&vrows, &buf.lines, buf.cursor_row, buf.cursor_col);
        if let Some(vr) = vrows.get(cur_vr) {
            let vcol = buf
                .lines
                .get(buf.cursor_row)
                .map(|l| logical_to_visual_col(l, buf.cursor_col))
                .unwrap_or(0);
            cursor_r = cur_vr.saturating_sub(buf.scroll_row) + 2;
            let rel_c = if wrap_mode {
                vcol.saturating_sub(vr.vis_start)
            } else {
                vcol.saturating_sub(buf.scroll_col)
            };
            cursor_c = (left_offset + gutter_w + rel_c + 1).min(cols.max(1));
        }

        let file_scroll = selected_idx.saturating_sub(body_rows.saturating_sub(2));
        let div = "\x1b[48;5;234;38;5;239m│\x1b[39m";
        for r in 0..body_rows {
            // Left pane: Project Explorer
            if left_w > 0 {
                let title_style = if focus == FocusPane::Explorer {
                    "\x1b[48;5;235;38;5;81;1m"
                } else {
                    "\x1b[48;5;234;38;5;244m"
                };
                if r == 0 {
                    let hdr = format!(
                        " {:<width$}",
                        "FILES (n:new r:ref)",
                        width = left_w.saturating_sub(1)
                    );
                    out.push_str(&format!("{title_style}{hdr}\x1b[22m{div} "));
                } else {
                    let f_idx = file_scroll + r - 1;
                    if let Some(p) = files.get(f_idx) {
                        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("?");
                        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                        let icon = match ext {
                            "dg" => "◆",
                            "pack" | "lock" => "◈",
                            _ => "·",
                        };
                        let label = format!(" {icon} {name}");
                        let clipped: String = label.chars().take(left_w).collect();
                        let padded = format!("{:<width$}", clipped, width = left_w);
                        if f_idx == selected_idx {
                            out.push_str(&format!("\x1b[48;5;24;38;5;255m{padded}{div} "));
                        } else {
                            out.push_str(&format!("\x1b[48;5;234;38;5;250m{padded}{div} "));
                        }
                    } else {
                        out.push_str(&format!(
                            "\x1b[48;5;234m{:<width$}{div} ",
                            "",
                            width = left_w
                        ));
                    }
                }
            }

            // Center pane: Soft-Wrapped Code Editor
            let vr_idx = buf.scroll_row + r;
            if let Some(vr) = vrows.get(vr_idx) {
                let file_r = vr.file_row;
                let line = &buf.lines[file_r];
                let ln = file_r + 1;
                let is_active = file_r == buf.cursor_row;
                let is_err = diag.is_some_and(|d| d.span.line == ln);
                let row_bg = if is_active {
                    "\x1b[48;5;236m"
                } else {
                    "\x1b[48;5;234m"
                };
                if vr.is_cont {
                    out.push_str(&format!("{row_bg}\x1b[38;5;240m  ↳  \x1b[38;5;252m"));
                } else {
                    let ln_str = format_ln_fixed(ln, 3);
                    if is_err {
                        out.push_str(&format!("{row_bg}\x1b[38;5;203;1m{ln_str}✗\x1b[22;38;5;252m "));
                    } else if is_active {
                        out.push_str(&format!("{row_bg}\x1b[38;5;221;1m{ln_str} \x1b[22;38;5;252m "));
                    } else {
                        out.push_str(&format!("{row_bg}\x1b[38;5;242m{ln_str} \x1b[38;5;252m "));
                    }
                }

                let expanded = visual_expand(line);
                let visible: String = if wrap_mode {
                    expanded
                        .chars()
                        .skip(vr.vis_start)
                        .take(vr.vis_end.saturating_sub(vr.vis_start).min(code_w))
                        .collect()
                } else {
                    expanded.chars().skip(buf.scroll_col).take(code_w).collect()
                };
                let mut vis_len = visible.chars().count();
                out.push_str(&highlight_dog_line(&visible));

                let is_last_seg = vrows
                    .get(vr_idx + 1)
                    .map(|n| n.file_row != file_r)
                    .unwrap_or(true);
                if is_active && is_last_seg && buf.cursor_col == line.chars().count() {
                    if let Some(ref g) = ghost {
                        let avail = code_w.saturating_sub(vis_len);
                        if avail > 0 {
                            let clipped_g: String = g.chars().take(avail).collect();
                            vis_len += clipped_g.chars().count();
                            out.push_str(&format!("\x1b[38;5;242m{clipped_g}\x1b[38;5;252m"));
                        }
                    }
                }

                if right_w > 0 {
                    let pad_spaces = code_w.saturating_sub(vis_len);
                    out.push_str(&" ".repeat(pad_spaces));
                }
            } else if right_w > 0 {
                let empty_code = format!("{:<width$}", "", width = code_w);
                out.push_str(&format!("\x1b[48;5;234;38;5;238m   ~ {empty_code}\x1b[39m"));
            } else {
                let empty_gutter: String = "   ~ ".chars().take(cols).collect();
                out.push_str(&format!("\x1b[48;5;234;38;5;238m{empty_gutter}\x1b[39m"));
            }

            // Right pane: Live Symbols
            if right_w > 0 {
                out.push_str(&format!(" {div} "));
                if r == 0 {
                    out.push_str(&format!(
                        "\x1b[48;5;234;38;5;221;1m{:<width$}\x1b[22;39m",
                        "SYMBOLS",
                        width = right_w
                    ));
                } else if let Some((sym, ty)) = symbols.get(r - 1) {
                    let entry = format!("{sym}: {ty}");
                    let clipped: String = entry.chars().take(right_w).collect();
                    out.push_str(&format!(
                        "\x1b[48;5;234;38;5;152m{:<width$}\x1b[39m",
                        clipped,
                        width = right_w
                    ));
                } else {
                    out.push_str(&format!(
                        "\x1b[48;5;234m{:<width$}",
                        "",
                        width = right_w
                    ));
                }
            }

            out.push_str("\x1b[K\x1b[0m\r\n");
        }
    }

    if let Some(d) = diag {
        let msg = format!(
            " ✗ {} ({}:{}): {}  →  {}",
            d.code, d.span.line, d.span.column, d.title, d.fix
        );
        let clipped: String = msg.chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;52;38;5;224m{clipped}\x1b[K\x1b[0m\r\n"));
    } else if let Some(chosen) = suggestions.get(popup_selected) {
        let hint = format!(" ▸ [{}] {}  —  {}", chosen.kind, chosen.label, chosen.detail);
        let clipped: String = hint.chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;24;38;5;159m{clipped}\x1b[K\x1b[0m\r\n"));
    } else if let Some(sig_hint) = buf.contextual_hint() {
        let clipped: String = format!(" ⓘ {sig_hint}").chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;235;38;5;117m{clipped}\x1b[K\x1b[0m\r\n"));
    } else {
        let clipped: String = format!(" ▸ {status_msg}").chars().take(cols).collect();
        out.push_str(&format!("\x1b[48;5;235;38;5;110m{clipped}\x1b[K\x1b[0m\r\n"));
    }

    let footer = if cols < 58 {
        format!(
            " L{}:{} │ {}L │ ^W ^R Run ^S Save ^Q Quit",
            buf.cursor_row + 1,
            buf.cursor_col + 1,
            buf.lines.len()
        )
    } else {
        format!(
            " Ln {}, Col {} │ {}L │ {} │ ^W Pane  ^Space Suggest  ^P Palette  Alt+W Wrap  ^R Run  ^S Save  ^Q Quit",
            buf.cursor_row + 1,
            buf.cursor_col + 1,
            buf.lines.len(),
            if wrap_mode { "WRAP" } else { "NOWRAP" }
        )
    };
    let clipped_footer: String = footer.chars().take(cols).collect();
    out.push_str(&format!("\x1b[48;5;235;38;5;250m{clipped_footer}\x1b[K\x1b[0m"));

    if !suggestions.is_empty() && focus == FocusPane::Editor && run_output.is_none() {
        render_suggestion_popup_overlay(
            &mut out,
            suggestions,
            popup_selected,
            cursor_r,
            cursor_c,
            1 + body_rows,
            rows,
            cols,
        );
    }

    let clamped_r = cursor_r.clamp(2, (1 + body_rows).max(2));
    let clamped_c = cursor_c.clamp(1, cols.max(1));
    out.push_str(&format!("\x1b[{clamped_r};{clamped_c}H\x1b[?25h"));

    print!("{out}");
    let _ = io::stdout().flush();
}
