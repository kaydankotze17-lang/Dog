use crate::ast::Span;
use crate::diagnostics::DogDiagnostic;

#[derive(Debug, Clone, PartialEq)]
pub enum InterpLexPart {
    Literal(String),
    Expr { source: String, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Let,
    Var,
    Const,
    If,
    Otherwise,
    End,
    Or,
    And,
    Not,
    True,
    False,
    Fn,
    Return,
    While,
    Repeat,
    For,
    In,
    Break,
    Continue,
    Struct,
    Enum,
    Match,
    Case,
    Try,
    Catch,
    Use,

    // Literals & Identifiers
    Identifier(String),
    StringLit(String),
    InterpString(Vec<InterpLexPart>),
    NumberLit(i64),
    DecimalLit(f64),

    // Operators & Punctuation
    Assign,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    EqEq,
    BangEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Arrow,
    Colon,
    Dot,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,

    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub struct Lexer {
    file: String,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(file: impl Into<String>, source: &str) -> Self {
        Self {
            file: file.into(),
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, DogDiagnostic> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token {
                        kind: TokenKind::Newline,
                        span,
                    });
                }
                '#' => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                '/' if self.peek_next() == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                '$' if self.peek_next() == Some('"') => {
                    tokens.push(self.read_interp_string()?);
                }
                '"' => {
                    if self.chars.get(self.pos..self.pos + 3) == Some(&['"', '"', '"']) {
                        tokens.push(self.read_triple_string()?);
                    } else {
                        tokens.push(self.read_string()?);
                    }
                }
                '0'..='9' => {
                    tokens.push(self.read_number()?);
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    tokens.push(self.read_ident_or_keyword());
                }
                '=' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token {
                            kind: TokenKind::EqEq,
                            span,
                        });
                    } else {
                        tokens.push(Token {
                            kind: TokenKind::Assign,
                            span,
                        });
                    }
                }
                '!' if self.peek_next() == Some('=') => {
                    let span = self.span();
                    self.advance();
                    self.advance();
                    tokens.push(Token {
                        kind: TokenKind::BangEq,
                        span,
                    });
                }
                '>' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token {
                            kind: TokenKind::GtEq,
                            span,
                        });
                    } else {
                        tokens.push(Token {
                            kind: TokenKind::Gt,
                            span,
                        });
                    }
                }
                '<' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token {
                            kind: TokenKind::LtEq,
                            span,
                        });
                    } else {
                        tokens.push(Token {
                            kind: TokenKind::Lt,
                            span,
                        });
                    }
                }
                '+' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::PlusEq, span });
                    } else {
                        tokens.push(Token { kind: TokenKind::Plus, span });
                    }
                }
                '-' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::Arrow, span });
                    } else if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::MinusEq, span });
                    } else {
                        tokens.push(Token { kind: TokenKind::Minus, span });
                    }
                }
                '*' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::StarEq, span });
                    } else {
                        tokens.push(Token { kind: TokenKind::Star, span });
                    }
                }
                '/' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::SlashEq, span });
                    } else {
                        tokens.push(Token { kind: TokenKind::Slash, span });
                    }
                }
                '%' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::PercentEq, span });
                    } else {
                        tokens.push(Token { kind: TokenKind::Percent, span });
                    }
                }
                ':' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Colon, span });
                }
                '.' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Dot, span });
                }
                '(' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::LParen, span });
                }
                ')' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::RParen, span });
                }
                '[' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::LBracket, span });
                }
                ']' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::RBracket, span });
                }
                '{' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::LBrace, span });
                }
                '}' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::RBrace, span });
                }
                ',' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Comma, span });
                }
                other => {
                    let span = self.span();
                    return Err(DogDiagnostic::new(
                        "E001",
                        format!("Unexpected character '{other}'."),
                        span,
                        "The lexer encountered a character that is not valid DOG syntax.",
                        "Remove the unexpected character or wrap text inside double quotes.",
                        "DOG likes clean syntax, not stray punctuation.",
                        "Did your keyboard sneeze?",
                    ));
                }
            }
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.span(),
        });
        Ok(tokens)
    }

    fn span(&self) -> Span {
        Span::new(&self.file, self.line, self.column)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn read_string(&mut self) -> Result<Token, DogDiagnostic> {
        let span = self.span();
        self.advance();
        let mut out = String::new();

        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Ok(Token {
                    kind: TokenKind::StringLit(out),
                    span,
                });
            }
            if ch == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(esc) => out.push(esc),
                    None => {
                        return Err(DogDiagnostic::new(
                            "E002",
                            "Unterminated string literal.",
                            span,
                            "Reached end of file while reading an escape sequence inside quotes.",
                            "Add a closing '\"' before the end of the line.",
                            "Every opening quote needs a matching closing quote.",
                            "Strings can't stay open forever.",
                        ));
                    }
                }
                continue;
            }
            if ch == '\n' {
                return Err(DogDiagnostic::new(
                    "E002",
                    "Unterminated string literal.",
                    span,
                    "A string literal was opened with '\"' but not closed before the end of the line.",
                    "Close the string with '\"'.",
                    "Check for missing quotes at the end of say().",
                    "You started talking and forgot to stop.",
                ));
            }
            out.push(ch);
            self.advance();
        }

        Err(DogDiagnostic::new(
            "E002",
            "Unterminated string literal.",
            span,
            "Reached end of file before finding a closing '\"'.",
            "Add a closing '\"' to finish the text value.",
            "Match every opening quote with a closing quote.",
            "Left hanging at the end of the file.",
        ))
    }

    fn read_triple_string(&mut self) -> Result<Token, DogDiagnostic> {
        let span = self.span();
        self.advance();
        self.advance();
        self.advance();
        if self.peek() == Some('\r') && self.peek_next() == Some('\n') {
            self.advance();
            self.advance();
        } else if self.peek() == Some('\n') {
            self.advance();
        }

        let mut out = String::new();
        while self.pos < self.chars.len() {
            if self.chars.get(self.pos..self.pos + 3) == Some(&['"', '"', '"']) {
                self.advance();
                self.advance();
                self.advance();
                return Ok(Token {
                    kind: TokenKind::StringLit(out),
                    span,
                });
            }
            let ch = self.chars[self.pos];
            if ch == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(esc) => out.push(esc),
                    None => break,
                }
            } else {
                out.push(ch);
                self.advance();
            }
        }

        Err(DogDiagnostic::new(
            "E002",
            "Unterminated triple-quoted string literal.",
            span,
            "Reached end of file before finding a closing '\"\"\"'.",
            "Add closing '\"\"\"' to terminate the multi-line text literal.",
            "Every opening '\"\"\"' block must end with '\"\"\"'.",
            "Multi-line strings still need a closing triple quote.",
        ))
    }

    fn read_interp_string(&mut self) -> Result<Token, DogDiagnostic> {
        let span = self.span();
        self.advance(); // $
        self.advance(); // "

        let mut parts: Vec<InterpLexPart> = Vec::new();
        let mut lit = String::new();

        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                if !lit.is_empty() || parts.is_empty() {
                    parts.push(InterpLexPart::Literal(lit));
                }
                return Ok(Token {
                    kind: TokenKind::InterpString(parts),
                    span,
                });
            }
            if ch == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => lit.push('\n'),
                    Some('t') => lit.push('\t'),
                    Some('r') => lit.push('\r'),
                    Some('"') => lit.push('"'),
                    Some('\\') => lit.push('\\'),
                    Some('{') => lit.push('{'),
                    Some('}') => lit.push('}'),
                    Some(esc) => lit.push(esc),
                    None => {
                        return Err(DogDiagnostic::new(
                            "E002",
                            "Unterminated interpolated string literal.",
                            span,
                            "Reached end of file while reading an escape sequence inside $\"...\".",
                            "Add a closing '\"' before the end of the line.",
                            "Use `\\{` or `{{` for literal braces in $\"...\" strings.",
                            "Strings can't stay open forever.",
                        ));
                    }
                }
                continue;
            }
            if ch == '\n' {
                return Err(DogDiagnostic::new(
                    "E002",
                    "Unterminated interpolated string literal.",
                    span,
                    "An interpolated string opened with $\" was not closed before the end of the line.",
                    "Close the string with '\"'.",
                    "Every opening quote needs a matching closing quote.",
                    "Started talking and forgot to stop.",
                ));
            }
            if ch == '{' {
                if self.peek_next() == Some('{') {
                    self.advance();
                    self.advance();
                    lit.push('{');
                    continue;
                }
                if !lit.is_empty() {
                    parts.push(InterpLexPart::Literal(std::mem::take(&mut lit)));
                }
                self.advance(); // {
                let expr_span = self.span();
                let mut expr_src = String::new();
                let mut depth = 1usize;
                let mut in_inner_str = false;

                while let Some(ech) = self.peek() {
                    if in_inner_str {
                        expr_src.push(ech);
                        self.advance();
                        if ech == '\\' {
                            if let Some(esc) = self.peek() {
                                expr_src.push(esc);
                                self.advance();
                            }
                        } else if ech == '"' {
                            in_inner_str = false;
                        }
                        continue;
                    }
                    if ech == '\n' {
                        return Err(DogDiagnostic::new(
                            "E002",
                            "Unclosed '{' inside interpolated string.",
                            expr_span,
                            "Interpolation placeholder `{...}` was not closed with '}' before end of line.",
                            "Add '}' to close the interpolated expression.",
                            "Use `{{` if you meant a literal '{'.",
                            "Braces inside strings still need partners.",
                        ));
                    }
                    if ech == '"' {
                        in_inner_str = true;
                        expr_src.push(ech);
                        self.advance();
                        continue;
                    }
                    if ech == '{' {
                        depth += 1;
                        expr_src.push(ech);
                        self.advance();
                        continue;
                    }
                    if ech == '}' {
                        depth -= 1;
                        self.advance();
                        if depth == 0 {
                            break;
                        }
                        expr_src.push('}');
                        continue;
                    }
                    expr_src.push(ech);
                    self.advance();
                }

                if depth != 0 {
                    return Err(DogDiagnostic::new(
                        "E002",
                        "Unclosed '{' inside interpolated string.",
                        expr_span,
                        "Reached end of file before closing '}' in interpolated expression.",
                        "Add '}' and close the string with '\"'.",
                        "Match every '{' with '}' inside $\"...\" strings.",
                        "Braces left hanging.",
                    ));
                }

                let trimmed_expr = expr_src.trim().to_string();
                if trimmed_expr.is_empty() {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Empty interpolation expression '{}' in $\"...\".",
                        expr_span,
                        "An interpolation placeholder must contain an expression.",
                        "Write an expression inside `{...}`, like `$\"Hi {name}\"`, or escape braces with `{{}}`.",
                        "DOG interpolates values with `$\"Hello {expr}\"`.",
                        "Empty braces have nothing to say.",
                    ));
                }

                parts.push(InterpLexPart::Expr {
                    source: trimmed_expr,
                    span: expr_span,
                });
                continue;
            }
            if ch == '}' && self.peek_next() == Some('}') {
                self.advance();
                self.advance();
                lit.push('}');
                continue;
            }

            lit.push(ch);
            self.advance();
        }

        Err(DogDiagnostic::new(
            "E002",
            "Unterminated interpolated string literal.",
            span,
            "Reached end of file before finding a closing '\"'.",
            "Add a closing '\"' to finish the interpolated string.",
            "Match every opening quote with a closing quote.",
            "Left hanging at the end of the file.",
        ))
    }

    fn read_number(&mut self) -> Result<Token, DogDiagnostic> {
        let span = self.span();
        if self.peek() == Some('0') && matches!(self.peek_next(), Some('x' | 'X' | 'b' | 'B')) {
            self.advance(); // '0'
            let base_ch = self.advance().unwrap_or('x');
            let radix = if matches!(base_ch, 'x' | 'X') { 16 } else { 2 };
            let mut raw = String::new();
            while let Some(ch) = self.peek() {
                if ch == '_'
                    || (radix == 16 && ch.is_ascii_hexdigit())
                    || (radix == 2 && matches!(ch, '0' | '1'))
                {
                    raw.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }
            let clean: String = raw.chars().filter(|c| *c != '_').collect();
            if clean.is_empty() || raw.starts_with('_') || raw.ends_with('_') {
                return Err(DogDiagnostic::new(
                    "E003",
                    format!("Invalid base-{radix} integer literal '0{base_ch}{raw}'."),
                    span,
                    "Hex (0x) and binary (0b) literals require at least one valid digit and cannot start or end with '_'.",
                    "Write e.g. `0xFF_00` or `0b1010_0101`.",
                    "Check prefix digits for 0x or 0b.",
                    "Even hex bowls need real digits.",
                ));
            }
            let value = i64::from_str_radix(&clean, radix).map_err(|_| {
                DogDiagnostic::new(
                    "E003",
                    format!("Integer literal '0{base_ch}{raw}' out of 64-bit signed range."),
                    span.clone(),
                    "The integer literal exceeds signed 64-bit limits.",
                    "Keep hex/binary values within signed 64-bit bounds.",
                    "Use smaller constants or split fields.",
                    "Too big for one number bowl.",
                )
            })?;
            return Ok(Token {
                kind: TokenKind::NumberLit(value),
                span,
            });
        }

        let mut raw = String::new();
        let mut is_decimal = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                raw.push(ch);
                self.advance();
            } else if ch == '.' && !is_decimal && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
                is_decimal = true;
                raw.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if raw.ends_with('_') || raw.contains("_.") || raw.contains("._") {
            return Err(DogDiagnostic::new(
                "E003",
                format!("Invalid numeric separator placement in '{raw}'."),
                span,
                "Underscores in number literals must appear strictly between digits.",
                "Write e.g. `1_000_000` or `3.14_15`.",
                "Remove trailing underscores or underscores next to '.'.",
                "Keep underscores cozy between digits.",
            ));
        }

        let clean: String = raw.chars().filter(|c| *c != '_').collect();
        if is_decimal {
            let value = clean.parse::<f64>().map_err(|_| {
                DogDiagnostic::new(
                    "E003",
                    format!("Invalid decimal literal '{raw}'."),
                    span.clone(),
                    "The decimal number could not be parsed as a 64-bit floating-point number.",
                    "Use standard digits with a single decimal point, like 3.14.",
                    "Check for extra digits or overflow.",
                    "Even calculators have limits.",
                )
            })?;
            Ok(Token {
                kind: TokenKind::DecimalLit(value),
                span,
            })
        } else {
            let value = clean.parse::<i64>().map_err(|_| {
                DogDiagnostic::new(
                    "E003",
                    format!("Invalid number literal '{raw}'."),
                    span.clone(),
                    "The integer literal exceeds the 64-bit signed range.",
                    "Use an integer between -9223372036854775808 and 9223372036854775807.",
                    "Use decimal() if you need floating-point scale.",
                    "That number is too big for one bowl.",
                )
            })?;
            Ok(Token {
                kind: TokenKind::NumberLit(value),
                span,
            })
        }
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let span = self.span();
        let mut raw = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                raw.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match raw.as_str() {
            "let" => TokenKind::Let,
            "var" => TokenKind::Var,
            "const" => TokenKind::Const,
            "if" => TokenKind::If,
            "otherwise" => TokenKind::Otherwise,
            "end" => TokenKind::End,
            "or" => TokenKind::Or,
            "and" => TokenKind::And,
            "not" => TokenKind::Not,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "while" => TokenKind::While,
            "repeat" => TokenKind::Repeat,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "case" => TokenKind::Case,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "use" => TokenKind::Use,
            _ => TokenKind::Identifier(raw),
        };

        Token { kind, span }
    }
}
