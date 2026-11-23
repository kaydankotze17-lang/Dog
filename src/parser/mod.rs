use crate::ast::{
    BinOp, BindingKind, Expr, InterpPart, MatchCase, Param, Program, Stmt, StructField,
    StyleRuleDecl, StyleVariantBlock, TypeRef, UnaryOp,
};
use crate::diagnostics::DogDiagnostic;
use crate::lexer::{InterpLexPart, Lexer, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(mut self) -> Result<Program, DogDiagnostic> {
        let mut statements = Vec::new();
        self.skip_newlines();

        while !self.is_at_end() {
            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }

        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Stmt, DogDiagnostic> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Use => self.parse_use(),
            TokenKind::Struct => self.parse_struct(),
            TokenKind::Enum => self.parse_enum(),
            TokenKind::Let => self.parse_binding(BindingKind::Let),
            TokenKind::Var => self.parse_binding(BindingKind::Var),
            TokenKind::Const => self.parse_binding(BindingKind::Const),
            TokenKind::If => self.parse_if(),
            TokenKind::Unless => self.parse_unless(),
            TokenKind::Match => self.parse_match(),
            TokenKind::Try => self.parse_try(),
            TokenKind::While => self.parse_while(),
            TokenKind::Until => self.parse_until(),
            TokenKind::Repeat => self.parse_repeat(),
            TokenKind::For => self.parse_for(),
            TokenKind::Fn | TokenKind::Define | TokenKind::Function => self.parse_fn(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => {
                let span = self.advance().span.clone();
                self.expect_end_of_statement()?;
                Ok(Stmt::Break { span })
            }
            TokenKind::Continue => {
                let span = self.advance().span.clone();
                self.expect_end_of_statement()?;
                Ok(Stmt::Continue { span })
            }
            TokenKind::Identifier(kw)
                if kw == "style"
                    && matches!(
                        self.peek_next_kind(),
                        Some(
                            TokenKind::StringLit(_)
                                | TokenKind::InterpString(_)
                                | TokenKind::Identifier(_)
                        )
                    ) =>
            {
                self.parse_style()
            }
            TokenKind::Identifier(kw)
                if kw == "style"
                    && matches!(
                        self.peek_next_kind(),
                        Some(
                            TokenKind::StringLit(_)
                                | TokenKind::InterpString(_)
                                | TokenKind::Identifier(_)
                        )
                    ) =>
            {
                self.parse_style()
            }
            TokenKind::Identifier(kw)
                if kw == "set" && matches!(self.peek_next_kind(), Some(TokenKind::Identifier(_))) =>
            {
                self.parse_english_set()
            }
            TokenKind::Identifier(kw)
                if matches!(kw.as_str(), "increase" | "decrease")
                    && matches!(self.peek_next_kind(), Some(TokenKind::Identifier(_))) =>
            {
                self.parse_english_delta()
            }
            TokenKind::Identifier(name) if self.peek_next_kind() == Some(&TokenKind::Assign) => {
                let name = name.clone();
                let span = tok.span.clone();
                self.advance(); // ident
                self.advance(); // =
                let value = self.parse_expression()?;
                self.expect_end_of_statement()?;
                Ok(Stmt::Assign { name, value, span })
            }
            TokenKind::Identifier(name)
                if matches!(
                    self.peek_next_kind(),
                    Some(
                        TokenKind::PlusEq
                            | TokenKind::MinusEq
                            | TokenKind::StarEq
                            | TokenKind::SlashEq
                            | TokenKind::PercentEq
                    )
                ) =>
            {
                let name = name.clone();
                let span = tok.span.clone();
                self.advance(); // ident
                let op_tok = self.advance().clone();
                let op = match op_tok.kind {
                    TokenKind::PlusEq => BinOp::Add,
                    TokenKind::MinusEq => BinOp::Sub,
                    TokenKind::StarEq => BinOp::Mul,
                    TokenKind::SlashEq => BinOp::Div,
                    TokenKind::PercentEq => BinOp::Mod,
                    _ => unreachable!(),
                };
                let value = self.parse_expression()?;
                self.expect_end_of_statement()?;
                Ok(Stmt::CompoundAssign {
                    name,
                    op,
                    value,
                    span,
                })
            }
            _ => {
                let span = tok.span.clone();
                let expr = self.parse_expression()?;
                if self.peek().kind == TokenKind::Assign {
                    match expr {
                        Expr::Index { target, index, .. } => {
                            if let Expr::Identifier { name, .. } = *target {
                                self.advance(); // =
                                let value = self.parse_expression()?;
                                self.expect_end_of_statement()?;
                                return Ok(Stmt::IndexAssign {
                                    target: name,
                                    index: *index,
                                    value,
                                    span,
                                });
                            }
                        }
                        Expr::FieldAccess { target, field, .. } => {
                            if let Expr::Identifier { name, .. } = *target {
                                self.advance(); // =
                                let value = self.parse_expression()?;
                                self.expect_end_of_statement()?;
                                return Ok(Stmt::FieldAssign {
                                    target: name,
                                    field,
                                    value,
                                    span,
                                });
                            }
                        }
                        _ => {}
                    }
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Invalid assignment target.",
                        self.peek().span.clone(),
                        "Only variables, struct fields (`obj.field`), or indexed lists (`items[0]`) can appear on the left side of '='.",
                        "Assign to a variable name like `x = 10`, `u.age = 22`, or `items[0] = 10`.",
                        "Use `==` when checking equality.",
                        "You can't assign a value to thin air.",
                    ));
                }
                self.expect_end_of_statement()?;
                Ok(Stmt::ExprStmt { expr, span })
            }
        }
    }

    fn parse_use(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let path = match &self.peek().kind {
            TokenKind::StringLit(s) => s.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected module path string after 'use'.",
                    self.peek().span.clone(),
                    "Local imports must specify a quoted `.dg` path.",
                    "Write e.g. `use \"helpers.dg\"`.",
                    "Paths are resolved relative to the importing file.",
                    "Use what? Give me a filename in quotes.",
                ));
            }
        };
        self.advance();
        self.expect_end_of_statement()?;
        Ok(Stmt::Use {
            path,
            span: kw_tok.span,
        })
    }

    fn parse_struct(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected struct name after 'struct'.",
                    self.peek().span.clone(),
                    "Struct definitions require a type name.",
                    "Write e.g. `struct User`.",
                    "Use PascalCase for struct names.",
                    "An unnamed blueprint builds nothing.",
                ));
            }
        };
        self.advance();
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::End {
            let f_span = self.peek().span.clone();
            let f_name = match &self.peek().kind {
                TokenKind::Identifier(n) => n.clone(),
                _ => {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected field name inside struct.",
                        f_span,
                        "Each line inside `struct ... end` defines `field: type`.",
                        "Example: `name: text`.",
                        "Close the struct with `end`.",
                        "Fields need names.",
                    ));
                }
            };
            self.advance();
            if self.peek().kind != TokenKind::Colon {
                return Err(DogDiagnostic::new(
                    "E004",
                    format!("Expected ':' after field '{f_name}'."),
                    self.peek().span.clone(),
                    "Struct fields require explicit type annotations.",
                    format!("Write `{f_name}: text` or another type."),
                    "Strict typing forbids untyped struct fields.",
                    "Every field needs a type collar.",
                ));
            }
            self.advance();
            let ty = self.parse_type_ref()?;
            self.expect_end_of_statement()?;
            self.skip_newlines();
            fields.push(StructField {
                name: f_name,
                ty,
                span: f_span,
            });
        }

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                format!("Missing 'end' for struct '{name}'."),
                kw_tok.span,
                "Every `struct` definition must be closed with `end`.",
                "Add `end` on its own line.",
                "Indentation is visual; `end` closes blocks.",
                "Left the blueprint unfinished.",
            ));
        }
        self.advance();
        self.expect_end_of_statement()?;

        Ok(Stmt::StructDef {
            name,
            fields,
            span: kw_tok.span,
        })
    }

    fn parse_enum(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected enum name after 'enum'.",
                    self.peek().span.clone(),
                    "Enum definitions require a type name.",
                    "Write e.g. `enum Role`.",
                    "Use PascalCase for enum names.",
                    "An enum without a name?",
                ));
            }
        };
        self.advance();
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut variants = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::End {
            let v_span = self.peek().span.clone();
            let v_name = match &self.peek().kind {
                TokenKind::Identifier(n) => n.clone(),
                _ => {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected variant name inside enum.",
                        v_span,
                        "Each line inside `enum ... end` lists a variant identifier.",
                        "Example: `Admin`.",
                        "Close the enum with `end`.",
                        "Variants must be valid identifiers.",
                    ));
                }
            };
            self.advance();
            self.expect_end_of_statement()?;
            self.skip_newlines();
            variants.push(v_name);
        }

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                format!("Missing 'end' for enum '{name}'."),
                kw_tok.span,
                "Every `enum` definition must be closed with `end`.",
                "Add `end` on its own line.",
                "Check matching `end` keywords.",
                "Enum left hanging.",
            ));
        }
        self.advance();
        self.expect_end_of_statement()?;

        Ok(Stmt::EnumDef {
            name,
            variants,
            span: kw_tok.span,
        })
    }

    fn parse_match(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let subject = self.parse_expression()?;
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut cases = Vec::new();
        while self.peek().kind == TokenKind::Case {
            let case_tok = self.advance().clone();
            let pattern = self.parse_expression()?;
            self.expect_end_of_statement()?;
            self.skip_newlines();
            let mut body = Vec::new();
            while !self.is_at_end()
                && self.peek().kind != TokenKind::Case
                && self.peek().kind != TokenKind::Otherwise
                && self.peek().kind != TokenKind::Else
                && self.peek().kind != TokenKind::End
            {
                body.push(self.parse_statement()?);
                self.skip_newlines();
            }
            cases.push(MatchCase {
                pattern,
                body,
                span: case_tok.span,
            });
        }

        let otherwise_branch =
            if matches!(self.peek().kind, TokenKind::Otherwise | TokenKind::Else) {
                self.advance();
                self.expect_end_of_statement()?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.is_at_end() && self.peek().kind != TokenKind::End {
                    body.push(self.parse_statement()?);
                    self.skip_newlines();
                }
                Some(body)
            } else {
                None
            };

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                "Missing 'end' for 'match' block.",
                kw_tok.span,
                "Every `match` block must end with `end`.",
                "Add `end` after the final `case` or `otherwise` branch.",
                "Use `case <value>` and optional `otherwise` inside `match`.",
                "Matched everything except the closing `end`.",
            ));
        }
        self.advance();
        self.expect_end_of_statement()?;

        Ok(Stmt::Match {
            subject,
            cases,
            otherwise_branch,
            span: kw_tok.span,
        })
    }

    fn parse_try(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut try_body = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::Catch {
            try_body.push(self.parse_statement()?);
            self.skip_newlines();
        }

        if self.peek().kind != TokenKind::Catch {
            return Err(DogDiagnostic::new(
                "E005",
                "Expected 'catch <err>' after 'try' block.",
                kw_tok.span,
                "Every `try` block must be paired with `catch err ... end`.",
                "Add `catch err` to handle runtime errors.",
                "Caught errors bind to `err` as `text`.",
                "You tried, but forgot to catch.",
            ));
        }
        self.advance(); // catch

        let err_name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected error variable name after 'catch'.",
                    self.peek().span.clone(),
                    "Specify a variable name to bind the error message, e.g. `catch err`.",
                    "Write `catch err`.",
                    "The error variable has type `text`.",
                    "Catch into what variable?",
                ));
            }
        };
        self.advance();
        self.expect_end_of_statement()?;
        let catch_body = self.parse_block_until_end(&kw_tok, "try")?;

        Ok(Stmt::TryCatch {
            try_body,
            err_name,
            catch_body,
            span: kw_tok.span,
        })
    }

    fn parse_binding(&mut self, kind: BindingKind) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected variable name after binding keyword.",
                    self.peek().span.clone(),
                    "Variable declarations require an identifier name before '='.",
                    "Write e.g. `let username = \"DOG\"`.",
                    "Pick a clear identifier using letters, digits, or underscores.",
                    "Unnamed variables are just mysteries.",
                ));
            }
        };
        self.advance(); // ident

        let type_ann = if self.peek().kind == TokenKind::Colon {
            self.advance(); // :
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        if self.peek().kind != TokenKind::Assign {
            return Err(DogDiagnostic::new(
                "E004",
                format!("Expected '=' after '{name}'."),
                self.peek().span.clone(),
                "DOG requires variables to be initialized with '=' at declaration.",
                format!("Provide an initial value: `{name} = ...`."),
                "Strict typing requires every variable to start with a known value.",
                "An empty bowl doesn't feed anybody.",
            ));
        }
        self.advance(); // =

        let value = self.parse_expression()?;
        self.expect_end_of_statement()?;

        Ok(Stmt::Binding {
            kind,
            name,
            type_ann,
            value,
            span: kw_tok.span,
        })
    }

    fn parse_english_set(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone(); // set
        let name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => unreachable!(),
        };
        self.advance();

        enum SetTarget {
            Var(String),
            Field(String, String),
            Index(String, Expr),
        }

        let target = if self.peek().kind == TokenKind::Dot {
            self.advance();
            let field = match &self.peek().kind {
                TokenKind::Identifier(f) => f.clone(),
                _ => {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected field name after '.' in 'set' statement.",
                        self.peek().span.clone(),
                        "Property assignment requires a field identifier.",
                        format!("Write e.g. `set {name}.field to value`."),
                        "Example: `set user.name to \"DOG\"`.",
                        "Set which field?",
                    ));
                }
            };
            self.advance();
            SetTarget::Field(name, field)
        } else if self.peek().kind == TokenKind::LBracket {
            self.advance();
            let index = self.parse_expression()?;
            if self.peek().kind != TokenKind::RBracket {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected ']' after index in 'set' statement.",
                    self.peek().span.clone(),
                    "Indexed assignment requires a closing ']'.",
                    format!("Write e.g. `set {name}[0] to value`."),
                    "Balance list brackets.",
                    "Unclosed list index.",
                ));
            }
            self.advance();
            SetTarget::Index(name, index)
        } else {
            SetTarget::Var(name)
        };

        if self.peek().kind == TokenKind::Assign || self.peek_ident_at(0) == Some("to") {
            self.advance();
        } else {
            return Err(DogDiagnostic::new(
                "E004",
                "Expected 'to' or '=' in 'set' statement.",
                self.peek().span.clone(),
                "English assignment follows the form `set variable to value`.",
                "Write e.g. `set count to 10`.",
                "Remember to declare new variables first with `var count = 0` or `let`.",
                "Set it to what?",
            ));
        }

        let value = self.parse_expression()?;
        self.expect_end_of_statement()?;

        Ok(match target {
            SetTarget::Var(name) => Stmt::Assign {
                name,
                value,
                span: kw_tok.span,
            },
            SetTarget::Field(target, field) => Stmt::FieldAssign {
                target,
                field,
                value,
                span: kw_tok.span,
            },
            SetTarget::Index(target, index) => Stmt::IndexAssign {
                target,
                index,
                value,
                span: kw_tok.span,
            },
        })
    }

    fn parse_english_delta(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let verb = match &kw_tok.kind {
            TokenKind::Identifier(kw) if kw == "increase" => "increase",
            _ => "decrease",
        };
        let op = if verb == "increase" {
            BinOp::Add
        } else {
            BinOp::Sub
        };
        let name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => unreachable!(),
        };
        let target_span = self.advance().span.clone();

        enum DeltaTarget {
            Var(String),
            Field(String, String),
            Index(String, Expr),
        }

        let target = if self.peek().kind == TokenKind::Dot {
            self.advance();
            let field = match &self.peek().kind {
                TokenKind::Identifier(f) => f.clone(),
                _ => {
                    return Err(DogDiagnostic::new(
                        "E004",
                        format!("Expected field name after '.' in '{verb}' statement."),
                        self.peek().span.clone(),
                        "Property update requires a field identifier.",
                        format!("Write e.g. `{verb} {name}.field by 1`."),
                        format!("Example: `{verb} player.score by 10`."),
                        "Update which field?",
                    ));
                }
            };
            self.advance();
            DeltaTarget::Field(name, field)
        } else if self.peek().kind == TokenKind::LBracket {
            self.advance();
            let index = self.parse_expression()?;
            if self.peek().kind != TokenKind::RBracket {
                return Err(DogDiagnostic::new(
                    "E004",
                    format!("Expected ']' after index in '{verb}' statement."),
                    self.peek().span.clone(),
                    "Indexed update requires a closing ']'.",
                    format!("Write e.g. `{verb} {name}[0] by 1`."),
                    "Balance list brackets.",
                    "Unclosed list index.",
                ));
            }
            self.advance();
            DeltaTarget::Index(name, index)
        } else {
            DeltaTarget::Var(name)
        };

        let target_label = match &target {
            DeltaTarget::Var(n) => n.clone(),
            DeltaTarget::Field(t, f) => format!("{t}.{f}"),
            DeltaTarget::Index(t, _) => format!("{t}[i]"),
        };

        if self.peek_ident_at(0) == Some("by") {
            self.advance();
        } else {
            return Err(DogDiagnostic::new(
                "E004",
                format!("Expected 'by' after '{target_label}' in '{verb}' statement."),
                self.peek().span.clone(),
                format!("English updates follow the form `{verb} variable by amount`."),
                format!("Write e.g. `{verb} {target_label} by 1`."),
                "Only mutable `var` bindings can be updated.",
                "Change it by how much?",
            ));
        }

        let value = self.parse_expression()?;
        self.expect_end_of_statement()?;

        Ok(match target {
            DeltaTarget::Var(name) => Stmt::CompoundAssign {
                name,
                op,
                value,
                span: kw_tok.span,
            },
            DeltaTarget::Field(target, field) => Stmt::FieldAssign {
                target: target.clone(),
                field: field.clone(),
                value: Expr::Binary {
                    left: Box::new(Expr::FieldAccess {
                        target: Box::new(Expr::Identifier {
                            name: target,
                            span: target_span.clone(),
                        }),
                        field,
                        span: target_span,
                    }),
                    op,
                    right: Box::new(value),
                    span: kw_tok.span.clone(),
                },
                span: kw_tok.span,
            },
            DeltaTarget::Index(target, index) => Stmt::IndexAssign {
                target: target.clone(),
                index: index.clone(),
                value: Expr::Binary {
                    left: Box::new(Expr::Index {
                        target: Box::new(Expr::Identifier {
                            name: target,
                            span: target_span.clone(),
                        }),
                        index: Box::new(index),
                        span: target_span,
                    }),
                    op,
                    right: Box::new(value),
                    span: kw_tok.span.clone(),
                },
                span: kw_tok.span,
            },
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, DogDiagnostic> {
        let if_tok = self.advance().clone();
        let condition = self.parse_expression()?;
        if self.peek_ident_at(0) == Some("then") {
            self.advance();
        }
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut then_branch = Vec::new();
        while !self.is_at_end()
            && !matches!(
                self.peek().kind,
                TokenKind::Otherwise | TokenKind::Else | TokenKind::Elif | TokenKind::End
            )
        {
            then_branch.push(self.parse_statement()?);
            self.skip_newlines();
        }

        let otherwise_branch = if self.peek().kind == TokenKind::Elif {
            let nested_if = self.parse_if()?;
            return Ok(Stmt::If {
                condition,
                then_branch,
                otherwise_branch: Some(vec![nested_if]),
                span: if_tok.span,
            });
        } else if matches!(self.peek().kind, TokenKind::Otherwise | TokenKind::Else) {
            self.advance(); // otherwise | else
            if self.peek().kind == TokenKind::If {
                let nested_if = self.parse_if()?;
                return Ok(Stmt::If {
                    condition,
                    then_branch,
                    otherwise_branch: Some(vec![nested_if]),
                    span: if_tok.span,
                });
            }
            self.expect_end_of_statement()?;
            self.skip_newlines();
            let mut else_stmts = Vec::new();
            while !self.is_at_end() && self.peek().kind != TokenKind::End {
                else_stmts.push(self.parse_statement()?);
                self.skip_newlines();
            }
            Some(else_stmts)
        } else {
            None
        };

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                "Missing 'end' keyword for 'if' block.",
                if_tok.span,
                "DOG uses 'end' (not indentation) to close every 'if', 'else', or 'otherwise' block.",
                "Add `end` on its own line after the block body.",
                "Indentation is visual; `end` closes the block.",
                "You opened a door and walked away without closing it.",
            ));
        }
        self.advance(); // end
        self.expect_end_of_statement()?;

        Ok(Stmt::If {
            condition,
            then_branch,
            otherwise_branch,
            span: if_tok.span,
        })
    }

    fn parse_unless(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let raw_cond = self.parse_expression()?;
        if self.peek_ident_at(0) == Some("then") {
            self.advance();
        }
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut then_branch = Vec::new();
        while !self.is_at_end()
            && !matches!(
                self.peek().kind,
                TokenKind::Otherwise | TokenKind::Else | TokenKind::End
            )
        {
            then_branch.push(self.parse_statement()?);
            self.skip_newlines();
        }

        let otherwise_branch =
            if matches!(self.peek().kind, TokenKind::Otherwise | TokenKind::Else) {
                self.advance();
                self.expect_end_of_statement()?;
                self.skip_newlines();
                let mut else_stmts = Vec::new();
                while !self.is_at_end() && self.peek().kind != TokenKind::End {
                    else_stmts.push(self.parse_statement()?);
                    self.skip_newlines();
                }
                Some(else_stmts)
            } else {
                None
            };

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                "Missing 'end' keyword for 'unless' block.",
                kw_tok.span,
                "DOG uses 'end' to close every 'unless' block.",
                "Add `end` on its own line after the block body.",
                "`unless cond` runs the block when `cond` is false.",
                "Left an 'unless' block open.",
            ));
        }
        self.advance();
        self.expect_end_of_statement()?;

        Ok(Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(raw_cond),
                span: kw_tok.span.clone(),
            },
            then_branch,
            otherwise_branch,
            span: kw_tok.span,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let condition = self.parse_expression()?;
        self.expect_end_of_statement()?;
        let body = self.parse_block_until_end(&kw_tok, "while")?;
        Ok(Stmt::While {
            condition,
            body,
            span: kw_tok.span,
        })
    }

    fn parse_until(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let raw_cond = self.parse_expression()?;
        self.expect_end_of_statement()?;
        let body = self.parse_block_until_end(&kw_tok, "until")?;
        Ok(Stmt::While {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(raw_cond),
                span: kw_tok.span.clone(),
            },
            body,
            span: kw_tok.span,
        })
    }

    fn parse_repeat(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let count = self.parse_expression()?;
        if self.peek_ident_at(0) == Some("times") {
            self.advance();
        }
        self.expect_end_of_statement()?;
        let body = self.parse_block_until_end(&kw_tok, "repeat")?;
        Ok(Stmt::Repeat {
            count,
            body,
            span: kw_tok.span,
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        if self.peek_ident_at(0) == Some("each")
            && matches!(self.peek_next_kind(), Some(TokenKind::Identifier(_)))
            && self.peek_ident_at(1) != Some("from")
        {
            self.advance(); // each
        }
        let var_name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected loop variable name after 'for'.",
                    self.peek().span.clone(),
                    "A 'for' loop binds each element to a variable name.",
                    "Write `for item in items` or `for n from 1 to 5`.",
                    "Pick a clear loop variable name.",
                    "For whom does the loop toll?",
                ));
            }
        };
        self.advance();

        let iterable = if self.peek().kind == TokenKind::In {
            self.advance();
            self.parse_expression()?
        } else if self.peek_ident_at(0) == Some("from") {
            self.advance(); // from
            let start = self.parse_expression()?;
            if self.peek_ident_at(0) != Some("to") {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected 'to' after range start in 'for ... from ... to ...' loop.",
                    self.peek().span.clone(),
                    "Inclusive range loops follow the form `for n from 1 to 10`.",
                    format!("Add `to <end>` after the start value in `for {var_name} from ...`."),
                    "Or use `for n in range(1, 11)` for half-open ranges.",
                    "Missing destination after 'from'.",
                ));
            }
            self.advance(); // to
            let end_expr = self.parse_expression()?;
            let end_plus_one = Expr::Binary {
                left: Box::new(end_expr),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit {
                    value: 1,
                    span: kw_tok.span.clone(),
                }),
                span: kw_tok.span.clone(),
            };
            Expr::Call {
                callee: "range".to_string(),
                args: vec![start, end_plus_one],
                span: kw_tok.span.clone(),
            }
        } else {
            return Err(DogDiagnostic::new(
                "E004",
                "Expected 'in' or 'from' after loop variable in 'for' loop.",
                self.peek().span.clone(),
                "DOG 'for' loops follow `for item in collection` or `for n from 1 to 5`.",
                format!("Add `in` or `from` after `{var_name}`."),
                "Check loop header syntax.",
                "Missing 'in' or 'from' after loop variable.",
            ));
        };

        self.expect_end_of_statement()?;
        let body = self.parse_block_until_end(&kw_tok, "for")?;
        Ok(Stmt::For {
            var_name,
            iterable,
            body,
            span: kw_tok.span,
        })
    }

    fn parse_fn(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected function name after function keyword.",
                    self.peek().span.clone(),
                    "Functions must have a valid identifier name.",
                    "Write e.g. `fn greet(name: text) -> text` or `define greet(name: text) returns text`.",
                    "Use lowercase_snake_case for function names.",
                    "Anonymous paws need a name here.",
                ));
            }
        };
        self.advance();

        if self.peek().kind != TokenKind::LParen {
            return Err(DogDiagnostic::new(
                "E004",
                format!("Expected '(' after function name '{name}'."),
                self.peek().span.clone(),
                "Function declarations require a parameter list in parentheses.",
                format!("Write `fn {name}()` if it takes no arguments."),
                "Always include `()` after the function name.",
                "Where do the parameters go?",
            ));
        }
        self.advance();

        let mut params = Vec::new();
        if self.peek().kind != TokenKind::RParen {
            loop {
                let param_span = self.peek().span.clone();
                let param_name = match &self.peek().kind {
                    TokenKind::Identifier(n) => n.clone(),
                    _ => {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected parameter name.",
                            param_span,
                            "Function parameters must start with an identifier name.",
                            "Write parameters as `name: text`.",
                            "Separate multiple parameters with commas.",
                            "Give your parameters proper names.",
                        ));
                    }
                };
                self.advance();

                if self.peek().kind != TokenKind::Colon {
                    return Err(DogDiagnostic::new(
                        "E004",
                        format!("Expected ':' and type annotation for parameter '{param_name}'."),
                        self.peek().span.clone(),
                        "Strict typing requires explicit parameter types in function signatures.",
                        format!("Annotate the parameter type, e.g. `{param_name}: number`."),
                        "Available types: text, number, decimal, bool, list<T>, map<K, V>, Struct, Enum.",
                        "DOG refuses to guess parameter types.",
                    ));
                }
                self.advance();
                let ty = self.parse_type_ref()?;
                let default = if self.peek().kind == TokenKind::Assign {
                    self.advance();
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                params.push(Param {
                    name: param_name,
                    ty,
                    default,
                    span: param_span,
                });

                if self.peek().kind == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if self.peek().kind != TokenKind::RParen {
            return Err(DogDiagnostic::new(
                "E004",
                "Expected ')' after function parameters.",
                self.peek().span.clone(),
                "Parameter list was not closed with ')'.",
                "Add ')' after the last parameter.",
                "Balance parentheses in function headers.",
                "Left the parameter list wide open.",
            ));
        }
        self.advance();

        let return_type =
            if self.peek().kind == TokenKind::Arrow || self.peek_ident_at(0) == Some("returns") {
                self.advance();
                self.parse_type_ref()?
            } else {
                TypeRef::Void
            };

        self.expect_end_of_statement()?;
        let body = self.parse_block_until_end(&kw_tok, "fn")?;

        Ok(Stmt::FnDef {
            name,
            params,
            return_type,
            body,
            span: kw_tok.span,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let value = match self.peek().kind {
            TokenKind::Newline | TokenKind::Eof => None,
            _ => Some(self.parse_expression()?),
        };
        self.expect_end_of_statement()?;
        Ok(Stmt::Return {
            value,
            span: kw_tok.span,
        })
    }

    fn parse_style(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone(); // style
        let selector = match &self.peek().kind {
            TokenKind::Identifier(id) if matches!(id.as_str(), "theme" | "root" | "vars") => {
                let s = self.advance().span.clone();
                Expr::StringLit {
                    value: ":root".to_string(),
                    span: s,
                }
            }
            _ => self.parse_expression()?,
        };
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut rules = Vec::new();
        let mut variants = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::End {
            let line_span = self.peek().span.clone();
            if let Some(condition) = self.try_parse_style_variant_header()? {
                self.expect_end_of_statement()?;
                self.skip_newlines();
                let mut v_rules = Vec::new();
                while !self.is_at_end() && self.peek().kind != TokenKind::End {
                    v_rules.push(self.parse_style_prop_rule()?);
                    self.skip_newlines();
                }
                if self.peek().kind != TokenKind::End {
                    return Err(DogDiagnostic::new(
                        "E005",
                        "Missing 'end' for style variant block.",
                        line_span,
                        "Variant blocks inside `style ... end` (such as `hover` or `on mobile`) must close with `end`.",
                        "Add `end` on its own line.",
                        "Close inner blocks before closing the outer `style` block.",
                        "An unclosed style variant wanders off.",
                    ));
                }
                self.advance();
                self.expect_end_of_statement()?;
                self.skip_newlines();
                variants.push(StyleVariantBlock {
                    condition,
                    rules: v_rules,
                    span: line_span,
                });
            } else {
                rules.push(self.parse_style_prop_rule()?);
                self.skip_newlines();
            }
        }

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                "Missing 'end' for 'style' block.",
                kw_tok.span,
                "Every `style \"<selector>\" ... end` block must be closed with `end`.",
                "Add `end` on its own line after the style rules.",
                "Example:\nstyle \".card\"\n    bg: \"#FFFFFF\"\n    pad: \"18px\"\nend",
                "Left the stylesheet rule unfinished.",
            ));
        }
        self.advance();
        self.expect_end_of_statement()?;

        Ok(Stmt::StyleDef {
            selector,
            rules,
            variants,
            span: kw_tok.span,
        })
    }

    fn try_parse_style_variant_header(&mut self) -> Result<Option<String>, DogDiagnostic> {
        let is_on_prefix = self.peek_ident_at(0) == Some("on");
        let target_offset = if is_on_prefix { 1 } else { 0 };
        let Some(kw) = self.peek_ident_at(target_offset).map(str::to_string) else {
            return Ok(None);
        };
        let cond = match kw.as_str() {
            "hover" => ":hover".to_string(),
            "focus" => ":focus".to_string(),
            "active" => ":active".to_string(),
            "mobile" => "@media (max-width: 640px)".to_string(),
            "tablet" => "@media (max-width: 960px)".to_string(),
            "desktop" => "@media (min-width: 1024px)".to_string(),
            "dark" => "@media (prefers-color-scheme: dark)".to_string(),
            "media" if !is_on_prefix => {
                let after = self.peek_kind_at(1).cloned();
                if matches!(after, Some(TokenKind::StringLit(_))) {
                    self.advance(); // media
                    if let TokenKind::StringLit(q) = self.advance().kind.clone() {
                        return Ok(Some(format!("@media {q}")));
                    }
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };
        if matches!(self.peek_kind_at(target_offset + 1), Some(TokenKind::Colon)) {
            return Ok(None);
        }
        if is_on_prefix {
            self.advance();
        }
        self.advance();
        Ok(Some(cond))
    }

    fn parse_style_prop_rule(&mut self) -> Result<StyleRuleDecl, DogDiagnostic> {
        let start_span = self.peek().span.clone();
        let mut prop = String::new();
        while !self.is_at_end()
            && !matches!(
                self.peek().kind,
                TokenKind::Colon | TokenKind::Newline | TokenKind::Eof
            )
        {
            let tok = self.advance().clone();
            match tok.kind {
                TokenKind::Identifier(s) => prop.push_str(&s),
                TokenKind::Minus => prop.push('-'),
                TokenKind::NumberLit(n) => prop.push_str(&n.to_string()),
                _ => {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Invalid CSS property name inside 'style' block.",
                        tok.span,
                        "Style properties must use `property: \"value\"` syntax.",
                        "Write e.g. `bg: \"#F7F6F2\"` or `pad: \"16px\"`.",
                        "Valid shorthands include bg, ink, pad, px, py, radius, gap, cols, shadow.",
                        "Unexpected token before ':'.",
                    ));
                }
            }
        }
        if prop.is_empty() || self.peek().kind != TokenKind::Colon {
            return Err(DogDiagnostic::new(
                "E004",
                "Expected `property: value` inside 'style' block.",
                start_span,
                "Each declaration inside `style ... end` requires a CSS property name followed by ':'.",
                "Write e.g. `bg: \"#FFFFFF\"` or `--accent: \"#2F6BFF\"`.",
                "Use `on hover ... end` or `on mobile ... end` for interactive/responsive variants.",
                "Style property is missing ':'.",
            ));
        }
        self.advance(); // :
        let value = self.parse_expression()?;
        if self.peek().kind == TokenKind::Comma {
            self.advance();
        }
        self.expect_end_of_statement()?;
        Ok(StyleRuleDecl {
            prop,
            value,
            span: start_span,
        })
    }

    fn parse_style(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone(); // style
        let selector = match &self.peek().kind {
            TokenKind::Identifier(id) if matches!(id.as_str(), "theme" | "root" | "vars") => {
                let s = self.advance().span.clone();
                Expr::StringLit {
                    value: ":root".to_string(),
                    span: s,
                }
            }
            _ => self.parse_expression()?,
        };
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut rules = Vec::new();
        let mut variants = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::End {
            let line_span = self.peek().span.clone();
            if let Some(condition) = self.try_parse_style_variant_header()? {
                self.expect_end_of_statement()?;
                self.skip_newlines();
                let mut v_rules = Vec::new();
                while !self.is_at_end() && self.peek().kind != TokenKind::End {
                    v_rules.push(self.parse_style_prop_rule()?);
                    self.skip_newlines();
                }
                if self.peek().kind != TokenKind::End {
                    return Err(DogDiagnostic::new(
                        "E005",
                        "Missing 'end' for style variant block.",
                        line_span,
                        "Variant blocks inside `style ... end` (such as `hover` or `on mobile`) must close with `end`.",
                        "Add `end` on its own line.",
                        "Close inner blocks before closing the outer `style` block.",
                        "An unclosed style variant wanders off.",
                    ));
                }
                self.advance();
                self.expect_end_of_statement()?;
                self.skip_newlines();
                variants.push(StyleVariantBlock {
                    condition,
                    rules: v_rules,
                    span: line_span,
                });
            } else {
                rules.push(self.parse_style_prop_rule()?);
                self.skip_newlines();
            }
        }

        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                "Missing 'end' for 'style' block.",
                kw_tok.span,
                "Every `style \"<selector>\" ... end` block must be closed with `end`.",
                "Add `end` on its own line after the style rules.",
                "Example:\nstyle \".card\"\n    bg: \"#FFFFFF\"\n    pad: \"18px\"\nend",
                "Left the stylesheet rule unfinished.",
            ));
        }
        self.advance();
        self.expect_end_of_statement()?;

        Ok(Stmt::StyleDef {
            selector,
            rules,
            variants,
            span: kw_tok.span,
        })
    }

    fn try_parse_style_variant_header(&mut self) -> Result<Option<String>, DogDiagnostic> {
        let is_on_prefix = self.peek_ident_at(0) == Some("on");
        let target_offset = if is_on_prefix { 1 } else { 0 };
        let Some(kw) = self.peek_ident_at(target_offset).map(str::to_string) else {
            return Ok(None);
        };
        let cond = match kw.as_str() {
            "hover" => ":hover".to_string(),
            "focus" => ":focus".to_string(),
            "active" => ":active".to_string(),
            "mobile" => "@media (max-width: 640px)".to_string(),
            "tablet" => "@media (max-width: 960px)".to_string(),
            "desktop" => "@media (min-width: 1024px)".to_string(),
            "dark" => "@media (prefers-color-scheme: dark)".to_string(),
            "media" if !is_on_prefix => {
                let after = self.peek_kind_at(1).cloned();
                if matches!(after, Some(TokenKind::StringLit(_))) {
                    self.advance(); // media
                    if let TokenKind::StringLit(q) = self.advance().kind.clone() {
                        return Ok(Some(format!("@media {q}")));
                    }
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };
        if matches!(self.peek_kind_at(target_offset + 1), Some(TokenKind::Colon)) {
            return Ok(None);
        }
        if is_on_prefix {
            self.advance();
        }
        self.advance();
        Ok(Some(cond))
    }

    fn parse_style_prop_rule(&mut self) -> Result<StyleRuleDecl, DogDiagnostic> {
        let start_span = self.peek().span.clone();
        let mut prop = String::new();
        while !self.is_at_end()
            && !matches!(
                self.peek().kind,
                TokenKind::Colon | TokenKind::Newline | TokenKind::Eof
            )
        {
            let tok = self.advance().clone();
            match tok.kind {
                TokenKind::Identifier(s) => prop.push_str(&s),
                TokenKind::Minus => prop.push('-'),
                TokenKind::NumberLit(n) => prop.push_str(&n.to_string()),
                _ => {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Invalid CSS property name inside 'style' block.",
                        tok.span,
                        "Style properties must use `property: \"value\"` syntax.",
                        "Write e.g. `bg: \"#F7F6F2\"` or `pad: \"16px\"`.",
                        "Valid shorthands include bg, ink, pad, px, py, radius, gap, cols, shadow.",
                        "Unexpected token before ':'.",
                    ));
                }
            }
        }
        if prop.is_empty() || self.peek().kind != TokenKind::Colon {
            return Err(DogDiagnostic::new(
                "E004",
                "Expected `property: value` inside 'style' block.",
                start_span,
                "Each declaration inside `style ... end` requires a CSS property name followed by ':'.",
                "Write e.g. `bg: \"#FFFFFF\"` or `--accent: \"#2F6BFF\"`.",
                "Use `on hover ... end` or `on mobile ... end` for interactive/responsive variants.",
                "Style property is missing ':'.",
            ));
        }
        self.advance(); // :
        let value = self.parse_expression()?;
        if self.peek().kind == TokenKind::Comma {
            self.advance();
        }
        self.expect_end_of_statement()?;
        Ok(StyleRuleDecl {
            prop,
            value,
            span: start_span,
        })
    }

    fn parse_block_until_end(
        &mut self,
        start_tok: &Token,
        label: &str,
    ) -> Result<Vec<Stmt>, DogDiagnostic> {
        self.skip_newlines();
        let mut body = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::End {
            body.push(self.parse_statement()?);
            self.skip_newlines();
        }
        if self.peek().kind != TokenKind::End {
            return Err(DogDiagnostic::new(
                "E005",
                format!("Missing 'end' keyword for '{label}' block."),
                start_tok.span.clone(),
                format!("DOG uses 'end' to close every '{label}' block."),
                "Add `end` on its own line at the end of the block.",
                "Indentation is visual; `end` closes the block.",
                "An unclosed block wanders off forever.",
            ));
        }
        self.advance(); // end
        self.expect_end_of_statement()?;
        Ok(body)
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, DogDiagnostic> {
        let tok = self.advance().clone();
        match tok.kind {
            TokenKind::Identifier(name) => match name.as_str() {
                "text" => Ok(TypeRef::Text),
                "number" => Ok(TypeRef::Number),
                "decimal" => Ok(TypeRef::Decimal),
                "bool" => Ok(TypeRef::Bool),
                "void" => Ok(TypeRef::Void),
                "list" => {
                    if self.peek().kind != TokenKind::Lt {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected '<type>' after 'list'.",
                            self.peek().span.clone(),
                            "Lists require an element type.",
                            "Write e.g. `list<text>` or `list<number>`.",
                            "Specify the inner element type inside `<...>`.",
                            "A list of what, exactly?",
                        ));
                    }
                    self.advance(); // <
                    let inner = self.parse_type_ref()?;
                    if self.peek().kind != TokenKind::Gt {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected '>' to close 'list<...>' type.",
                            self.peek().span.clone(),
                            "Generic list type was not closed with '>'.",
                            "Close the angle bracket, e.g. `list<number>`.",
                            "Check nested brackets.",
                            "Angle brackets need closure too.",
                        ));
                    }
                    self.advance(); // >
                    Ok(TypeRef::List(Box::new(inner)))
                }
                "map" => {
                    if self.peek().kind != TokenKind::Lt {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected '<key, value>' after 'map'.",
                            self.peek().span.clone(),
                            "Maps require key and value types.",
                            "Write e.g. `map<text, number>`.",
                            "Specify key and value types inside `<...>`.",
                            "A map from what to what?",
                        ));
                    }
                    self.advance(); // <
                    let k = self.parse_type_ref()?;
                    if self.peek().kind != TokenKind::Comma {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected ',' between map key and value types.",
                            self.peek().span.clone(),
                            "Map types take two type arguments: `map<key, value>`.",
                            "Write e.g. `map<text, text>`.",
                            "Separate key and value types with a comma.",
                            "Maps always need two types.",
                        ));
                    }
                    self.advance(); // ,
                    let v = self.parse_type_ref()?;
                    if self.peek().kind != TokenKind::Gt {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected '>' to close 'map<...>' type.",
                            self.peek().span.clone(),
                            "Generic map type was not closed with '>'.",
                            "Add '>' after the value type.",
                            "Check angle brackets.",
                            "Angle brackets need closure too.",
                        ));
                    }
                    self.advance(); // >
                    Ok(TypeRef::Map(Box::new(k), Box::new(v)))
                }
                other => Ok(TypeRef::Named(other.to_string())),
            },
            _ => Err(DogDiagnostic::new(
                "E004",
                "Expected type name.",
                tok.span,
                "Expected a DOG type annotation here.",
                "Use text, number, decimal, bool, void, list<T>, map<K, V>, or a struct/enum name.",
                "Check type syntax after `:` or `->`.",
                "Types aren't optional when you ask for one.",
            )),
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, DogDiagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_and()?;
        while self.peek().kind == TokenKind::Or {
            let op_tok = self.advance().clone();
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinOp::Or,
                right: Box::new(right),
                span: op_tok.span,
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_equality()?;
        while self.peek().kind == TokenKind::And {
            let op_tok = self.advance().clone();
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinOp::And,
                right: Box::new(right),
                span: op_tok.span,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_comparison()?;
        loop {
            let (op, consume_count) = match self.peek().kind {
                TokenKind::EqEq => (BinOp::Eq, 1usize),
                TokenKind::BangEq => (BinOp::NotEq, 1usize),
                _ if self.peek_ident_at(0) == Some("equals") => (BinOp::Eq, 1usize),
                _ if self.peek_ident_at(0) == Some("is") => {
                    if self.peek_kind_at(1) == Some(&TokenKind::Not)
                        && self.peek_ident_at(2) == Some("equal")
                        && self.peek_ident_at(3) == Some("to")
                    {
                        (BinOp::NotEq, 4usize)
                    } else if self.peek_kind_at(1) == Some(&TokenKind::Not) {
                        (BinOp::NotEq, 2usize)
                    } else if self.peek_ident_at(1) == Some("equal")
                        && self.peek_ident_at(2) == Some("to")
                    {
                        (BinOp::Eq, 3usize)
                    } else {
                        (BinOp::Eq, 1usize)
                    }
                }
                _ => break,
            };
            let op_tok = self.peek().clone();
            for _ in 0..consume_count {
                self.advance();
            }
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span: op_tok.span,
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_term()?;
        loop {
            let (op, consume_count) = match self.peek().kind {
                TokenKind::Gt => (BinOp::Gt, 1usize),
                TokenKind::Lt => (BinOp::Lt, 1usize),
                TokenKind::GtEq => (BinOp::GtEq, 1usize),
                TokenKind::LtEq => (BinOp::LtEq, 1usize),
                _ if self.peek_ident_at(0) == Some("is") => {
                    if self.peek_ident_at(1) == Some("greater")
                        && self.peek_ident_at(2) == Some("than")
                    {
                        (BinOp::Gt, 3usize)
                    } else if self.peek_ident_at(1) == Some("less")
                        && self.peek_ident_at(2) == Some("than")
                    {
                        (BinOp::Lt, 3usize)
                    } else if self.peek_ident_at(1) == Some("at")
                        && self.peek_ident_at(2) == Some("least")
                    {
                        (BinOp::GtEq, 3usize)
                    } else if self.peek_ident_at(1) == Some("at")
                        && self.peek_ident_at(2) == Some("most")
                    {
                        (BinOp::LtEq, 3usize)
                    } else if self.peek_ident_at(1) == Some("over") {
                        (BinOp::Gt, 2usize)
                    } else if self.peek_ident_at(1) == Some("under") {
                        (BinOp::Lt, 2usize)
                    } else {
                        break;
                    }
                }
                _ => break,
            };
            let op_tok = self.peek().clone();
            for _ in 0..consume_count {
                self.advance();
            }
            let right = self.parse_term()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span: op_tok.span,
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            let op_tok = self.advance().clone();
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span: op_tok.span,
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            let op_tok = self.advance().clone();
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span: op_tok.span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, DogDiagnostic> {
        match self.peek().kind {
            TokenKind::Minus => {
                let op_tok = self.advance().clone();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span: op_tok.span,
                })
            }
            TokenKind::Not => {
                let op_tok = self.advance().clone();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span: op_tok.span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, DogDiagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.peek().kind == TokenKind::LBracket {
                let bracket_tok = self.advance().clone();
                let index = self.parse_expression()?;
                if self.peek().kind != TokenKind::RBracket {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected ']' after index expression.",
                        self.peek().span.clone(),
                        "An index expression opened with '[' must be closed with ']'.",
                        "Add ']' after the index expression.",
                        "Example: `items[0]`.",
                        "Open bracket, missing end.",
                    ));
                }
                self.advance(); // ]
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                    span: bracket_tok.span,
                };
            } else if self.peek().kind == TokenKind::Dot {
                let dot_tok = self.advance().clone();
                let field = match &self.peek().kind {
                    TokenKind::Identifier(n) => n.clone(),
                    _ => {
                        return Err(DogDiagnostic::new(
                            "E004",
                            "Expected field or variant name after '.'.",
                            self.peek().span.clone(),
                            "Dot access requires an identifier on the right side.",
                            "Example: `user.name` or `Role.Admin`.",
                            "Check property spelling.",
                            "A dot pointing at nothing.",
                        ));
                    }
                };
                self.advance();
                expr = Expr::FieldAccess {
                    target: Box::new(expr),
                    field,
                    span: dot_tok.span.clone(),
                };
                if self.peek().kind == TokenKind::LParen {
                    if let Some(mut parts) = extract_dotted_path(&expr) {
                        if parts.len() >= 3 && parts[0] == "std" {
                            let func = parts.pop().unwrap_or_default();
                            let module = parts.join(".");
                            let label = format!("{module}.{func}");
                            let args = self.parse_call_args(&label)?;
                            expr = Expr::NamespaceCall {
                                module,
                                func,
                                args,
                                span: dot_tok.span,
                            };
                            continue;
                        }
                    }
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Unexpected '(' after dot access.",
                        self.peek().span.clone(),
                        "Dot calls in DOG are reserved for standard library modules like `std.io.ask(...)` or `std.text.upper(...)`.",
                        "Use `std.<module>.<fn>(...)` for module calls or `fn_name(value, ...)` for functions.",
                        "DOG functions take values as explicit arguments.",
                        "Values don't carry secret methods here — unless they live in `std.*`.",
                    ));
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self, call_name: &str) -> Result<Vec<Expr>, DogDiagnostic> {
        self.advance(); // (
        let mut args = Vec::new();
        if self.peek().kind != TokenKind::RParen {
            loop {
                args.push(self.parse_expression()?);
                if self.peek().kind == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        if self.peek().kind != TokenKind::RParen {
            return Err(DogDiagnostic::new(
                "E004",
                format!("Expected ')' after arguments to '{call_name}()'."),
                self.peek().span.clone(),
                "A function call opened with '(' was not closed with ')'.",
                "Add the missing closing parenthesis ')'.",
                "Check nested calls like number(answer(\"Age: \")).",
                "Unclosed parentheses keep me awake at night.",
            ));
        }
        self.advance(); // )
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, DogDiagnostic> {
        let tok = self.advance().clone();
        match tok.kind {
            TokenKind::StringLit(value) => Ok(Expr::StringLit {
                value,
                span: tok.span,
            }),
            TokenKind::InterpString(lex_parts) => {
                let mut parts = Vec::with_capacity(lex_parts.len());
                for part in lex_parts {
                    match part {
                        InterpLexPart::Literal(s) => parts.push(InterpPart::Literal(s)),
                        InterpLexPart::Expr { source, span } => {
                            let sub_tokens = Lexer::new(&span.file, &source).tokenize()?;
                            let mut sub_parser = Parser::new(sub_tokens);
                            sub_parser.skip_newlines();
                            let expr = sub_parser.parse_expression()?;
                            sub_parser.skip_newlines();
                            if !sub_parser.is_at_end() {
                                return Err(DogDiagnostic::new(
                                    "E004",
                                    "Unexpected trailing tokens inside interpolation expression.",
                                    span,
                                    "Each `{...}` inside an interpolated string must contain a single valid expression.",
                                    "Write one expression per `{...}`, like `$\"{a} {b}\"`.",
                                    "Avoid extra statements inside `{...}`.",
                                    "Too much happening inside one pair of braces.",
                                ));
                            }
                            parts.push(InterpPart::Expr(expr));
                        }
                    }
                }
                Ok(Expr::InterpString {
                    parts,
                    span: tok.span,
                })
            }
            TokenKind::NumberLit(value) => Ok(Expr::NumberLit {
                value,
                span: tok.span,
            }),
            TokenKind::DecimalLit(value) => Ok(Expr::DecimalLit {
                value,
                span: tok.span,
            }),
            TokenKind::True => Ok(Expr::BoolLit {
                value: true,
                span: tok.span,
            }),
            TokenKind::False => Ok(Expr::BoolLit {
                value: false,
                span: tok.span,
            }),
            TokenKind::LBracket => {
                let mut elements = Vec::new();
                if self.peek().kind != TokenKind::RBracket {
                    loop {
                        elements.push(self.parse_expression()?);
                        if self.peek().kind == TokenKind::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                if self.peek().kind != TokenKind::RBracket {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected ']' after list literal.",
                        self.peek().span.clone(),
                        "A list literal opened with '[' was not closed with ']'.",
                        "Add ']' after the final element.",
                        "Example: `[\"a\", \"b\"]`.",
                        "Dropped the closing bracket somewhere in the yard.",
                    ));
                }
                self.advance(); // ]
                Ok(Expr::ListLit {
                    elements,
                    span: tok.span,
                })
            }
            TokenKind::LBrace => {
                let mut entries = Vec::new();
                self.skip_newlines();
                if self.peek().kind != TokenKind::RBrace {
                    loop {
                        self.skip_newlines();
                        let key = self.parse_expression()?;
                        if self.peek().kind != TokenKind::Colon {
                            return Err(DogDiagnostic::new(
                                "E004",
                                "Expected ':' after map key.",
                                self.peek().span.clone(),
                                "Map entries use `key: value` syntax.",
                                "Example: `{\"host\": \"localhost\"}`.",
                                "Separate entries with commas.",
                                "Keys and values need a colon bridge.",
                            ));
                        }
                        self.advance(); // :
                        let val = self.parse_expression()?;
                        entries.push((key, val));
                        self.skip_newlines();
                        if self.peek().kind == TokenKind::Comma {
                            self.advance();
                            self.skip_newlines();
                            if self.peek().kind == TokenKind::RBrace {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                if self.peek().kind != TokenKind::RBrace {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected '}' after map literal.",
                        self.peek().span.clone(),
                        "A map literal opened with '{' was not closed with '}'.",
                        "Add '}' after the last entry.",
                        "Example: `{\"a\": 1, \"b\": 2}`.",
                        "Unclosed map brace.",
                    ));
                }
                self.advance(); // }
                Ok(Expr::MapLit {
                    entries,
                    span: tok.span,
                })
            }
            TokenKind::Identifier(name) => {
                if self.peek().kind == TokenKind::LParen {
                    let args = self.parse_call_args(&name)?;
                    Ok(Expr::Call {
                        callee: name,
                        args,
                        span: tok.span,
                    })
                } else if self.peek().kind == TokenKind::LBrace {
                    self.advance(); // {
                    self.skip_newlines();
                    let mut fields = Vec::new();
                    if self.peek().kind != TokenKind::RBrace {
                        loop {
                            self.skip_newlines();
                            let f_name = match &self.peek().kind {
                                TokenKind::Identifier(n) => n.clone(),
                                _ => {
                                    return Err(DogDiagnostic::new(
                                        "E004",
                                        "Expected field name in struct initializer.",
                                        self.peek().span.clone(),
                                        "Struct initializers write `field: value` inside `{ ... }`.",
                                        format!("Example: `{name} {{ field: value }}`."),
                                        "Check field names against the struct definition.",
                                        "Fields need identifier names.",
                                    ));
                                }
                            };
                            self.advance();
                            if self.peek().kind != TokenKind::Colon {
                                return Err(DogDiagnostic::new(
                                    "E004",
                                    format!("Expected ':' after field '{f_name}'."),
                                    self.peek().span.clone(),
                                    "Struct initializers use `field: value`.",
                                    format!("Write `{f_name}: ...`."),
                                    "Use commas between fields.",
                                    "Colon missing between field and value.",
                                ));
                            }
                            self.advance();
                            let val = self.parse_expression()?;
                            fields.push((f_name, val));
                            self.skip_newlines();
                            if self.peek().kind == TokenKind::Comma {
                                self.advance();
                                self.skip_newlines();
                                if self.peek().kind == TokenKind::RBrace {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    if self.peek().kind != TokenKind::RBrace {
                        return Err(DogDiagnostic::new(
                            "E004",
                            format!("Expected '}}' to close '{name}' initializer."),
                            self.peek().span.clone(),
                            "A struct initializer opened with '{' was not closed.",
                            "Add '}' after the last field.",
                            "Check comma and brace balance.",
                            "Left the struct door open.",
                        ));
                    }
                    self.advance();
                    Ok(Expr::StructInit {
                        name,
                        fields,
                        span: tok.span,
                    })
                } else {
                    Ok(Expr::Identifier {
                        name,
                        span: tok.span,
                    })
                }
            }
            TokenKind::LParen => {
                let inner = self.parse_expression()?;
                if self.peek().kind != TokenKind::RParen {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Expected closing ')'.",
                        self.peek().span.clone(),
                        "A grouped expression opened with '(' has no matching ')'.",
                        "Add ')' after the expression.",
                        "Balance every '(' with ')'.",
                        "One parenthesis lonely in the dark.",
                    ));
                }
                self.advance();
                Ok(inner)
            }
            _ => Err(DogDiagnostic::new(
                "E004",
                "Unexpected token in expression.",
                tok.span,
                "Expected a value, identifier, or function call here.",
                "Provide a valid expression such as a string, number, or variable.",
                "Check for missing arguments or stray operators.",
                "I stared at this token and it stared back.",
            )),
        }
    }

    fn expect_end_of_statement(&mut self) -> Result<(), DogDiagnostic> {
        match self.peek().kind {
            TokenKind::Newline | TokenKind::Eof => {
                if self.peek().kind == TokenKind::Newline {
                    self.advance();
                }
                Ok(())
            }
            _ => Err(DogDiagnostic::new(
                "E004",
                "Unexpected trailing token on line.",
                self.peek().span.clone(),
                "Expected a new line after statement.",
                "Place the next statement on its own line.",
                "DOG uses one statement per line without mandatory semicolons.",
                "One thing at a time, please.",
            )),
        }
    }

    fn skip_newlines(&mut self) {
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("Lexer always appends Eof token")
        })
    }

    fn peek_next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos + 1).map(|t| &t.kind)
    }

    fn peek_kind_at(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens.get(self.pos + offset).map(|t| &t.kind)
    }

    fn peek_ident_at(&self, offset: usize) -> Option<&str> {
        match self.tokens.get(self.pos + offset).map(|t| &t.kind) {
            Some(TokenKind::Identifier(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        &self.tokens[self.pos - 1]
    }
}

fn extract_dotted_path(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Identifier { name, .. } => Some(vec![name.clone()]),
        Expr::FieldAccess { target, field, .. } => {
            let mut parts = extract_dotted_path(target)?;
            parts.push(field.clone());
            Some(parts)
        }
        _ => None,
    }
}
