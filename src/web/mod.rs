use crate::ast::{
    BinOp, BindingKind, Expr, InterpPart, Program, Stmt, StyleRuleDecl, StyleVariantBlock, UnaryOp,
};
use crate::stdlib::InterpreterIo;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledStyleRule {
    pub media: Option<String>,
    pub selector: String,
    pub declarations: Vec<(String, String)>,
}

#[derive(Debug, Default, Clone)]
pub struct HeadlessWebState {
    pub route: String,
    pub title: String,
    pub mounted_html: BTreeMap<String, String>,
    pub text_nodes: HashMap<String, String>,
    pub html_nodes: HashMap<String, String>,
    pub values: HashMap<String, String>,
    pub classes: HashMap<String, Vec<String>>,
    pub styles: HashMap<String, BTreeMap<String, String>>,
    pub listeners: Vec<(String, String, String)>,
    pub storage: HashMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct WebRuntimeState {
    pub css_vars: Vec<(String, String)>,
    pub rules: Vec<CompiledStyleRule>,
    pub last_title: String,
    pub last_body_html: String,
    pub last_page_html: String,
}

thread_local! {
    static WEB_STATE: RefCell<WebRuntimeState> = RefCell::new(WebRuntimeState::default());
}

pub fn reset_web_runtime() {
    WEB_STATE.with(|cell| {
        *cell.borrow_mut() = WebRuntimeState::default();
    });
}

pub fn snapshot_web_state() -> WebRuntimeState {
    WEB_STATE.with(|cell| cell.borrow().clone())
}

struct SilentWebIo;

impl InterpreterIo for SilentWebIo {
    fn write_stdout(&mut self, _text: &str) {}
    fn writeln_stdout(&mut self, _text: &str) {}
    fn read_line(&mut self) -> Result<String, String> {
        Ok(String::new())
    }
}

fn expand_var_refs(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && (chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_') {
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '-')
            {
                j += 1;
            }
            let name: String = chars[i + 1..j].iter().collect();
            out.push_str(&format!("var(--{name})"));
            i = j;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

pub fn expand_css_property_and_value(raw_key: &str, raw_val: &str) -> Vec<(String, String)> {
    let prop = raw_key.trim().to_ascii_lowercase();
    let val = expand_var_refs(raw_val.trim());
    match prop.as_str() {
        "bg" => vec![("background".to_string(), val)],
        "ink" | "fg" | "text" => vec![("color".to_string(), val)],
        "pad" | "p" => vec![("padding".to_string(), val)],
        "px" | "pad-x" => vec![("padding-inline".to_string(), val)],
        "py" | "pad-y" => vec![("padding-block".to_string(), val)],
        "m" => vec![("margin".to_string(), val)],
        "mx" | "margin-x" => vec![("margin-inline".to_string(), val)],
        "my" | "margin-y" => vec![("margin-block".to_string(), val)],
        "w" => vec![("width".to_string(), val)],
        "h" => vec![("height".to_string(), val)],
        "max-w" => vec![("max-width".to_string(), val)],
        "min-w" => vec![("min-width".to_string(), val)],
        "max-h" => vec![("max-height".to_string(), val)],
        "min-h" => vec![("min-height".to_string(), val)],
        "radius" | "round" => vec![("border-radius".to_string(), val)],
        "size" => vec![("font-size".to_string(), val)],
        "weight" => vec![("font-weight".to_string(), val)],
        "font" => vec![("font-family".to_string(), val)],
        "leading" => vec![("line-height".to_string(), val)],
        "ease" => vec![("transition".to_string(), val)],
        "align" => vec![("align-items".to_string(), val)],
        "justify" => vec![("justify-content".to_string(), val)],
        "rows" => vec![("grid-template-rows".to_string(), val)],
        "cols" => {
            let tmpl = if !val.is_empty() && val.chars().all(|c| c.is_ascii_digit()) {
                format!("repeat({val}, minmax(0, 1fr))")
            } else {
                val
            };
            vec![
                ("display".to_string(), "grid".to_string()),
                ("grid-template-columns".to_string(), tmpl),
            ]
        }
        "grid" => vec![
            ("display".to_string(), "grid".to_string()),
            ("grid-template-columns".to_string(), val),
        ],
        "flex" => {
            let lower = val.to_ascii_lowercase();
            if lower == "row" {
                vec![
                    ("display".to_string(), "flex".to_string()),
                    ("flex-direction".to_string(), "row".to_string()),
                ]
            } else if lower == "col" || lower == "column" {
                vec![
                    ("display".to_string(), "flex".to_string()),
                    ("flex-direction".to_string(), "column".to_string()),
                ]
            } else {
                vec![("flex".to_string(), val)]
            }
        }
        "center" => {
            let lower = val.to_ascii_lowercase();
            if matches!(lower.as_str(), "true" | "all" | "yes") {
                vec![
                    ("display".to_string(), "flex".to_string()),
                    ("align-items".to_string(), "center".to_string()),
                    ("justify-content".to_string(), "center".to_string()),
                ]
            } else {
                vec![("text-align".to_string(), "center".to_string())]
            }
        }
        "shadow" => {
            let expanded = match val.to_ascii_lowercase().as_str() {
                "sm" => "0 1px 3px rgba(24, 26, 31, 0.08)".to_string(),
                "md" => "0 6px 20px rgba(24, 26, 31, 0.08)".to_string(),
                "lg" => "0 16px 36px rgba(24, 26, 31, 0.12)".to_string(),
                _ => val,
            };
            vec![("box-shadow".to_string(), expanded)]
        }
        _ => vec![(prop, val)],
    }
}

pub fn normalize_selector(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == "theme" || trimmed == "root" {
        ":root".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn register_style_var(name: &str, value: &str) {
    let clean_name = name.trim();
    let var_name = if clean_name.starts_with("--") {
        clean_name.to_string()
    } else {
        format!("--{}", clean_name.trim_start_matches('-'))
    };
    let expanded = expand_var_refs(value.trim());
    WEB_STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        if let Some((_, slot)) = st.css_vars.iter_mut().find(|(k, _)| k == &var_name) {
            *slot = expanded;
        } else {
            st.css_vars.push((var_name, expanded));
        }
    });
}

pub fn register_style_rule(media: Option<String>, selector: &str, props: Vec<(String, String)>) {
    let norm_sel = normalize_selector(selector);
    let mut decls = Vec::with_capacity(props.len());
    for (k, v) in props {
        if norm_sel == ":root" {
            let key_clean = k.trim();
            if key_clean.starts_with("--") {
                decls.push((key_clean.to_string(), expand_var_refs(v.trim())));
            } else {
                register_style_var(key_clean, v.trim());
            }
        } else {
            for (ek, ev) in expand_css_property_and_value(&k, &v) {
                decls.push((ek, ev));
            }
        }
    }
    if decls.is_empty() {
        return;
    }
    WEB_STATE.with(|cell| {
        cell.borrow_mut().rules.push(CompiledStyleRule {
            media,
            selector: norm_sel,
            declarations: decls,
        });
    });
}

pub fn build_compiled_css() -> String {
    WEB_STATE.with(|cell| render_stylesheet(&cell.borrow()))
}

pub fn render_stylesheet(state: &WebRuntimeState) -> String {
    let mut out = String::new();
    if !state.css_vars.is_empty() {
        out.push_str(":root {\n");
        for (k, v) in &state.css_vars {
            out.push_str(&format!("  {k}: {v};\n"));
        }
        out.push_str("}\n");
    }

    let mut media_groups: BTreeMap<String, Vec<&CompiledStyleRule>> = BTreeMap::new();
    for rule in &state.rules {
        if let Some(ref q) = rule.media {
            media_groups.entry(q.clone()).or_default().push(rule);
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{} {{\n", rule.selector));
        for (prop, val) in &rule.declarations {
            out.push_str(&format!("  {prop}: {val};\n"));
        }
        out.push_str("}\n");
    }

    for (query, rules) in media_groups {
        if !out.is_empty() {
            out.push('\n');
        }
        let media_head = expand_media_query(&query);
        out.push_str(&format!("@media {media_head} {{\n"));
        for rule in rules {
            out.push_str(&format!("  {} {{\n", rule.selector));
            for (prop, val) in &rule.declarations {
                out.push_str(&format!("    {prop}: {val};\n"));
            }
            out.push_str("  }\n");
        }
        out.push_str("}\n");
    }

    out
}

fn expand_media_query(raw: &str) -> String {
    let q = raw.trim();
    match q.to_ascii_lowercase().as_str() {
        "mobile" => "(max-width: 768px)".to_string(),
        "tablet" => "(max-width: 1024px)".to_string(),
        "desktop" => "(min-width: 1025px)".to_string(),
        "dark" => "(prefers-color-scheme: dark)".to_string(),
        _ => {
            if q.starts_with('(') {
                q.to_string()
            } else {
                format!("({q})")
            }
        }
    }
}

pub fn render_css_vars(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return String::new();
    }
    let mut lines = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        let key = k.trim().trim_start_matches('-');
        lines.push(format!("  --{key}: {};", expand_var_refs(v.trim())));
    }
    format!(":root {{\n{}\n}}", lines.join("\n"))
}

pub fn render_css_rule(selector: &str, pairs: &[(String, String)]) -> String {
    let sel = normalize_selector(selector);
    let mut lines = Vec::new();
    for (k, v) in pairs {
        for (ek, ev) in expand_css_property_and_value(k, v) {
            lines.push(format!("  {ek}: {ev};"));
        }
    }
    if lines.is_empty() {
        format!("{sel} {{}}")
    } else {
        format!("{sel} {{\n{}\n}}", lines.join("\n"))
    }
}

pub fn render_css_media(query: &str, inner_css: &str) -> String {
    let head = expand_media_query(query);
    let indented = inner_css
        .lines()
        .map(|l| if l.trim().is_empty() { String::new() } else { format!("  {l}") })
        .collect::<Vec<_>>()
        .join("\n");
    format!("@media {head} {{\n{indented}\n}}")
}

pub fn render_css_keyframes(name: &str, pairs: &[(String, String)]) -> String {
    let clean_name = name.trim();
    let stops = pairs
        .iter()
        .map(|(stop, decls)| format!("  {} {{ {} }}", stop.trim(), decls.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    format!("@keyframes {clean_name} {{\n{stops}\n}}")
}

pub fn render_css_theme(bg: &str, surface: &str, ink: &str, accent: &str, font: &str) -> String {
    format!(
        ":root {{\n  --bg: {bg};\n  --surface: {surface};\n  --ink: {ink};\n  --accent: {accent};\n  --font: {font};\n}}\n* {{ box-sizing: border-box; }}\nbody {{ margin: 0; background: var(--bg); color: var(--ink); font-family: var(--font); line-height: 1.6; }}\n.dg-nav {{ display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 14px 24px; background: var(--surface); border-bottom: 1px solid rgba(128,128,128,0.2); }}\n.dg-nav-links {{ display: flex; gap: 18px; flex-wrap: wrap; }}\n.dg-nav a {{ color: inherit; text-decoration: none; font-weight: 500; }}\n.dg-card {{ background: var(--surface); border: 1px solid rgba(128,128,128,0.22); border-radius: 10px; padding: 20px; }}\n.dg-grid {{ display: grid; gap: 18px; }}\n.dg-btn {{ cursor: pointer; border: none; border-radius: 6px; padding: 10px 16px; font-weight: 600; background: var(--accent); color: #fff; }}\n.dg-input {{ width: 100%; padding: 10px 12px; border-radius: 6px; border: 1px solid rgba(128,128,128,0.35); background: var(--surface); color: var(--ink); }}"
    )
}

#[derive(Debug, Clone)]
enum DgssBlock {
    Vars {
        line: usize,
        entries: Vec<(String, String)>,
    },
    Rule {
        selector: String,
        line: usize,
        decls: Vec<(String, String)>,
        children: Vec<DgssBlock>,
    },
    State {
        trigger: String,
        line: usize,
        decls: Vec<(String, String)>,
        rules: Vec<DgssBlock>,
    },
    Media {
        query: String,
        line: usize,
        rules: Vec<DgssBlock>,
    },
    Animate {
        name: String,
        stops: Vec<DgssBlock>,
    },
    KeyStop {
        stop: String,
        decls: Vec<(String, String)>,
    },
}

impl DgssBlock {
    fn kind_name(&self) -> &'static str {
        match self {
            DgssBlock::Vars { .. } => "vars",
            DgssBlock::Rule { .. } => "rule",
            DgssBlock::State { .. } => "on",
            DgssBlock::Media { .. } => "media",
            DgssBlock::Animate { .. } => "animate",
            DgssBlock::KeyStop { .. } => "keystop",
        }
    }

    fn line_no(&self) -> usize {
        match self {
            DgssBlock::Vars { line, .. }
            | DgssBlock::Rule { line, .. }
            | DgssBlock::State { line, .. }
            | DgssBlock::Media { line, .. } => *line,
            _ => 1,
        }
    }
}

fn strip_dgss_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'\'' && !in_double {
            in_single = !in_single;
        } else if ch == b'"' && !in_single {
            in_double = !in_double;
        } else if !in_single && !in_double {
            if ch == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                return &line[..i];
            }
            if ch == b'#' {
                let rest = &line[i + 1..];
                let looks_hex = rest
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_hexdigit());
                if i == 0 || (bytes[i - 1].is_ascii_whitespace() && !looks_hex) {
                    return &line[..i];
                }
            }
        }
        i += 1;
    }
    line
}

