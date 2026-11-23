use crate::ast::Span;
use crate::config::Personality;
use std::fmt;

pub const DOG_TIPS: &[&str] = &[
    "Use `let` for fixed bindings and `var` only when mutation is intentional.",
    "Indentation is 4 spaces by convention, while `end` strictly defines blocks.",
    "Convert text from `answer()` with `number()` or `decimal()` before math.",
    "Use `try ... catch err ... end` around external I/O or JSON parsing.",
    "Run `dog format --check` before committing to keep your codebase clean.",
];

pub const DOG_FACTS: &[&str] = &[
    "DOG compiles zero external Rust crates by default so it builds smoothly on Termux ARMv7.",
    "DOG forbids implicit `text + number` coercion to prevent subtle runtime bugs.",
    "DOG uses `end` instead of indentation or braces so copy-pasted blocks never break.",
    "DOG diagnostics always separate What, Why, Fix, and Tips so jokes never hide errors.",
];

pub const DOG_QUOTES: &[&str] = &[
    "Strict types today save three hours of debugging at 2 AM tomorrow.",
    "Good code reads like prose; great code fails loudly when types disagree.",
    "Never trust user input before `number()` or `trim()` checks it.",
];

pub const DOG_ROASTS: &[&str] = &[
    "That type mismatch just aged me three dog years.",
    "I've seen chewed-up Ethernet cables with cleaner syntax.",
    "Putting text inside math? Bold, artistic, completely illegal.",
    "Even my chew toy knows `let` means immutable.",
];

pub fn welcome_banner(personality: Personality, runs: u64) -> Option<String> {
    match personality {
        Personality::Silent => None,
        Personality::Professional => Some(format!("DOG Language Engine (sessions: {runs})")),
        Personality::Normal => Some(format!(
            "Welcome back to DOG! Tip: {}",
            DOG_TIPS[(runs as usize) % DOG_TIPS.len()]
        )),
        Personality::Sarcastic => Some(format!(
            "Oh, look who's coding again. Fact: {}",
            DOG_FACTS[(runs as usize) % DOG_FACTS.len()]
        )),
        Personality::Chaotic => Some(format!(
            "Unleashed mode active! Quote: \"{}\"",
            DOG_QUOTES[(runs as usize) % DOG_QUOTES.len()]
        )),
    }
}

pub fn achievement_for_runs(runs: u64) -> Option<&'static str> {
    match runs {
        1 => Some("Achievement Unlocked: First Bark (executed your first DOG run)"),
        10 => Some("Achievement Unlocked: Pack Leader (10 DOG runs completed)"),
        50 => Some("Achievement Unlocked: Alpha Compiler Whisperer (50 DOG runs)"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DogDiagnostic {
    pub code: &'static str,
    pub title: String,
    pub expected: Option<String>,
    pub received: Option<String>,
    pub span: Span,
    pub why: String,
    pub fix: String,
    pub tip: String,
    pub joke: String,
    pub stack_trace: Vec<String>,
}

impl DogDiagnostic {
    pub fn new(
        code: &'static str,
        title: impl Into<String>,
        span: Span,
        why: impl Into<String>,
        fix: impl Into<String>,
        tip: impl Into<String>,
        joke: impl Into<String>,
    ) -> Self {
        Self {
            code,
            title: title.into(),
            expected: None,
            received: None,
            span,
            why: why.into(),
            fix: fix.into(),
            tip: tip.into(),
            joke: joke.into(),
            stack_trace: Vec::new(),
        }
    }

    pub fn with_types(mut self, expected: impl Into<String>, received: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.received = Some(received.into());
        self
    }

    pub fn with_stack_trace(mut self, frames: Vec<String>) -> Self {
        self.stack_trace = frames;
        self
    }

    pub fn format_with(&self, personality: Personality, source: Option<&str>) -> String {
        let mut out = String::new();
        out.push_str(&format!("DOG ERROR {}\n\n", self.code));
        out.push_str(&format!("{}\n", self.title));
        if let (Some(exp), Some(rec)) = (&self.expected, &self.received) {
            out.push_str(&format!("\nExpected: {exp}\nReceived: {rec}\n"));
        }
        out.push_str(&format!("\nLocation: {}\n", self.span));

        if let Some(src) = source {
            if let Some(line_text) = src.lines().nth(self.span.line.saturating_sub(1)) {
                out.push_str(&format!("\n  {:>4} | {}\n", self.span.line, line_text));
                let caret_pad = " ".repeat(self.span.column.saturating_sub(1));
                out.push_str(&format!("       | {}^\n", caret_pad));
            }
        }

        if !self.stack_trace.is_empty() {
            out.push_str("\nSTACK TRACE:\n");
            for (idx, frame) in self.stack_trace.iter().rev().enumerate() {
                out.push_str(&format!("  {}: {frame}\n", idx + 1));
            }
        }

        out.push_str(&format!("\nWHY:\n{}\n", self.why));
        out.push_str(&format!("\nFIX:\n{}\n", self.fix));

        if personality != Personality::Silent {
            out.push_str(&format!("\nTIP:\n{}\n", self.tip));
        }

        let remark = match personality {
            Personality::Silent | Personality::Professional => None,
            Personality::Normal | Personality::Sarcastic => Some(self.joke.as_str()),
            Personality::Chaotic => {
                let idx = self.span.line % DOG_ROASTS.len();
                Some(DOG_ROASTS[idx])
            }
        };

        if let Some(line) = remark {
            out.push_str(&format!("\nDOG:\n\"{}\"\n", line));
        }

        out
    }
}

impl fmt::Display for DogDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_with(Personality::Sarcastic, None))
    }
}

pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    if a_chars.is_empty() {
        return b_chars.len();
    }
    if b_chars.is_empty() {
        return a_chars.len();
    }
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr = vec![0usize; b_chars.len() + 1];
    for (i, &ca) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

pub fn suggest_similar<'a>(
    target: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    if target.is_empty() {
        return None;
    }
    let target_lower = target.to_ascii_lowercase();
    let max_dist = (target_lower.chars().count() / 3).max(2).min(4);
    let mut best: Option<(usize, &'a str)> = None;
    for cand in candidates {
        if cand.is_empty() || cand == target {
            continue;
        }
        let cand_lower = cand.to_ascii_lowercase();
        let dist = levenshtein_distance(&target_lower, &cand_lower);
        if dist <= max_dist {
            match best {
                Some((best_dist, _)) if dist >= best_dist => {}
                _ => best = Some((dist, cand)),
            }
        }
    }
    best.map(|(_, s)| s)
}
