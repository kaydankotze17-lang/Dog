use crate::ast::Span;
use crate::diagnostics::DogDiagnostic;

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

    // Literals & Identifiers
    Identifier(String),
    StringLit(String),
    NumberLit(i64),
    DecimalLit(f64),

    // Operators & Punctuation
    Assign,
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
    LParen,
    RParen,
    LBracket,
    RBracket,
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
                '"' => {
                    tokens.push(self.read_string()?);
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
                    tokens.push(Token { kind: TokenKind::Plus, span });
                }
                '-' => {
                    let span = self.span();
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        tokens.push(Token { kind: TokenKind::Arrow, span });
                    } else {
                        tokens.push(Token { kind: TokenKind::Minus, span });
                    }
                }
                '*' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Star, span });
                }
                '/' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Slash, span });
                }
                '%' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Percent, span });
                }
                ':' => {
                    let span = self.span();
                    self.advance();
                    tokens.push(Token { kind: TokenKind::Colon, span });
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

    fn read_number(&mut self) -> Result<Token, DogDiagnostic> {
        let span = self.span();
        let mut raw = String::new();
        let mut is_decimal = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
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

        if is_decimal {
            let value = raw.parse::<f64>().map_err(|_| {
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
            let value = raw.parse::<i64>().map_err(|_| {
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
            _ => TokenKind::Identifier(raw),
        };

        Token { kind, span }
    }
}