pub fn compile_dog_css(dsl: &str) -> Result<String, String> {
    let mut stack: Vec<DgssBlock> = Vec::new();
    let mut roots: Vec<DgssBlock> = Vec::new();

    for (idx, raw) in dsl.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = strip_dgss_comment(raw).trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "end" || trimmed == "}" {
            let finished = stack.pop().ok_or_else(|| {
                format!("DOG CSS line {line_no}: unexpected `end` with no open block.")
            })?;
            match stack.last_mut() {
                None => roots.push(finished),
                Some(DgssBlock::Rule { children, .. })
                    if matches!(finished, DgssBlock::State { .. } | DgssBlock::Rule { .. }) =>
                {
                    children.push(finished);
                }
                Some(DgssBlock::Media { rules, .. }) | Some(DgssBlock::State { rules, .. })
                    if matches!(finished, DgssBlock::Rule { .. } | DgssBlock::Vars { .. }) =>
                {
                    rules.push(finished);
                }
                Some(DgssBlock::Animate { stops, .. })
                    if matches!(finished, DgssBlock::KeyStop { .. }) =>
                {
                    stops.push(finished);
                }
                Some(parent) => {
                    return Err(format!(
                        "DOG CSS line {line_no}: cannot nest `{}` directly inside `{}`.",
                        finished.kind_name(),
                        parent.kind_name()
                    ));
                }
            }
            continue;
        }

        let open_body = trimmed.trim_end_matches('{').trim();
        if open_body == "vars" {
            stack.push(DgssBlock::Vars {
                line: line_no,
                entries: Vec::new(),
            });
            continue;
        }
        if let Some(sel_raw) = open_body.strip_prefix("rule ") {
            let selector = sel_raw.trim().to_string();
            if selector.is_empty() {
                return Err(format!(
                    "DOG CSS line {line_no}: expected selector after `rule`."
                ));
            }
            stack.push(DgssBlock::Rule {
                selector,
                line: line_no,
                decls: Vec::new(),
                children: Vec::new(),
            });
            continue;
        }
        if let Some(trig_raw) = open_body.strip_prefix("on ") {
            let trigger = trig_raw.trim().to_string();
            if trigger.is_empty() {
                return Err(format!(
                    "DOG CSS line {line_no}: expected state or breakpoint after `on`."
                ));
            }
            stack.push(DgssBlock::State {
                trigger,
                line: line_no,
                decls: Vec::new(),
                rules: Vec::new(),
            });
            continue;
        }
        if let Some(q_raw) = open_body.strip_prefix("media ") {
            stack.push(DgssBlock::Media {
                query: q_raw.trim().to_string(),
                line: line_no,
                rules: Vec::new(),
            });
            continue;
        }
        if let Some(name_raw) = open_body.strip_prefix("animate ") {
            stack.push(DgssBlock::Animate {
                name: name_raw.trim().to_string(),
                stops: Vec::new(),
            });
            continue;
        }
        if matches!(stack.last(), Some(DgssBlock::Animate { .. }))
            && (open_body == "from"
                || open_body == "to"
                || (open_body.ends_with('%')
                    && open_body[..open_body.len() - 1]
                        .chars()
                        .all(|c| c.is_ascii_digit())))
        {
            stack.push(DgssBlock::KeyStop {
                stop: open_body.to_string(),
                decls: Vec::new(),
            });
            continue;
        }

        let decl_clean = trimmed.trim_end_matches(';').trim();
        let split_at = decl_clean
            .find(':')
            .filter(|&p| p > 0)
            .or_else(|| decl_clean.find('=').filter(|&p| p > 0));
        let Some(pos) = split_at else {
            return Err(format!(
                "DOG CSS line {line_no}: unrecognized line `{trimmed}`. Expected `prop: value`."
            ));
        };
        let k = decl_clean[..pos].trim().to_string();
        let v = decl_clean[pos + 1..].trim().to_string();
        match stack.last_mut() {
            None => {
                return Err(format!(
                    "DOG CSS line {line_no}: declaration `{trimmed}` must be inside `vars`, `rule`, or `on`."
                ));
            }
            Some(DgssBlock::Vars { entries, .. }) => entries.push((k, v)),
            Some(DgssBlock::Rule { decls, .. })
            | Some(DgssBlock::State { decls, .. })
            | Some(DgssBlock::KeyStop { decls, .. }) => decls.push((k, v)),
            Some(other) => {
                return Err(format!(
                    "DOG CSS line {line_no}: property `{k}` cannot appear directly inside `{}`. Open a `rule` or stop first.",
                    other.kind_name()
                ));
            }
        }
    }

    if let Some(unclosed) = stack.last() {
        return Err(format!(
            "DOG CSS line {}: unclosed `{}` block (missing `end`).",
            unclosed.line_no(),
            unclosed.kind_name()
        ));
    }

    let mut blocks = Vec::new();
    for b in &roots {
        emit_dgss_block(b, None, &mut blocks)?;
    }
    Ok(blocks.join("\n\n"))
}

