use crate::ast::{BinOp, BindingKind, Expr, Param, Program, Stmt, TypeRef, UnaryOp};
use crate::diagnostics::DogDiagnostic;
use crate::lexer::{Token, TokenKind};

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
            TokenKind::Let => self.parse_binding(BindingKind::Let),
            TokenKind::Var => self.parse_binding(BindingKind::Var),
            TokenKind::Const => self.parse_binding(BindingKind::Const),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Repeat => self.parse_repeat(),
            TokenKind::For => self.parse_for(),
            TokenKind::Fn => self.parse_fn(),
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
            TokenKind::Identifier(name) if self.peek_next_kind() == Some(&TokenKind::Assign) => {
                let name = name.clone();
                let span = tok.span.clone();
                self.advance(); // ident
                self.advance(); // =
                let value = self.parse_expression()?;
                self.expect_end_of_statement()?;
                Ok(Stmt::Assign { name, value, span })
            }
            _ => {
                let span = tok.span.clone();
                let expr = self.parse_expression()?;
                if self.peek().kind == TokenKind::Assign {
                    if let Expr::Index { target, index, .. } = expr {
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
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Invalid assignment target.",
                        self.peek().span.clone(),
                        "Only variables or indexed list variables can appear on the left side of '='.",
                        "Assign to a variable name like `x = 10` or `items[0] = 10`.",
                        "Use `==` when checking equality.",
                        "You can't assign a value to thin air.",
                    ));
                }
                self.expect_end_of_statement()?;
                Ok(Stmt::ExprStmt { expr, span })
            }
        }
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

    fn parse_if(&mut self) -> Result<Stmt, DogDiagnostic> {
        let if_tok = self.advance().clone();
        let condition = self.parse_expression()?;
        self.expect_end_of_statement()?;
        self.skip_newlines();

        let mut then_branch = Vec::new();
        while !self.is_at_end()
            && self.peek().kind != TokenKind::Otherwise
            && self.peek().kind != TokenKind::End
        {
            then_branch.push(self.parse_statement()?);
            self.skip_newlines();
        }

        let otherwise_branch = if self.peek().kind == TokenKind::Otherwise {
            self.advance(); // otherwise
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
                "DOG uses 'end' (not indentation) to close every 'if' or 'otherwise' block.",
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

    fn parse_repeat(&mut self) -> Result<Stmt, DogDiagnostic> {
        let kw_tok = self.advance().clone();
        let count = self.parse_expression()?;
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
        let var_name = match &self.peek().kind {
            TokenKind::Identifier(n) => n.clone(),
            _ => {
                return Err(DogDiagnostic::new(
                    "E004",
                    "Expected loop variable name after 'for'.",
                    self.peek().span.clone(),
                    "A 'for' loop binds each element to a variable name.",
                    "Write e.g. `for item in items`.",
                    "Pick a clear loop variable name.",
                    "For whom does the loop toll?",
                ));
            }
        };
        self.advance();

        if self.peek().kind != TokenKind::In {
            return Err(DogDiagnostic::new(
                "E004",
                "Expected 'in' after loop variable in 'for' loop.",
                self.peek().span.clone(),
                "DOG 'for' loops follow the form `for item in collection`.",
                format!("Add `in` after `{var_name}`."),
                "Check loop header syntax.",
                "Missing 'in' between dog and yard.",
            ));
        }
        self.advance();

        let iterable = self.parse_expression()?;
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
                    "Expected function name after 'fn'.",
                    self.peek().span.clone(),
                    "Functions must have a valid identifier name.",
                    "Write e.g. `fn greet(name: text) -> text`.",
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
                        "Available types: text, number, decimal, bool, list<T>.",
                        "DOG refuses to guess parameter types.",
                    ));
                }
                self.advance();
                let ty = self.parse_type_ref()?;
                params.push(Param {
                    name: param_name,
                    ty,
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

        let return_type = if self.peek().kind == TokenKind::Arrow {
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
                            "Lists are strictly typed in DOG and require an element type.",
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
                other => Err(DogDiagnostic::new(
                    "E004",
                    format!("Unknown type '{other}'."),
                    tok.span,
                    format!("'{other}' is not a built-in DOG type."),
                    "Use text, number, decimal, bool, void, or list<T>.",
                    "Check spelling of the type annotation.",
                    "I haven't learned that species yet.",
                )),
            },
            _ => Err(DogDiagnostic::new(
                "E004",
                "Expected type name.",
                tok.span,
                "Expected a DOG type annotation here.",
                "Use text, number, decimal, bool, void, or list<T>.",
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
            let op = match self.peek().kind {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::BangEq => BinOp::NotEq,
                _ => break,
            };
            let op_tok = self.advance().clone();
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
            let op = match self.peek().kind {
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::GtEq => BinOp::GtEq,
                TokenKind::LtEq => BinOp::LtEq,
                _ => break,
            };
            let op_tok = self.advance().clone();
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
        while self.peek().kind == TokenKind::LBracket {
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
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, DogDiagnostic> {
        let tok = self.advance().clone();
        match tok.kind {
            TokenKind::StringLit(value) => Ok(Expr::StringLit {
                value,
                span: tok.span,
            }),
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
            TokenKind::Identifier(name) => {
                if self.peek().kind == TokenKind::LParen {
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
                            format!("Expected ')' after arguments to '{name}()'."),
                            self.peek().span.clone(),
                            "A function call opened with '(' was not closed with ')'.",
                            "Add the missing closing parenthesis ')'.",
                            "Check nested calls like number(answer(\"Age: \")).",
                            "Unclosed parentheses keep me awake at night.",
                        ));
                    }
                    self.advance(); // )
                    Ok(Expr::Call {
                        callee: name,
                        args,
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

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        &self.tokens[self.pos - 1]
    }
}
