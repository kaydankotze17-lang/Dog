use crate::ast::Span;
use crate::config::Personality;
use std::fmt;

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
        }
    }

    pub fn with_types(mut self, expected: impl Into<String>, received: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.received = Some(received.into());
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

        out.push_str(&format!("\nWHY:\n{}\n", self.why));
        out.push_str(&format!("\nFIX:\n{}\n", self.fix));

        if personality != Personality::Silent {
            out.push_str(&format!("\nTIP:\n{}\n", self.tip));
        }

        let remark = match personality {
            Personality::Silent | Personality::Professional => None,
            Personality::Normal => Some(self.joke.as_str()),
            Personality::Sarcastic => Some(self.joke.as_str()),
            Personality::Chaotic => Some("Even my chew toy parses better than this."),
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