fn emit_dgss_block(
    block: &DgssBlock,
    parent_sel: Option<&str>,
    out: &mut Vec<String>,
) -> Result<(), String> {
    match block {
        DgssBlock::Vars { entries, .. } => {
            if !entries.is_empty() {
                out.push(render_css_vars(entries));
            }
        }
        DgssBlock::Rule {
            selector,
            decls,
            children,
            ..
        } => {
            let full_sel = match parent_sel {
                Some(p) => {
                    if selector.contains('&') {
                        selector.replace('&', p)
                    } else {
                        format!("{p} {selector}")
                    }
                }
                None => selector.clone(),
            };
            if !decls.is_empty() {
                out.push(render_css_rule(&full_sel, decls));
            }
            for ch in children {
                emit_dgss_block(ch, Some(&full_sel), out)?;
            }
        }
        DgssBlock::State {
            trigger,
            line,
            decls,
            rules,
        } => {
            let trig_l = trigger.to_ascii_lowercase();
            if matches!(trig_l.as_str(), "mobile" | "tablet" | "desktop" | "dark") {
                let mut inner = Vec::new();
                if let Some(p) = parent_sel {
                    if !decls.is_empty() {
                        inner.push(render_css_rule(p, decls));
                    }
                }
                for r in rules {
                    emit_dgss_block(r, None, &mut inner)?;
                }
                out.push(render_css_media(&trig_l, &inner.join("\n\n")));
            } else if let Some(p) = parent_sel {
                if !decls.is_empty() {
                    let pseudo = format!("{p}:{trigger}");
                    out.push(render_css_rule(&pseudo, decls));
                }
            } else {
                return Err(format!(
                    "DOG CSS line {line}: `on {trigger}` must be nested inside a `rule` block."
                ));
            }
        }
        DgssBlock::Media { query, rules, .. } => {
            let mut inner = Vec::new();
            for r in rules {
                emit_dgss_block(r, None, &mut inner)?;
            }
            out.push(render_css_media(query, &inner.join("\n\n")));
        }
        DgssBlock::Animate { name, stops } => {
            let mut rendered_stops = Vec::with_capacity(stops.len());
            for st in stops {
                if let DgssBlock::KeyStop { stop, decls } = st {
                    let mut dlines = Vec::new();
                    for (k, v) in decls {
                        for (ek, ev) in expand_css_property_and_value(k, v) {
                            dlines.push(format!("    {ek}: {ev};"));
                        }
                    }
                    rendered_stops.push(format!("  {stop} {{\n{}\n  }}", dlines.join("\n")));
                }
            }
            out.push(format!(
                "@keyframes {} {{\n{}\n}}",
                name,
                rendered_stops.join("\n")
            ));
        }
        DgssBlock::KeyStop { .. } => {}
    }
    Ok(())
}

pub fn html_attr_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn sanitize_tag_name(raw: &str) -> String {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if clean.is_empty() {
        "div".to_string()
    } else {
        clean.to_ascii_lowercase()
    }
}

fn is_void_html_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub fn render_html_el(tag: &str, attrs: &[(String, String)], content: &str) -> String {
    let clean_tag = sanitize_tag_name(tag);
    let mut attr_str = String::new();
    for (k, v) in attrs {
        let attr_name: String = k
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if attr_name.is_empty() {
            continue;
        }
        attr_str.push_str(&format!(" {attr_name}=\"{}\"", html_attr_escape(v)));
    }
    if is_void_html_tag(&clean_tag) {
        format!("<{clean_tag}{attr_str} />")
    } else {
        format!("<{clean_tag}{attr_str}>{content}</{clean_tag}>")
    }
}

pub fn render_html_page(title: &str, css: &str, body: &str) -> String {
    let escaped_title = crate::net::html_escape(title);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n  <title>{escaped_title}</title>\n  <style>\n{css}\n  </style>\n</head>\n<body>\n{body}\n</body>\n</html>\n"
    )
}

pub fn render_web_nav(brand: &str, links: &[(String, String)]) -> String {
    let links_html = links
        .iter()
        .map(|(label, href)| {
            format!(
                "<a href=\"{}\">{}</a>",
                html_attr_escape(href),
                crate::net::html_escape(label)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<header class=\"dg-nav\"><strong>{}</strong><nav class=\"dg-nav-links\">{links_html}</nav></header>",
        crate::net::html_escape(brand)
    )
}

pub fn render_web_card(title: &str, body: &str) -> String {
    format!(
        "<article class=\"dg-card\"><h3>{}</h3><div>{body}</div></article>",
        crate::net::html_escape(title)
    )
}

pub fn render_web_grid(cols: i64, items: &[String]) -> String {
    let c = cols.max(1);
    format!(
        "<div class=\"dg-grid\" style=\"display:grid;grid-template-columns:repeat({c},minmax(0,1fr));\">{}</div>",
        items.join("\n")
    )
}

pub fn render_web_button(id: &str, label: &str) -> String {
    format!(
        "<button id=\"{}\" class=\"dg-btn\" type=\"button\">{}</button>",
        html_attr_escape(id),
        crate::net::html_escape(label)
    )
}

pub fn render_web_input(id: &str, placeholder: &str, value: &str) -> String {
    format!(
        "<input id=\"{}\" class=\"dg-input\" type=\"text\" placeholder=\"{}\" value=\"{}\" />",
        html_attr_escape(id),
        html_attr_escape(placeholder),
        html_attr_escape(value)
    )
}

#[derive(Debug, Clone)]
pub struct WebBundle {
    pub index_html: String,
    pub css: String,
    pub js: String,
}

#[derive(Debug, Clone)]
pub struct WebBuildArtifacts {
    pub index_html: String,
    pub styles_css: String,
    pub app_js: String,
    pub single_file_html: String,
}

pub fn build_web_bundle(
    dg_src: &str,
    dgss_src: Option<&str>,
    title: &str,
) -> Result<WebBundle, String> {
    let program = crate::interpreter::check_source("<web>", dg_src).map_err(|d| format!("{d}"))?;
    let js = transpile_program_to_js(&program)?;

    let mut css_parts = Vec::new();
    if let Some(raw_dgss) = dgss_src {
        if !raw_dgss.trim().is_empty() {
            css_parts.push(compile_dog_css(raw_dgss)?);
        }
    }

    reset_web_runtime();
    crate::stdlib::reset_headless_web();
    let mut io = SilentWebIo;
    let mut interp = crate::interpreter::Interpreter::new(&mut io);
    let _ = interp.eval_program(&program);
    let runtime_css = build_compiled_css();
    if !runtime_css.trim().is_empty() {
        css_parts.push(runtime_css);
    }

    let css = css_parts.join("\n\n");
    let escaped_title = crate::net::html_escape(title);
    let index_html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n  <title>{escaped_title}</title>\n  <link rel=\"stylesheet\" href=\"./styles.css\" />\n</head>\n<body>\n  <div id=\"app\"></div>\n  <script src=\"./app.js\"></script>\n</body>\n</html>\n"
    );

    Ok(WebBundle {
        index_html,
        css,
        js,
    })
}

pub fn build_web_artifacts(program: &Program, default_title: &str) -> Result<WebBuildArtifacts, String> {
    let app_js = transpile_program_to_js(program)?;
    reset_web_runtime();
    crate::stdlib::reset_headless_web();
    let mut dummy_io = SilentWebIo;
    let mut interp = crate::interpreter::Interpreter::new(&mut dummy_io);
    let _ = interp.eval_program(program);
    let styles_css = build_compiled_css();
    let escaped_title = crate::net::html_escape(default_title);

    let index_html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n  <title>{escaped_title}</title>\n  <link rel=\"stylesheet\" href=\"./styles.css\" />\n</head>\n<body>\n  <div id=\"app\"></div>\n  <script src=\"./app.js\"></script>\n</body>\n</html>\n"
    );

    let safe_js = app_js.replace("</script", "<\\/script");
    let single_file_html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n  <title>{escaped_title}</title>\n  <style id=\"dog-style-sheet\">\n{styles_css}\n  </style>\n</head>\n<body>\n  <div id=\"app\"></div>\n  <script>\n{safe_js}\n  </script>\n</body>\n</html>\n"
    );

    Ok(WebBuildArtifacts {
        index_html,
        styles_css,
        app_js,
        single_file_html,
    })
}

