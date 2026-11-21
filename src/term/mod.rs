use std::cell::RefCell;
use std::env;
use std::io::{self, IsTerminal, Write};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermTheme {
    Default,
    Kali,
    Dracula,
    Solar,
    Mono,
}

impl TermTheme {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "default" | "dog" | "cobalt" => Some(Self::Default),
            "kali" | "cyber" => Some(Self::Kali),
            "dracula" | "violet" => Some(Self::Dracula),
            "solar" | "solarized" => Some(Self::Solar),
            "mono" | "plain" | "none" => Some(Self::Mono),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Kali => "kali",
            Self::Dracula => "dracula",
            Self::Solar => "solar",
            Self::Mono => "mono",
        }
    }

    pub fn available() -> &'static [&'static str] {
        &["default", "kali", "dracula", "solar", "mono"]
    }

    pub fn uses_color(&self) -> bool {
        !matches!(self, Self::Mono)
    }

    pub fn palette(&self) -> ThemePalette {
        match self {
            Self::Default => ThemePalette {
                info: "\x1b[38;5;39m",
                success: "\x1b[38;5;78m",
                warn: "\x1b[38;5;214m",
                error: "\x1b[38;5;203m",
                debug: "\x1b[38;5;141m",
                progress: "\x1b[38;5;45m",
                accent: "\x1b[38;5;33;1m",
                muted: "\x1b[38;5;244m",
                border: "\x1b[38;5;240m",
                bold: "\x1b[1m",
                reset: "\x1b[0m",
            },
            Self::Kali => ThemePalette {
                info: "\x1b[1;34m",
                success: "\x1b[1;32m",
                warn: "\x1b[1;33m",
                error: "\x1b[1;31m",
                debug: "\x1b[1;35m",
                progress: "\x1b[1;36m",
                accent: "\x1b[1;31m",
                muted: "\x1b[38;5;246m",
                border: "\x1b[1;34m",
                bold: "\x1b[1m",
                reset: "\x1b[0m",
            },
            Self::Dracula => ThemePalette {
                info: "\x1b[38;5;117m",
                success: "\x1b[38;5;84m",
                warn: "\x1b[38;5;215m",
                error: "\x1b[38;5;204m",
                debug: "\x1b[38;5;141m",
                progress: "\x1b[38;5;212m",
                accent: "\x1b[38;5;141;1m",
                muted: "\x1b[38;5;103m",
                border: "\x1b[38;5;61m",
                bold: "\x1b[1m",
                reset: "\x1b[0m",
            },
            Self::Solar => ThemePalette {
                info: "\x1b[38;5;37m",
                success: "\x1b[38;5;64m",
                warn: "\x1b[38;5;136m",
                error: "\x1b[38;5;160m",
                debug: "\x1b[38;5;61m",
                progress: "\x1b[38;5;33m",
                accent: "\x1b[38;5;166;1m",
                muted: "\x1b[38;5;245m",
                border: "\x1b[38;5;242m",
                bold: "\x1b[1m",
                reset: "\x1b[0m",
            },
            Self::Mono => ThemePalette {
                info: "",
                success: "",
                warn: "",
                error: "",
                debug: "",
                progress: "",
                accent: "",
                muted: "",
                border: "",
                bold: "",
                reset: "",
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub info: &'static str,
    pub success: &'static str,
    pub warn: &'static str,
    pub error: &'static str,
    pub debug: &'static str,
    pub progress: &'static str,
    pub accent: &'static str,
    pub muted: &'static str,
    pub border: &'static str,
    pub bold: &'static str,
    pub reset: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStyle {
    Detailed,
    Compact,
    Minimal,
}

impl InstallStyle {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "detailed" | "full" => Some(Self::Detailed),
            "compact" | "brief" => Some(Self::Compact),
            "minimal" | "quiet" => Some(Self::Minimal),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Detailed => "detailed",
            Self::Compact => "compact",
            Self::Minimal => "minimal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermCapabilities {
    pub is_tty: bool,
    pub stderr_tty: bool,
    pub color: bool,
    pub unicode: bool,
    pub width: usize,
    pub is_termux: bool,
}

impl TermCapabilities {
    pub fn detect() -> Self {
        let is_tty = io::stdout().is_terminal();
        let stderr_tty = io::stderr().is_terminal();
        Self::detect_with(is_tty, stderr_tty, |k| env::var(k).ok())
    }

    pub fn detect_with<F>(is_tty: bool, stderr_tty: bool, mut env_get: F) -> Self
    where
        F: FnMut(&str) -> Option<String>,
    {
        let term = env_get("TERM").unwrap_or_default();
        let is_dumb = term.eq_ignore_ascii_case("dumb");
        let is_termux = env_get("TERMUX_VERSION").is_some()
            || env_get("PREFIX").is_some_and(|p| p.contains("com.termux"));

        let no_color = env_get("NO_COLOR").is_some_and(|v| !v.is_empty());
        let force_color = env_get("CLICOLOR_FORCE")
            .or_else(|| env_get("FORCE_COLOR"))
            .is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes"));
        let clicolor_zero = env_get("CLICOLOR").is_some_and(|v| v.trim() == "0");

        let color = if no_color || is_dumb || clicolor_zero {
            false
        } else if force_color {
            true
        } else {
            is_tty
        };

        let locale = env_get("LC_ALL")
            .or_else(|| env_get("LC_CTYPE"))
            .or_else(|| env_get("LANG"))
            .unwrap_or_default();
        let locale_lower = locale.to_ascii_lowercase();
        let force_ascii = env_get("DOG_ASCII").is_some_and(|v| v.trim() == "1");
        let unicode = if is_dumb || force_ascii {
            false
        } else if locale_lower.contains("utf-8") || locale_lower.contains("utf8") {
            true
        } else if locale_lower == "c" || locale_lower == "posix" {
            false
        } else {
            is_termux || env_get("WT_SESSION").is_some() || is_tty
        };

        let width = env_get("COLUMNS")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .map(|w| w.clamp(40, 240))
            .unwrap_or(80);

        Self {
            is_tty,
            stderr_tty,
            color,
            unicode,
            width,
            is_termux,
        }
    }

    pub fn plain() -> Self {
        Self {
            is_tty: false,
            stderr_tty: false,
            color: false,
            unicode: false,
            width: 80,
            is_termux: false,
        }
    }

    pub fn rich() -> Self {
        Self {
            is_tty: true,
            stderr_tty: true,
            color: true,
            unicode: true,
            width: 80,
            is_termux: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermStyleConfig {
    pub theme: TermTheme,
    pub timestamps: bool,
    pub prefix: String,
    pub debug: bool,
    pub install_style: InstallStyle,
    pub color_override: Option<bool>,
    pub unicode_override: Option<bool>,
}

impl Default for TermStyleConfig {
    fn default() -> Self {
        Self {
            theme: TermTheme::Default,
            timestamps: false,
            prefix: "dog".to_string(),
            debug: true,
            install_style: InstallStyle::Detailed,
            color_override: None,
            unicode_override: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgLevel {
    Info,
    Success,
    Warning,
    Error,
    Debug,
    Progress,
}

impl MsgLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "OK",
            Self::Warning => "WARN",
            Self::Error => "ERR",
            Self::Debug => "DEBUG",
            Self::Progress => "STEP",
        }
    }

    pub fn icon(&self, unicode: bool) -> &'static str {
        if unicode {
            match self {
                Self::Info => "ℹ",
                Self::Success => "✔",
                Self::Warning => "▲",
                Self::Error => "✖",
                Self::Debug => "◆",
                Self::Progress => "▸",
            }
        } else {
            match self {
                Self::Info => "i",
                Self::Success => "+",
                Self::Warning => "!",
                Self::Error => "x",
                Self::Debug => "?",
                Self::Progress => ">",
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DogMessenger {
    pub style: TermStyleConfig,
    pub caps: TermCapabilities,
}

impl DogMessenger {
    pub fn new(style: TermStyleConfig, caps: TermCapabilities) -> Self {
        Self { style, caps }
    }

    pub fn detect(style: TermStyleConfig) -> Self {
        Self {
            style,
            caps: TermCapabilities::detect(),
        }
    }

    pub fn use_color(&self) -> bool {
        if !self.style.theme.uses_color() {
            return false;
        }
        self.style.color_override.unwrap_or(self.caps.color)
    }

    pub fn use_unicode(&self) -> bool {
        self.style.unicode_override.unwrap_or(self.caps.unicode)
    }

    fn palette(&self) -> ThemePalette {
        if self.use_color() {
            self.style.theme.palette()
        } else {
            TermTheme::Mono.palette()
        }
    }

    fn level_color(&self, level: MsgLevel) -> &'static str {
        let p = self.palette();
        match level {
            MsgLevel::Info => p.info,
            MsgLevel::Success => p.success,
            MsgLevel::Warning => p.warn,
            MsgLevel::Error => p.error,
            MsgLevel::Debug => p.debug,
            MsgLevel::Progress => p.progress,
        }
    }

    pub fn render_line(
        &self,
        level: MsgLevel,
        msg: &str,
        unix_secs_override: Option<u64>,
    ) -> String {
        let p = self.palette();
        let unicode = self.use_unicode();
        let icon = level.icon(unicode);
        let tag = level.label();
        let lcol = self.level_color(level);

        let mut out = String::new();
        if self.style.timestamps {
            let secs = unix_secs_override.unwrap_or_else(current_unix_secs);
            let hms = format_utc_hms(secs);
            if self.use_color() {
                out.push_str(&format!("{}[{}]{} ", p.muted, hms, p.reset));
            } else {
                out.push_str(&format!("[{hms}] "));
            }
        }
        if !self.style.prefix.trim().is_empty() {
            let pref = self.style.prefix.trim();
            if self.use_color() {
                out.push_str(&format!("{}[{}]{} ", p.accent, pref, p.reset));
            } else {
                out.push_str(&format!("[{pref}] "));
            }
        }
        if self.use_color() {
            out.push_str(&format!(
                "{}{} {:<5}{} {}",
                lcol, icon, tag, p.reset, msg
            ));
        } else {
            out.push_str(&format!("[{icon} {tag}] {msg}"));
        }
        out
    }

    pub fn render_stage(&self, step: usize, total: usize, title: &str, detail: &str) -> String {
        let p = self.palette();
        let unicode = self.use_unicode();
        let arrow = if unicode { "▸" } else { ">" };
        let mut head = if self.style.prefix.trim().is_empty() {
            String::new()
        } else if self.use_color() {
            format!("{}[{}]{} ", p.accent, self.style.prefix.trim(), p.reset)
        } else {
            format!("[{}] ", self.style.prefix.trim())
        };
        if self.style.timestamps {
            let hms = format_utc_hms(current_unix_secs());
            if self.use_color() {
                head = format!("{}[{}]{} {head}", p.muted, hms, p.reset);
            } else {
                head = format!("[{hms}] {head}");
            }
        }

        if detail.is_empty() {
            if self.use_color() {
                format!(
                    "{head}{}[{step}/{total}]{} {}{}{}",
                    p.progress, p.reset, p.bold, title, p.reset
                )
            } else {
                format!("{head}[{step}/{total}] {title}")
            }
        } else if self.use_color() {
            format!(
                "{head}{}[{step}/{total}]{} {}{}{} {} {}{}{}",
                p.progress, p.reset, p.bold, title, p.reset, arrow, p.muted, detail, p.reset
            )
        } else {
            format!("{head}[{step}/{total}] {title} {arrow} {detail}")
        }
    }

    pub fn render_box(&self, title: &str, lines: &[String]) -> String {
        let p = self.palette();
        let unicode = self.use_unicode();
        let mut all_widths: Vec<usize> = vec![title.chars().count() + 4];
        for line in lines {
            all_widths.push(line.chars().count());
        }
        let max_inner = all_widths
            .into_iter()
            .max()
            .unwrap_or(24)
            .clamp(24, self.caps.width.saturating_sub(6).max(24));

        let (tl, tr, bl, br, hz, vt) = if unicode {
            ("╭", "╮", "╰", "╯", "─", "│")
        } else {
            ("+", "+", "+", "+", "-", "|")
        };

        let mut out = String::new();
        let title_bar = format!(" {title} ");
        let fill = max_inner.saturating_sub(title_bar.chars().count());
        if self.use_color() {
            out.push_str(&format!(
                "{}{tl}{hz}{}{}{}{}{tr}{}\n",
                p.border,
                p.accent,
                title_bar,
                p.border,
                hz.repeat(fill),
                p.reset
            ));
        } else {
            out.push_str(&format!("{tl}{hz}{title_bar}{}{tr}\n", hz.repeat(fill)));
        }

        for line in lines {
            let trimmed: String = line.chars().take(max_inner).collect();
            let pad = max_inner.saturating_sub(trimmed.chars().count());
            if self.use_color() {
                out.push_str(&format!(
                    "{}{vt}{} {trimmed}{} {}{vt}{}\n",
                    p.border,
                    p.reset,
                    " ".repeat(pad),
                    p.border,
                    p.reset
                ));
            } else {
                out.push_str(&format!("{vt} {trimmed}{} {vt}\n", " ".repeat(pad)));
            }
        }

        let bottom = hz.repeat(max_inner + 2);
        if self.use_color() {
            out.push_str(&format!("{}{bl}{bottom}{br}{}", p.border, p.reset));
        } else {
            out.push_str(&format!("{bl}{bottom}{br}"));
        }
        out
    }

    pub fn render_summary(&self, title: &str, pairs: &[(&str, String)]) -> String {
        let mut rows = Vec::with_capacity(pairs.len());
        let label_w = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(8);
        for (k, v) in pairs {
            rows.push(format!("{k:<label_w$} : {v}"));
        }
        self.render_box(title, &rows)
    }

    pub fn info(&self, msg: &str) {
        println!("{}", self.render_line(MsgLevel::Info, msg, None));
    }

    pub fn success(&self, msg: &str) {
        println!("{}", self.render_line(MsgLevel::Success, msg, None));
    }

    pub fn warn(&self, msg: &str) {
        eprintln!("{}", self.render_line(MsgLevel::Warning, msg, None));
    }

    pub fn error(&self, msg: &str) {
        eprintln!("{}", self.render_line(MsgLevel::Error, msg, None));
    }

    pub fn debug(&self, msg: &str) {
        if self.style.debug {
            println!("{}", self.render_line(MsgLevel::Debug, msg, None));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressUnit {
    Items,
    Bytes,
}

#[derive(Debug, Clone)]
pub struct ProgressBar {
    pub label: String,
    pub current: u64,
    pub total: u64,
    pub unit: ProgressUnit,
    pub width: usize,
    pub messenger: DogMessenger,
    last_printed_pct: Option<u64>,
}

impl ProgressBar {
    pub fn new(label: impl Into<String>, total: u64, unit: ProgressUnit, messenger: DogMessenger) -> Self {
        Self {
            label: label.into(),
            current: 0,
            total,
            unit,
            width: 22,
            messenger,
            last_printed_pct: None,
        }
    }

    pub fn percent(&self) -> u64 {
        if self.total == 0 {
            100
        } else {
            let clamped = self.current.min(self.total) as u128;
            ((clamped * 100) / self.total as u128) as u64
        }
    }

    pub fn set(&mut self, current: u64) {
        self.current = current;
    }

    pub fn render(&self) -> String {
        let pct = self.percent();
        let filled = ((pct as usize * self.width) / 100).min(self.width);
        let empty = self.width.saturating_sub(filled);
        let unicode = self.messenger.use_unicode();
        let use_color = self.messenger.use_color();
        let p = self.messenger.palette();

        let (fill_ch, empty_ch) = if unicode { ("█", "░") } else { ("#", "-") };
        let bar = format!("{}{}", fill_ch.repeat(filled), empty_ch.repeat(empty));

        let counts = match self.unit {
            ProgressUnit::Items => format!("{}/{}", self.current, self.total),
            ProgressUnit::Bytes => {
                if self.total > 0 {
                    format!("{} / {}", format_bytes(self.current), format_bytes(self.total))
                } else {
                    format_bytes(self.current)
                }
            }
        };

        let body = if use_color {
            format!(
                "{} [{}{}{}] {:>3}% ({})",
                self.label, p.progress, bar, p.reset, pct, counts
            )
        } else {
            format!("{} [{}] {:>3}% ({})", self.label, bar, pct, counts)
        };
        self.messenger.render_line(MsgLevel::Progress, &body, None)
    }

    pub fn update_stdout(&mut self, current: u64) {
        self.current = current;
        let pct = self.percent();
        let live_tty = self.messenger.caps.is_tty && self.messenger.use_color();
        if live_tty {
            print!("\r\x1b[2K{}", self.render());
            let _ = io::stdout().flush();
            self.last_printed_pct = Some(pct);
        } else {
            let should_print = match self.last_printed_pct {
                None => true,
                Some(prev) => pct == 100 || pct.saturating_sub(prev) >= 25,
            };
            if should_print && self.last_printed_pct != Some(pct) {
                println!("{}", self.render());
                self.last_printed_pct = Some(pct);
            }
        }
    }

    pub fn finish_stdout(&mut self, done_msg: Option<&str>) {
        if self.total > 0 {
            self.current = self.total;
        }
        let live_tty = self.messenger.caps.is_tty && self.messenger.use_color();
        if live_tty {
            print!("\r\x1b[2K{}", self.render());
            println!();
        } else if self.last_printed_pct != Some(self.percent()) {
            println!("{}", self.render());
        }
        if let Some(msg) = done_msg {
            self.messenger.success(msg);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Spinner {
    pub label: String,
    pub step: usize,
    pub messenger: DogMessenger,
}

impl Spinner {
    pub fn new(label: impl Into<String>, messenger: DogMessenger) -> Self {
        Self {
            label: label.into(),
            step: 0,
            messenger,
        }
    }

    pub fn glyph(step: usize, unicode: bool) -> &'static str {
        const UNI: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        const ASC: &[&str] = &["|", "/", "-", "\\"];
        if unicode {
            UNI[step % UNI.len()]
        } else {
            ASC[step % ASC.len()]
        }
    }

    pub fn render_step(&self, step: usize, detail: &str) -> String {
        let unicode = self.messenger.use_unicode();
        let glyph = Self::glyph(step, unicode);
        let msg = if detail.is_empty() {
            format!("{glyph} {}", self.label)
        } else {
            format!("{glyph} {} — {detail}", self.label)
        };
        self.messenger.render_line(MsgLevel::Progress, &msg, None)
    }

    pub fn tick_stdout(&mut self, detail: &str) {
        let rendered = self.render_step(self.step, detail);
        self.step = self.step.wrapping_add(1);
        if self.messenger.caps.is_tty && self.messenger.use_color() {
            print!("\r\x1b[2K{rendered}");
            let _ = io::stdout().flush();
        } else {
            println!("{rendered}");
        }
    }

    pub fn finish_ok_stdout(&self, msg: &str) {
        if self.messenger.caps.is_tty && self.messenger.use_color() {
            print!("\r\x1b[2K");
        }
        self.messenger.success(msg);
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn format_utc_hms(secs: u64) -> String {
    let rem = secs % 86_400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub fn format_utc_datetime(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{year:04}-{m:02}-{d:02} {}", format_utc_hms(secs))
}

thread_local! {
    static SESSION_STYLE: RefCell<TermStyleConfig> = RefCell::new(TermStyleConfig::default());
}

pub fn reset_session_style_to(style: TermStyleConfig) {
    SESSION_STYLE.with(|cell| {
        *cell.borrow_mut() = style;
    });
}

pub fn session_messenger() -> DogMessenger {
    let style = SESSION_STYLE.with(|cell| cell.borrow().clone());
    DogMessenger::detect(style)
}

pub fn set_session_theme(name: &str) -> Result<TermTheme, String> {
    let Some(theme) = TermTheme::parse(name) else {
        return Err(format!(
            "Unknown theme '{name}'. Available themes: {}.",
            TermTheme::available().join(", ")
        ));
    };
    SESSION_STYLE.with(|cell| {
        cell.borrow_mut().theme = theme;
    });
    Ok(theme)
}

pub fn set_session_prefix(prefix: &str) {
    SESSION_STYLE.with(|cell| {
        cell.borrow_mut().prefix = prefix.to_string();
    });
}

pub fn set_session_timestamps(enabled: bool) {
    SESSION_STYLE.with(|cell| {
        cell.borrow_mut().timestamps = enabled;
    });
}