pub fn build_web_bundle_to_disk(
    input_path: &str,
    out_path: Option<&str>,
) -> Result<(PathBuf, WebBuildArtifacts), String> {
    let source = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read '{input_path}': {e}"))?;
    let dgss_candidate = Path::new(input_path).with_extension("dgss");
    let dgss_src = if dgss_candidate.is_file() {
        Some(
            fs::read_to_string(&dgss_candidate)
                .map_err(|e| format!("Failed to read '{}': {e}", dgss_candidate.display()))?,
        )
    } else {
        None
    };

    let stem = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dog_site");
    let bundle = build_web_bundle(&source, dgss_src.as_deref(), stem)?;
    let safe_js = bundle.js.replace("</script", "<\\/script");
    let single_file_html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\" />\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n  <title>{}</title>\n  <style id=\"dog-style-sheet\">\n{}\n  </style>\n</head>\n<body>\n  <div id=\"app\"></div>\n  <script>\n{}\n  </script>\n</body>\n</html>\n",
        crate::net::html_escape(stem),
        bundle.css,
        safe_js
    );

    let artifacts = WebBuildArtifacts {
        index_html: bundle.index_html,
        styles_css: bundle.css,
        app_js: bundle.js,
        single_file_html,
    };

    let target = match out_path {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from("dist").join(stem),
    };

    if target
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
    {
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create '{}': {e}", parent.display()))?;
            }
        }
        fs::write(&target, &artifacts.single_file_html)
            .map_err(|e| format!("Failed to write '{}': {e}", target.display()))?;
        return Ok((target, artifacts));
    }

    fs::create_dir_all(&target)
        .map_err(|e| format!("Failed to create output directory '{}': {e}", target.display()))?;
    fs::write(target.join("index.html"), &artifacts.index_html)
        .map_err(|e| format!("Failed to write index.html: {e}"))?;
    fs::write(target.join("styles.css"), &artifacts.styles_css)
        .map_err(|e| format!("Failed to write styles.css: {e}"))?;
    fs::write(target.join("app.js"), &artifacts.app_js)
        .map_err(|e| format!("Failed to write app.js: {e}"))?;

    Ok((target, artifacts))
}

pub fn transpile_program_to_js(program: &Program) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("// Generated by DOG Compiler (dog web build / dog build --web)\n");
    out.push_str(JS_WEB_RUNTIME);
    out.push('\n');

    let mut fn_defaults: HashMap<String, Vec<Option<Expr>>> = HashMap::new();
    for stmt in &program.statements {
        if let Stmt::FnDef { name, params, .. } = stmt {
            fn_defaults.insert(
                name.clone(),
                params.iter().map(|p| p.default.clone()).collect(),
            );
        }
    }

    for stmt in &program.statements {
        match stmt {
            Stmt::Use { .. } => {}
            Stmt::StructDef { name, fields, .. } => {
                let fnames: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
                out.push_str(&format!(
                    "function __struct_{name}(fields) {{ return Object.assign({{ {} }}, fields); }}\n\n",
                    fnames
                        .iter()
                        .map(|f| format!("{f}: undefined"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Stmt::EnumDef { name, variants, .. } => {
                let pairs: Vec<String> = variants
                    .iter()
                    .map(|v| format!("{v}: \"{name}.{v}\""))
                    .collect();
                out.push_str(&format!(
                    "const {name} = Object.freeze({{ {} }});\n\n",
                    pairs.join(", ")
                ));
            }
            Stmt::FnDef {
                name, params, body, ..
            } => {
                let mut p_strs = Vec::with_capacity(params.len());
                for p in params {
                    if let Some(def) = &p.default {
                        p_strs.push(format!("{} = {}", p.name, emit_js_expr(def, &fn_defaults)?));
                    } else {
                        p_strs.push(p.name.clone());
                    }
                }
                out.push_str(&format!("function {name}({}) {{\n", p_strs.join(", ")));
                for s in body {
                    emit_js_stmt(&mut out, s, 1, &fn_defaults)?;
                }
                out.push_str("}\n");
                out.push_str(&format!("__dogFns[\"{name}\"] = {name};\n\n"));
            }
            _ => {}
        }
    }

    out.push_str("function __dog_main() {\n");
    for stmt in &program.statements {
        if matches!(
            stmt,
            Stmt::Use { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } | Stmt::FnDef { .. }
        ) {
            continue;
        }
        emit_js_stmt(&mut out, stmt, 1, &fn_defaults)?;
    }
    out.push_str("  __dog_flush_styles();\n");
    out.push_str("}\n\n");
    out.push_str("if (typeof document !== \"undefined\") {\n");
    out.push_str("  if (document.readyState === \"loading\") {\n");
    out.push_str("    document.addEventListener(\"DOMContentLoaded\", __dog_main);\n");
    out.push_str("  } else {\n");
    out.push_str("    __dog_main();\n");
    out.push_str("  }\n");
    out.push_str("}\n");

    Ok(out)
}

fn emit_js_style_rules(
    out: &mut String,
    pad: &str,
    media_expr: Option<&str>,
    sel_js: &str,
    rules: &[StyleRuleDecl],
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<(), String> {
    let mut pairs = Vec::with_capacity(rules.len());
    for r in rules {
        let prop_escaped = escape_js_str(&r.prop);
        let val_js = emit_js_expr(&r.value, fn_defaults)?;
        pairs.push(format!("\"{prop_escaped}\": __dog_text({val_js})"));
    }
    let map_js = format!("{{ {} }}", pairs.join(", "));
    match media_expr {
        Some(q) => {
            let q_esc = escape_js_str(q);
            out.push_str(&format!(
                "{pad}__dogWeb.styleMedia(\"{q_esc}\", {sel_js}, {map_js});\n"
            ));
        }
        None => {
            out.push_str(&format!("{pad}__dogWeb.styleRule({sel_js}, {map_js});\n"));
        }
    }
    Ok(())
}

fn emit_js_variant(
    out: &mut String,
    pad: &str,
    sel_js: &str,
    v: &StyleVariantBlock,
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<(), String> {
    if v.condition.starts_with(':') {
        let suffix = escape_js_str(&v.condition);
        let derived_sel = format!("`\\${{{sel_js}}}{suffix}`");
        emit_js_style_rules(out, pad, None, &derived_sel, &v.rules, fn_defaults)
    } else {
        let q = v
            .condition
            .strip_prefix("@media")
            .map(str::trim)
            .unwrap_or(v.condition.as_str());
        emit_js_style_rules(out, pad, Some(q), sel_js, &v.rules, fn_defaults)
    }
}

fn emit_js_stmt(
    out: &mut String,
    stmt: &Stmt,
    indent: usize,
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<(), String> {
    let pad = "  ".repeat(indent);
    match stmt {
        Stmt::Use { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } | Stmt::FnDef { .. } => {
            Ok(())
        }
        Stmt::StyleDef {
            selector,
            rules,
            variants,
            ..
        } => {
            let sel_js = emit_js_expr(selector, fn_defaults)?;
            if !rules.is_empty() {
                emit_js_style_rules(out, &pad, None, &sel_js, rules, fn_defaults)?;
            }
            for v in variants {
                emit_js_variant(out, &pad, &sel_js, v, fn_defaults)?;
            }
            Ok(())
        }
        Stmt::Binding {
            kind, name, value, ..
        } => {
            let kw = match kind {
                BindingKind::Var => "let",
                BindingKind::Let | BindingKind::Const => "const",
            };
            out.push_str(&format!(
                "{pad}{kw} {name} = {};\n",
                emit_js_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::Assign { name, value, .. } => {
            out.push_str(&format!(
                "{pad}{name} = {};\n",
                emit_js_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::CompoundAssign {
            name, op, value, ..
        } => {
            let rhs = emit_js_expr(value, fn_defaults)?;
            let sym = match op {
                BinOp::Add => "+=",
                BinOp::Sub => "-=",
                BinOp::Mul => "*=",
                BinOp::Div => "/=",
                BinOp::Mod => "%=",
                _ => return Err("Unsupported compound assignment operator.".to_string()),
            };
            out.push_str(&format!("{pad}{name} {sym} {rhs};\n"));
            Ok(())
        }
        Stmt::FieldAssign {
            target,
            field,
            value,
            ..
        } => {
            out.push_str(&format!(
                "{pad}{target}.{field} = {};\n",
                emit_js_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::IndexAssign {
            target,
            index,
            value,
            ..
        } => {
            out.push_str(&format!(
                "{pad}{target}[{}] = {};\n",
                emit_js_expr(index, fn_defaults)?,
                emit_js_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::If {
            condition,
            then_branch,
            otherwise_branch,
            ..
        } => {
            out.push_str(&format!(
                "{pad}if ({}) {{\n",
                emit_js_expr(condition, fn_defaults)?
            ));
            for s in then_branch {
                emit_js_stmt(out, s, indent + 1, fn_defaults)?;
            }
            if let Some(el) = otherwise_branch {
                out.push_str(&format!("{pad}}} else {{\n"));
                for s in el {
                    emit_js_stmt(out, s, indent + 1, fn_defaults)?;
                }
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::Match {
            subject,
            cases,
            otherwise_branch,
            ..
        } => {
            out.push_str(&format!(
                "{pad}const __dog_match = {};\n",
                emit_js_expr(subject, fn_defaults)?
            ));
            for (idx, c) in cases.iter().enumerate() {
                let kw = if idx == 0 { "if" } else { "} else if" };
                out.push_str(&format!(
                    "{pad}{kw} (__dog_match === {}) {{\n",
                    emit_js_expr(&c.pattern, fn_defaults)?
                ));
                for s in &c.body {
                    emit_js_stmt(out, s, indent + 1, fn_defaults)?;
                }
            }
            if let Some(el) = otherwise_branch {
                if cases.is_empty() {
                    for s in el {
                        emit_js_stmt(out, s, indent, fn_defaults)?;
                    }
                    return Ok(());
                }
                out.push_str(&format!("{pad}}} else {{\n"));
                for s in el {
                    emit_js_stmt(out, s, indent + 1, fn_defaults)?;
                }
            }
            if !cases.is_empty() {
                out.push_str(&format!("{pad}}}\n"));
            }
            Ok(())
        }
        Stmt::TryCatch {
            try_body,
            err_name,
            catch_body,
            ..
        } => {
            out.push_str(&format!("{pad}try {{\n"));
            for s in try_body {
                emit_js_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!(
                "{pad}}} catch (__raw_err) {{\n{pad}  const {err_name} = (__raw_err && __raw_err.message) ? __raw_err.message : String(__raw_err);\n"
            ));
            for s in catch_body {
                emit_js_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::While {
            condition, body, ..
        } => {
            out.push_str(&format!(
                "{pad}while ({}) {{\n",
                emit_js_expr(condition, fn_defaults)?
            ));
            for s in body {
                emit_js_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::Repeat { count, body, .. } => {
            out.push_str(&format!(
                "{pad}for (let __rep = 0; __rep < ({}); __rep++) {{\n",
                emit_js_expr(count, fn_defaults)?
            ));
            for s in body {
                emit_js_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::For {
            var_name,
            iterable,
            body,
            ..
        } => {
            out.push_str(&format!(
                "{pad}for (const {var_name} of ({})) {{\n",
                emit_js_expr(iterable, fn_defaults)?
            ));
            for s in body {
                emit_js_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                out.push_str(&format!("{pad}return {};\n", emit_js_expr(v, fn_defaults)?));
            } else {
                out.push_str(&format!("{pad}return;\n"));
            }
            Ok(())
        }
        Stmt::Break { .. } => {
            out.push_str(&format!("{pad}break;\n"));
            Ok(())
        }
        Stmt::Continue { .. } => {
            out.push_str(&format!("{pad}continue;\n"));
            Ok(())
        }
        Stmt::ExprStmt { expr, .. } => {
            out.push_str(&format!("{pad}{};\n", emit_js_expr(expr, fn_defaults)?));
            Ok(())
        }
    }
}

fn escape_js_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn emit_js_expr(
    expr: &Expr,
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<String, String> {
    match expr {
        Expr::StringLit { value, .. } => Ok(format!("\"{}\"", escape_js_str(value))),
        Expr::InterpString { parts, .. } => {
            let mut body = String::from("`");
            for p in parts {
                match p {
                    InterpPart::Literal(lit) => {
                        body.push_str(
                            &lit.replace('\\', "\\\\")
                                .replace('`', "\\`")
                                .replace("${", "\\${"),
                        );
                    }
                    InterpPart::Expr(e) => {
                        body.push_str(&format!("${{__dog_text({})}}", emit_js_expr(e, fn_defaults)?));
                    }
                }
            }
            body.push('`');
            Ok(body)
        }
        Expr::NumberLit { value, .. } => Ok(value.to_string()),
        Expr::DecimalLit { value, .. } => Ok(format!("{value:?}")),
        Expr::BoolLit { value, .. } => Ok(value.to_string()),
        Expr::Identifier { name, .. } => Ok(name.clone()),
        Expr::ListLit { elements, .. } => {
            let mut items = Vec::with_capacity(elements.len());
            for e in elements {
                items.push(emit_js_expr(e, fn_defaults)?);
            }
            Ok(format!("[{}]", items.join(", ")))
        }
        Expr::MapLit { entries, .. } => {
            let mut items = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                items.push(format!(
                    "[{}]: {}",
                    emit_js_expr(k, fn_defaults)?,
                    emit_js_expr(v, fn_defaults)?
                ));
            }
            Ok(format!("{{ {} }}", items.join(", ")))
        }
        Expr::StructInit { name, fields, .. } => {
            let mut parts = Vec::with_capacity(fields.len());
            for (k, v) in fields {
                parts.push(format!("{k}: {}", emit_js_expr(v, fn_defaults)?));
            }
            Ok(format!("__struct_{name}({{ {} }})", parts.join(", ")))
        }
        Expr::FieldAccess { target, field, .. } => Ok(format!(
            "{}.{field}",
            emit_js_expr(target, fn_defaults)?
        )),
        Expr::Index { target, index, .. } => Ok(format!(
            "{}[{}]",
            emit_js_expr(target, fn_defaults)?,
            emit_js_expr(index, fn_defaults)?
        )),
        Expr::Unary { op, expr, .. } => match op {
            UnaryOp::Neg => Ok(format!("(-({}))", emit_js_expr(expr, fn_defaults)?)),
            UnaryOp::Not => Ok(format!("(!({}))", emit_js_expr(expr, fn_defaults)?)),
        },
        Expr::Binary {
            left, op, right, ..
        } => {
            let l = emit_js_expr(left, fn_defaults)?;
            let r = emit_js_expr(right, fn_defaults)?;
            match op {
                BinOp::Add => Ok(format!("({l} + {r})")),
                BinOp::Sub => Ok(format!("({l} - {r})")),
                BinOp::Mul => Ok(format!("({l} * {r})")),
                BinOp::Div => Ok(format!("({l} / {r})")),
                BinOp::Mod => Ok(format!("({l} % {r})")),
                BinOp::Eq => Ok(format!("({l} === {r})")),
                BinOp::NotEq => Ok(format!("({l} !== {r})")),
                BinOp::Gt => Ok(format!("({l} > {r})")),
                BinOp::Lt => Ok(format!("({l} < {r})")),
                BinOp::GtEq => Ok(format!("({l} >= {r})")),
                BinOp::LtEq => Ok(format!("({l} <= {r})")),
                BinOp::And => Ok(format!("({l} && {r})")),
                BinOp::Or => Ok(format!("({l} || {r})")),
            }
        }
        Expr::Call { callee, args, .. } => emit_js_call(callee, args, fn_defaults),
        Expr::NamespaceCall {
            module, func, args, ..
        } => {
            let canonical = crate::stdlib::modules::resolve_std_call(module, func)?;
            emit_js_call(canonical, args, fn_defaults)
        }
    }
}

fn emit_js_call(
    callee: &str,
    args: &[Expr],
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<String, String> {
    let mut jargs = Vec::with_capacity(args.len());
    for a in args {
        jargs.push(emit_js_expr(a, fn_defaults)?);
    }

    match callee {
        "say" | "print" | "cli_info" | "cli_success" | "cli_warn" | "cli_error" | "cli_debug"
        | "log_info" | "log_warn" | "log_error" => Ok(format!("__dogWeb.print({})", jargs.join(", "))),
        "text" => Ok(format!("__dog_text({})", jargs[0])),
        "number" => Ok(format!("__dog_number({})", jargs[0])),
        "decimal" => Ok(format!("__dog_decimal({})", jargs[0])),
        "len" => Ok(format!("__dog_len({})", jargs[0])),
        "lower" => Ok(format!("String({}).toLowerCase()", jargs[0])),
        "upper" => Ok(format!("String({}).toUpperCase()", jargs[0])),
        "trim" => Ok(format!("String({}).trim()", jargs[0])),
        "contains" => Ok(format!("String({}).includes(String({}))", jargs[0], jargs[1])),
        "starts_with" => Ok(format!("String({}).startsWith(String({}))", jargs[0], jargs[1])),
        "ends_with" => Ok(format!("String({}).endsWith(String({}))", jargs[0], jargs[1])),
        "replace" => Ok(format!(
            "String({}).split(String({})).join(String({}))",
            jargs[0], jargs[1], jargs[2]
        )),
        "slice" => Ok(format!(
            "Array.from(String({})).slice({}, {}).join(\"\")",
            jargs[0], jargs[1], jargs[2]
        )),
        "char_at" => Ok(format!("(Array.from(String({}))[{}] || \"\")", jargs[0], jargs[1])),
        "index_of" => Ok(format!("String({}).indexOf(String({}))", jargs[0], jargs[1])),
        "last_index_of" => Ok(format!("String({}).lastIndexOf(String({}))", jargs[0], jargs[1])),
        "split" => Ok(format!("String({}).split(String({}))", jargs[0], jargs[1])),
        "join" => Ok(format!("({}).map(__dog_text).join(String({}))", jargs[0], jargs[1])),
        "lines" => Ok(format!("String({}).split(/\\r?\\n/)", jargs[0])),
        "title_case" => Ok(format!("__dog_title_case({})", jargs[0])),
        "reverse_text" => Ok(format!("Array.from(String({})).reverse().join(\"\")", jargs[0])),
        "str_repeat" => Ok(format!("String({}).repeat(Math.max(0, {}))", jargs[0], jargs[1])),
        "pad_left" => Ok(format!("String({}).padStart({}, String({}))", jargs[0], jargs[1], jargs[2])),
        "pad_right" => Ok(format!("String({}).padEnd({}, String({}))", jargs[0], jargs[1], jargs[2])),
        "html_escape" => Ok(format!("__dog_html_escape({})", jargs[0])),
        "url_encode" => Ok(format!("encodeURIComponent(String({}))", jargs[0])),
        "url_decode" => Ok(format!("decodeURIComponent(String({}))", jargs[0])),
        "push" => Ok(format!("{}.push({})", jargs[0], jargs[1])),
        "pop" => Ok(format!("{}.pop()", jargs[0])),
        "insert_at" => Ok(format!("{}.splice({}, 0, {})", jargs[0], jargs[1], jargs[2])),
        "remove_at" => Ok(format!("{}.splice({}, 1)[0]", jargs[0], jargs[1])),
        "clear" => {
            if jargs.is_empty() {
                Ok("console.clear()".to_string())
            } else {
                Ok(format!("({}.length = 0)", jargs[0]))
            }
        }
        "sort" => Ok(format!(
            "{}.sort((a, b) => (typeof a === \"number\" && typeof b === \"number\" ? a - b : String(a).localeCompare(String(b))))",
            jargs[0]
        )),
        "reverse" => Ok(format!("{}.reverse()", jargs[0])),
        "list_contains" => Ok(format!("({}).includes({})", jargs[0], jargs[1])),
        "list_index_of" => Ok(format!("({}).indexOf({})", jargs[0], jargs[1])),
        "list_slice" => Ok(format!("({}).slice({}, {})", jargs[0], jargs[1], jargs[2])),
        "list_unique" => Ok(format!("Array.from(new Set({}))", jargs[0])),
        "list_take" => Ok(format!("({}).slice(0, Math.max(0, {}))", jargs[0], jargs[1])),
        "list_skip" => Ok(format!("({}).slice(Math.max(0, {}))", jargs[0], jargs[1])),
        "list_flatten" => Ok(format!("({}).flat()", jargs[0])),
        "list_concat" => Ok(format!("({}).concat({})", jargs[0], jargs[1])),
        "range" => Ok(format!("__dog_range({}, {})", jargs[0], jargs[1])),
        "list_sum" => Ok(format!(
            "({}).reduce((acc, v) => acc + Number(v), 0)",
            jargs[0]
        )),
        "list_avg" => Ok(format!(
            "(({}).length ? ({}).reduce((acc, v) => acc + Number(v), 0) / ({}).length : 0)",
            jargs[0], jargs[0], jargs[0]
        )),
        "map_get" => Ok(format!("({}[{}] ?? \"\")", jargs[0], jargs[1])),
        "map_get_or" => Ok(format!("(Object.prototype.hasOwnProperty.call({}, {}) ? {}[{}] : {})", jargs[0], jargs[1], jargs[0], jargs[1], jargs[2])),
        "map_set" => Ok(format!("({}[{}] = {})", jargs[0], jargs[1], jargs[2])),
        "map_has" => Ok(format!("Object.prototype.hasOwnProperty.call({}, {})", jargs[0], jargs[1])),
        "map_remove" => Ok(format!("(delete {}[{}])", jargs[0], jargs[1])),
        "map_keys" => Ok(format!("Object.keys({})", jargs[0])),
        "map_values" => Ok(format!("Object.values({})", jargs[0])),
        "abs" => Ok(format!("Math.abs({})", jargs[0])),
        "min" => Ok(format!("Math.min({}, {})", jargs[0], jargs[1])),
        "max" => Ok(format!("Math.max({}, {})", jargs[0], jargs[1])),
        "clamp" => Ok(format!("Math.min(Math.max({}, {}), {})", jargs[0], jargs[1], jargs[2])),
        "round" => Ok(format!("Math.round({})", jargs[0])),
        "floor" => Ok(format!("Math.floor({})", jargs[0])),
        "ceil" => Ok(format!("Math.ceil({})", jargs[0])),
        "sqrt" => Ok(format!("Math.sqrt({})", jargs[0])),
        "pow" | "pow_int" => Ok(format!("Math.pow({}, {})", jargs[0], jargs[1])),
        "sin" => Ok(format!("Math.sin({})", jargs[0])),
        "cos" => Ok(format!("Math.cos({})", jargs[0])),
        "tan" => Ok(format!("Math.tan({})", jargs[0])),
        "atan2" => Ok(format!("Math.atan2({}, {})", jargs[0], jargs[1])),
        "exp" => Ok(format!("Math.exp({})", jargs[0])),
        "log" => Ok(format!("Math.log({})", jargs[0])),
        "log10" => Ok(format!("Math.log10({})", jargs[0])),
        "sign" => Ok(format!("Math.sign({})", jargs[0])),
        "math_pi" => Ok("Math.PI".to_string()),
        "math_e" => Ok("Math.E".to_string()),
        "json_parse" => Ok(format!("JSON.parse(String({}))", jargs[0])),
        "json_stringify" => Ok(format!("JSON.stringify({})", jargs[0])),
        "json_pretty" => Ok(format!("JSON.stringify({}, null, 2)", jargs[0])),
        "time_now_ms" => Ok("Date.now()".to_string()),
        "time_utc" => Ok("new Date().toISOString()".to_string()),
        "assert" => Ok(format!("__dog_assert({})", jargs.join(", "))),
        "fail" => Ok(format!("(() => {{ throw new Error(String({})); }})()", jargs[0])),
        // Canonical 29 DOG Web & DOG CSS built-ins (std.css + std.web)
        "dog_css" => Ok(format!("__dogWeb.dogCss({})", jargs[0])),
        "css_vars" => Ok(format!("__dogWeb.cssVars({})", jargs[0])),
        "css_rule" => Ok(format!("__dogWeb.cssRule({}, {})", jargs[0], jargs[1])),
        "css_media" => Ok(format!("__dogWeb.cssMedia({}, {})", jargs[0], jargs[1])),
        "css_keyframes" => Ok(format!("__dogWeb.cssKeyframes({}, {})", jargs[0], jargs[1])),
        "css_theme" => Ok(format!(
            "__dogWeb.cssTheme({}, {}, {}, {}, {})",
            jargs[0], jargs[1], jargs[2], jargs[3], jargs[4]
        )),
        "css_bundle" => Ok(format!("({}).map(__dog_text).join(\"\\n\\n\")", jargs[0])),
        "html_el" => Ok(format!("__dogWeb.htmlEl({}, {}, {})", jargs[0], jargs[1], jargs[2])),
        "html_page" => Ok(format!("__dogWeb.htmlPage({}, {}, {})", jargs[0], jargs[1], jargs[2])),
        "web_nav" => Ok(format!("__dogWeb.webNav({}, {})", jargs[0], jargs[1])),
        "web_card" => Ok(format!("__dogWeb.webCard({}, {})", jargs[0], jargs[1])),
        "web_grid" => Ok(format!("__dogWeb.webGrid({}, {})", jargs[0], jargs[1])),
        "web_button" => Ok(format!("__dogWeb.webButton({}, {})", jargs[0], jargs[1])),
        "web_input" => Ok(format!("__dogWeb.webInput({}, {}, {})", jargs[0], jargs[1], jargs[2])),
        "dom_mount" => Ok(format!("__dogWeb.domMount({}, {})", jargs[0], jargs[1])),
        "dom_set_text" => Ok(format!("__dogWeb.domSetText({}, {})", jargs[0], jargs[1])),
        "dom_set_html" => Ok(format!("__dogWeb.domSetHtml({}, {})", jargs[0], jargs[1])),
        "dom_get_value" => Ok(format!("__dogWeb.domGetValue({})", jargs[0])),
        "dom_set_value" => Ok(format!("__dogWeb.domSetValue({}, {})", jargs[0], jargs[1])),
        "dom_add_class" => Ok(format!("__dogWeb.domAddClass({}, {})", jargs[0], jargs[1])),
        "dom_remove_class" => Ok(format!("__dogWeb.domRemoveClass({}, {})", jargs[0], jargs[1])),
        "dom_toggle_class" => Ok(format!("__dogWeb.domToggleClass({}, {})", jargs[0], jargs[1])),
        "dom_set_style" => Ok(format!("__dogWeb.domSetStyle({}, {}, {})", jargs[0], jargs[1], jargs[2])),
        "dom_on" => Ok(format!("__dogWeb.domOn({}, {}, {})", jargs[0], jargs[1], jargs[2])),
        "web_title" => Ok(format!("__dogWeb.webTitle({})", jargs[0])),
        "web_storage_set" => Ok(format!("__dogWeb.webStorageSet({}, {})", jargs[0], jargs[1])),
        "web_storage_get" => Ok(format!("__dogWeb.webStorageGet({})", jargs[0])),
        "web_route" => Ok("__dogWeb.webRoute()".to_string()),
        "web_navigate" => Ok(format!("__dogWeb.webNavigate({})", jargs[0])),
        other => Ok(format!("{other}({})", jargs.join(", "))),
    }
}

const JS_WEB_RUNTIME: &str = r#""use strict";
const __dogFns = Object.create(null);
const __dogStorageFallback = Object.create(null);
const __dogCssVars = [];
const __dogStyleRules = [];

function __dog_text(v) {
  if (v === null || v === undefined) return "";
  if (Array.isArray(v)) return "[" + v.map(__dog_text).join(", ") + "]";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

function __dog_number(v) {
  const n = Number(v);
  if (!Number.isFinite(n) || (typeof v === "string" && v.trim() === "")) {
    throw new Error(`Cannot parse '${v}' as number.`);
  }
  return Math.trunc(n);
}

function __dog_decimal(v) {
  const n = Number(v);
  if (!Number.isFinite(n)) {
    throw new Error(`Cannot parse '${v}' as decimal.`);
  }
  return n;
}

function __dog_len(v) {
  if (typeof v === "string" || Array.isArray(v)) return v.length;
  if (v && typeof v === "object") return Object.keys(v).length;
  return 0;
}

function __dog_title_case(s) {
  return String(s).replace(/\b\w/g, (c) => c.toUpperCase());
}

function __dog_range(start, end) {
  const out = [];
  for (let i = start; i < end; i++) out.push(i);
  return out;
}

function __dog_assert(cond, msg) {
  if (!cond) throw new Error(msg ? `Assertion failed: ${msg}` : "Assertion failed.");
}

function __dog_html_escape(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function __dogQ(sel) {
  if (typeof document === "undefined") return null;
  const s = String(sel || "").trim();
  if (!s) return null;
  if (/^[#.\[]/.test(s)) return document.querySelector(s);
  return document.getElementById(s) || document.querySelector(s);
}

const __DOG_CSS_ALIASES = {
  bg: "background",
  fg: "color",
  ink: "color",
  text: "color",
  pad: "padding",
  p: "padding",
  "pad-x": "padding-inline",
  px: "padding-inline",
  "pad-y": "padding-block",
  py: "padding-block",
  m: "margin",
  "margin-x": "margin-inline",
  mx: "margin-inline",
  "margin-y": "margin-block",
  my: "margin-block",
  radius: "border-radius",
  round: "border-radius",
  font: "font-family",
  size: "font-size",
  weight: "font-weight",
  leading: "line-height",
  ease: "transition",
  rows: "grid-template-rows",
  align: "align-items",
  justify: "justify-content",
  w: "width",
  h: "height",
  "max-w": "max-width",
  "min-w": "min-width",
  "max-h": "max-height",
  "min-h": "min-height"
};

function __dog_expand_vars(val) {
  return String(val).replace(/\$([a-zA-Z_][a-zA-Z0-9_-]*)/g, "var(--$1)");
}

function __dog_expand_decl(k, v) {
  const prop = String(k).trim().toLowerCase();
  const val = __dog_expand_vars(String(v).trim());
  if (prop === "cols") {
    const tmpl = /^\d+$/.test(val) ? `repeat(${val}, minmax(0, 1fr))` : val;
    return [["display", "grid"], ["grid-template-columns", tmpl]];
  }
  if (prop === "grid") {
    return [["display", "grid"], ["grid-template-columns", val]];
  }
  if (prop === "flex") {
    const low = val.toLowerCase();
    if (low === "col" || low === "column") return [["display", "flex"], ["flex-direction", "column"]];
    if (low === "row") return [["display", "flex"], ["flex-direction", "row"]];
    return [["flex", val]];
  }
  if (prop === "shadow") {
    const presets = {
      sm: "0 1px 3px rgba(24, 26, 31, 0.08)",
      md: "0 6px 20px rgba(24, 26, 31, 0.08)",
      lg: "0 16px 36px rgba(24, 26, 31, 0.12)"
    };
    return [["box-shadow", presets[val.toLowerCase()] || val]];
  }
  return [[__DOG_CSS_ALIASES[prop] || prop, val]];
}

function __dog_expand_media(raw) {
  const q = String(raw || "").trim();
  const map = {
    mobile: "(max-width: 768px)",
    tablet: "(max-width: 1024px)",
    desktop: "(min-width: 1025px)",
    dark: "(prefers-color-scheme: dark)"
  };
  return map[q.toLowerCase()] || (q.startsWith("(") ? q : `(${q})`);
}

function __dog_flush_styles() {
  if (typeof document === "undefined" || (!__dogCssVars.length && !__dogStyleRules.length)) return;
  let el = document.getElementById("dog-inline-styles");
  if (!el) {
    el = document.createElement("style");
    el.id = "dog-inline-styles";
    (document.head || document.documentElement).appendChild(el);
  }
  const chunks = [];
  if (__dogCssVars.length) {
    chunks.push(`:root {\n${__dogCssVars.map(([k, v]) => `  ${k}: ${v};`).join("\n")}\n}`);
  }
  for (const r of __dogStyleRules) {
    const body = r.decls.map(([p, v]) => `  ${p}: ${v};`).join("\n");
    const block = `${r.selector} {\n${body}\n}`;
    chunks.push(r.media ? `@media ${__dog_expand_media(r.media)} {\n${block}\n}` : block);
  }
  el.textContent = chunks.join("\n\n");
}

const __dogWeb = {
  print(v) {
    const line = __dog_text(v);
    console.log(line);
    if (typeof window !== "undefined" && window.parent && window.parent !== window) {
      try { window.parent.postMessage({ source: "dog-web-preview", type: "log", line }, "*"); } catch {}
    }
  },
  styleVar(name, val) {
    const clean = String(name).trim().replace(/^--+/, "");
    const key = `--${clean}`;
    const v = __dog_expand_vars(String(val).trim());
    const found = __dogCssVars.find((p) => p[0] === key);
    if (found) found[1] = v;
    else __dogCssVars.push([key, v]);
    __dog_flush_styles();
  },
  styleRule(selector, props) {
    const raw = String(selector).trim();
    const sel = raw === "theme" || raw === "root" ? ":root" : raw;
    const decls = [];
    for (const [k, v] of Object.entries(props || {})) {
      if (sel === ":root" && !String(k).trim().startsWith("--")) {
        __dogWeb.styleVar(k, v);
      } else {
        for (const pair of __dog_expand_decl(k, v)) decls.push(pair);
      }
    }
    if (decls.length) __dogStyleRules.push({ media: null, selector: sel, decls });
    __dog_flush_styles();
  },
  styleMedia(query, selector, props) {
    const sel = String(selector).trim();
    const decls = [];
    for (const [k, v] of Object.entries(props || {})) {
      for (const pair of __dog_expand_decl(k, v)) decls.push(pair);
    }
    if (decls.length) __dogStyleRules.push({ media: String(query).trim(), selector: sel, decls });
    __dog_flush_styles();
  },
  dogCss(dsl) {
    const out = [];
    let mode = null;
    let sel = "";
    let decls = [];
    for (const raw of String(dsl || "").split(/\r?\n/)) {
      const ln = raw.trim();
      if (!ln || ln.startsWith("#") || ln.startsWith("//")) continue;
      if (ln === "end" || ln === "}") {
        if (mode === "vars") out.push(":root {\n" + decls.join("\n") + "\n}");
        else if (mode === "rule") out.push(sel + " {\n" + decls.join("\n") + "\n}");
        mode = null;
        sel = "";
        decls = [];
        continue;
      }
      if (ln === "vars") { mode = "vars"; decls = []; continue; }
      if (ln.startsWith("rule ")) { mode = "rule"; sel = ln.slice(5).trim(); decls = []; continue; }
      const idx = ln.indexOf(":");
      if (idx > 0 && mode) {
        const k = ln.slice(0, idx).trim();
        const v = __dog_expand_vars(ln.slice(idx + 1).trim());
        if (mode === "vars") decls.push("  --" + k.replace(/^--+/, "") + ": " + v + ";");
        else {
          for (const [ek, ev] of __dog_expand_decl(k, v)) decls.push("  " + ek + ": " + ev + ";");
        }
      }
    }
    return out.join("\n\n");
  },
  cssVars(vars) {
    const lines = Object.entries(vars || {}).map(([k, v]) => "  --" + String(k).replace(/^--+/, "") + ": " + __dog_expand_vars(v) + ";");
    return ":root {\n" + lines.join("\n") + "\n}";
  },
  cssRule(sel, props) {
    const lines = [];
    for (const [k, v] of Object.entries(props || {})) {
      for (const [ek, ev] of __dog_expand_decl(k, v)) lines.push("  " + ek + ": " + ev + ";");
    }
    return sel + " {\n" + lines.join("\n") + "\n}";
  },
  cssMedia(q, css) {
    return "@media " + __dog_expand_media(q) + " {\n" + css + "\n}";
  },
  cssKeyframes(name, steps) {
    return "@keyframes " + name + " {" + Object.entries(steps || {}).map(([s, d]) => s + "{" + d + "}").join(" ") + "}";
  },
  cssTheme(bg, surface, ink, accent, font) {
    return `:root{--bg:${bg};--surface:${surface};--ink:${ink};--accent:${accent};--font:${font};}*{box-sizing:border-box;}body{margin:0;background:var(--bg);color:var(--ink);font-family:var(--font);line-height:1.6;}.dg-nav{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:14px 24px;background:var(--surface);border-bottom:1px solid rgba(128,128,128,0.2);}.dg-nav-links{display:flex;gap:18px;flex-wrap:wrap;}.dg-nav a{color:inherit;text-decoration:none;font-weight:500;}.dg-card{background:var(--surface);border:1px solid rgba(128,128,128,0.22);border-radius:10px;padding:20px;}.dg-grid{display:grid;gap:18px;}.dg-btn{cursor:pointer;border:none;border-radius:6px;padding:10px 16px;font-weight:600;background:var(--accent);color:#fff;}.dg-input{width:100%;padding:10px 12px;border-radius:6px;border:1px solid rgba(128,128,128,0.35);background:var(--surface);color:var(--ink);}`;
  },
  htmlEl(tag, attrs, content) {
    const cleanTag = String(tag || "div").replace(/[^a-zA-Z0-9_-]/g, "") || "div";
    const a = Object.entries(attrs || {}).map(([k, v]) => " " + k + '="' + __dog_html_escape(v) + '"').join("");
    if (["area","base","br","col","embed","hr","img","input","link","meta","param","source","track","wbr"].includes(cleanTag.toLowerCase())) {
      return "<" + cleanTag + a + " />";
    }
    return "<" + cleanTag + a + ">" + String(content) + "</" + cleanTag + ">";
  },
  htmlPage(title, css, body) {
    return "<!doctype html><html><head><meta charset=\"utf-8\" /><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\" /><title>" + __dog_html_escape(title) + "</title><style>" + String(css) + "</style></head><body>" + String(body) + "</body></html>";
  },
  webNav(brand, links) {
    const a = Object.entries(links || {}).map(([k, href]) => '<a href="' + __dog_html_escape(href) + '">' + __dog_html_escape(k) + "</a>").join("");
    return '<header class="dg-nav"><strong>' + __dog_html_escape(brand) + '</strong><nav class="dg-nav-links">' + a + "</nav></header>";
  },
  webCard(title, body) {
    return '<article class="dg-card"><h3>' + __dog_html_escape(title) + "</h3><div>" + String(body) + "</div></article>";
  },
  webGrid(cols, items) {
    return '<div class="dg-grid" style="display:grid;grid-template-columns:repeat(' + Math.max(1, Number(cols) || 1) + ',minmax(0,1fr));">' + (items || []).join("\n") + "</div>";
  },
  webButton(id, label) {
    return '<button id="' + __dog_html_escape(id) + '" class="dg-btn" type="button">' + __dog_html_escape(label) + "</button>";
  },
  webInput(id, ph, val) {
    return '<input id="' + __dog_html_escape(id) + '" class="dg-input" type="text" placeholder="' + __dog_html_escape(ph) + '" value="' + __dog_html_escape(val) + '" />';
  },
  domMount(sel, html) {
    const el = __dogQ(sel) || (typeof document !== "undefined" ? (document.getElementById("app") || document.body) : null);
    if (el) el.innerHTML = String(html);
  },
  domSetText(sel, val) { const el = __dogQ(sel); if (el) el.textContent = String(val); },
  domSetHtml(sel, html) { const el = __dogQ(sel); if (el) el.innerHTML = String(html); },
  domGetValue(sel) { const el = __dogQ(sel); return el && "value" in el ? String(el.value) : ""; },
  domSetValue(sel, val) { const el = __dogQ(sel); if (el && "value" in el) el.value = String(val); },
  domAddClass(sel, cls) { const el = __dogQ(sel); if (el && el.classList) el.classList.add(...String(cls).split(/\s+/).filter(Boolean)); },
  domRemoveClass(sel, cls) { const el = __dogQ(sel); if (el && el.classList) el.classList.remove(...String(cls).split(/\s+/).filter(Boolean)); },
  domToggleClass(sel, cls) {
    const el = __dogQ(sel);
    if (!el || !el.classList) return false;
    let state = false;
    for (const c of String(cls).split(/\s+/).filter(Boolean)) {
      state = Boolean(el.classList.toggle(c));
    }
    return state;
  },
  domSetStyle(sel, prop, val) {
    const el = __dogQ(sel);
    if (!el || !el.style) return;
    for (const [ek, ev] of __dog_expand_decl(prop, val)) {
      el.style.setProperty(ek, ev);
    }
  },
  domOn(sel, ev, fnName) {
    const el = __dogQ(sel);
    if (!el) return;
    el.addEventListener(String(ev), () => {
      const fn = __dogFns[fnName];
      if (typeof fn === "function") fn();
    });
  },
  webTitle(t) { if (typeof document !== "undefined") document.title = String(t); },
  webStorageSet(k, v) { try { localStorage.setItem(String(k), __dog_text(v)); } catch { __dogStorageFallback[k] = __dog_text(v); } },
  webStorageGet(k) { try { return localStorage.getItem(String(k)) || ""; } catch { return __dogStorageFallback[k] || ""; } },
  webRoute() { return typeof location !== "undefined" ? (location.hash || location.pathname || "/") : "/"; },
  webNavigate(p) { if (typeof location !== "undefined") location.hash = String(p).startsWith("#") ? String(p) : "#" + String(p); }
};
"#;
