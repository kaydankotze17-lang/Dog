use crate::ast::{BinOp, BindingKind, Expr, Program, Span, Stmt, TypeRef, UnaryOp};
use crate::diagnostics::DogDiagnostic;
use crate::types::DogType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct BindingInfo {
    kind: BindingKind,
    ty: DogType,
}

#[derive(Debug, Clone)]
struct FnSig {
    params: Vec<DogType>,
    return_type: DogType,
}

const BUILTIN_NAMES: &[&str] = &[
    "say", "answer", "lower", "upper", "number", "decimal", "text", "len", "trim", "contains",
    "split", "join", "abs", "min", "max", "push", "pop", "assert",
];

pub struct TypeChecker {
    scopes: Vec<HashMap<String, BindingInfo>>,
    functions: HashMap<String, FnSig>,
    current_return_type: Option<DogType>,
    loop_depth: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            current_return_type: None,
            loop_depth: 0,
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), DogDiagnostic> {
        // Pass 1: register all top-level function signatures so recursion & mutual calls work.
        for stmt in &program.statements {
            if let Stmt::FnDef {
                name,
                params,
                return_type,
                span,
                ..
            } = stmt
            {
                if BUILTIN_NAMES.contains(&name.as_str()) {
                    return Err(DogDiagnostic::new(
                        "E011",
                        format!("Cannot redefine built-in function '{name}'."),
                        span.clone(),
                        format!("'{name}()' is a reserved DOG standard library built-in."),
                        "Choose a different function name.",
                        "Built-ins remain consistent across every DOG program.",
                        "Hands off the standard library.",
                    ));
                }
                if self.functions.contains_key(name) {
                    return Err(DogDiagnostic::new(
                        "E011",
                        format!("Function '{name}' is already defined."),
                        span.clone(),
                        format!("Two functions named '{name}' exist in this file."),
                        "Rename one of the functions.",
                        "Function names must be unique.",
                        "Two functions walked into a module with the same name.",
                    ));
                }
                self.functions.insert(
                    name.clone(),
                    FnSig {
                        params: params.iter().map(|p| DogType::from_ref(&p.ty)).collect(),
                        return_type: DogType::from_ref(return_type),
                    },
                );
            }
        }

        // Pass 2: type-check every statement and function body.
        for stmt in &program.statements {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), DogDiagnostic> {
        match stmt {
            Stmt::Binding {
                kind,
                name,
                type_ann,
                value,
                span,
            } => {
                let annotated = type_ann.as_ref().map(DogType::from_ref);
                let ty = self.check_expr_with_hint(value, annotated.as_ref())?;

                if ty == DogType::Void {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Cannot bind a 'void' expression to a variable.",
                        span.clone(),
                        format!("'{name}' cannot store the result of an action that returns nothing."),
                        "Assign an expression that produces text, number, decimal, bool, or list<T>.",
                        "say() prints output and returns void.",
                        "Trying to bottle empty air?",
                    )
                    .with_types("value", "void"));
                }

                if let Some(expected) = annotated {
                    if ty != expected {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch detected.",
                            span.clone(),
                            format!("'{name}' is annotated as '{expected}', but received '{ty}'."),
                            format!("Provide a '{expected}' value or update the type annotation."),
                            "Annotations lock down the expected type at declaration.",
                            "The label says one thing, the value says another.",
                        )
                        .with_types(expected.label(), ty.label()));
                    }
                }

                let current_scope = self.scopes.last_mut().expect("root scope exists");
                if current_scope.contains_key(name) {
                    return Err(DogDiagnostic::new(
                        "E011",
                        format!("Variable '{name}' is already declared in this scope."),
                        span.clone(),
                        format!("A binding named '{name}' already exists in the current block."),
                        format!("Use a different name, or reassign `{name} = ...` if it was declared with `var`."),
                        "Explicit names avoid accidental shadowing bugs.",
                        "Two dogs with the same collar causes chaos.",
                    ));
                }
                current_scope.insert(name.clone(), BindingInfo { kind: *kind, ty });
                Ok(())
            }
            Stmt::Assign { name, value, span } => {
                let existing = self.lookup(name).cloned().ok_or_else(|| {
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{name}'."),
                        span.clone(),
                        format!("No variable named '{name}' is in scope at this location."),
                        format!("Declare it first with `var {name} = ...`."),
                        "Variables must be declared before assignment.",
                        "Can't fetch a stick that was never thrown.",
                    )
                })?;

                if existing.kind != BindingKind::Var {
                    let kw = match existing.kind {
                        BindingKind::Let => "let",
                        BindingKind::Const => "const",
                        BindingKind::Var => "var",
                    };
                    return Err(DogDiagnostic::new(
                        "E012",
                        format!("Cannot reassign immutable {kw} binding '{name}'."),
                        span.clone(),
                        format!("'{name}' was declared with `{kw}`, which forbids mutation."),
                        format!("Change the declaration to `var {name} = ...` if it needs to change."),
                        "Prefer `let` for fixed values and `var` only when mutation is intentional.",
                        "Immutable means immutable. Even if you ask nicely.",
                    ));
                }

                let val_ty = self.check_expr_with_hint(value, Some(&existing.ty))?;
                if val_ty != existing.ty {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("'{name}' has type {}, so it cannot be assigned a {}.", existing.ty, val_ty),
                        format!("Convert the value to {} or assign a compatible expression.", existing.ty),
                        "Keep variable types consistent across assignments.",
                        "A leopard doesn't change its types.",
                    )
                    .with_types(existing.ty.label(), val_ty.label()));
                }
                Ok(())
            }
            Stmt::IndexAssign {
                target,
                index,
                value,
                span,
            } => {
                let existing = self.lookup(target).cloned().ok_or_else(|| {
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{target}'."),
                        span.clone(),
                        format!("No list named '{target}' is in scope."),
                        format!("Declare it with `var {target} = [...]`."),
                        "Declare variables before indexing into them.",
                        "No list, no index.",
                    )
                })?;
                if existing.kind != BindingKind::Var {
                    return Err(DogDiagnostic::new(
                        "E012",
                        format!("Cannot mutate immutable binding '{target}'."),
                        span.clone(),
                        format!("'{target}' was not declared with `var`, so its elements cannot be overwritten."),
                        format!("Declare `{target}` with `var` to allow indexed mutation."),
                        "Immutability protects list contents too.",
                        "Look, don't touch.",
                    ));
                }
                let inner_ty = match existing.ty {
                    DogType::List(inner) => *inner,
                    other => {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch detected.",
                            span.clone(),
                            format!("Cannot assign by index on type '{other}'."),
                            "Indexed assignment only works on `list<T>` variables.",
                            "Strings are immutable in DOG.",
                            "Only lists let you swap items in place.",
                        )
                        .with_types("list<T>", other.label()));
                    }
                };
                let idx_ty = self.check_expr(index)?;
                if idx_ty != DogType::Number {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "List index must be 'number'.",
                        index.span().clone(),
                        format!("Expected integer index of type 'number', received '{idx_ty}'."),
                        "Use a whole number expression like `0` or `i`.",
                        "Convert numeric text with number() first.",
                        "List slots are numbered, not named.",
                    )
                    .with_types("number", idx_ty.label()));
                }
                let val_ty = self.check_expr_with_hint(value, Some(&inner_ty))?;
                if val_ty != inner_ty {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("List '{target}' stores '{inner_ty}', cannot assign '{val_ty}'."),
                        format!("Assign a value of type '{inner_ty}'."),
                        "All elements in a DOG list share one type.",
                        "Keep the pack uniform.",
                    )
                    .with_types(inner_ty.label(), val_ty.label()));
                }
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                otherwise_branch,
                span,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != DogType::Bool {
                    return Err(DogDiagnostic::new(
                        "E022",
                        "Condition in 'if' statement must be a boolean.",
                        span.clone(),
                        format!("The condition evaluated to '{cond_ty}' instead of 'bool'."),
                        "Use a comparison like `age >= 18` or `response == \"yes\"`.",
                        "DOG does not implicitly treat numbers or strings as truthy.",
                        "Is that a yes or a maybe? Give me a bool.",
                    )
                    .with_types("bool", cond_ty.label()));
                }

                self.scopes.push(HashMap::new());
                for s in then_branch {
                    self.check_stmt(s)?;
                }
                self.scopes.pop();

                if let Some(else_stmts) = otherwise_branch {
                    self.scopes.push(HashMap::new());
                    for s in else_stmts {
                        self.check_stmt(s)?;
                    }
                    self.scopes.pop();
                }
                Ok(())
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != DogType::Bool {
                    return Err(DogDiagnostic::new(
                        "E022",
                        "Condition in 'while' loop must be a boolean.",
                        span.clone(),
                        format!("The loop condition evaluated to '{cond_ty}' instead of 'bool'."),
                        "Use a boolean expression like `count < 10`.",
                        "DOG keeps loop conditions strictly boolean.",
                        "True or false, no guessing.",
                    )
                    .with_types("bool", cond_ty.label()));
                }
                self.loop_depth += 1;
                self.scopes.push(HashMap::new());
                let res = self.check_block(body);
                self.scopes.pop();
                self.loop_depth -= 1;
                res
            }
            Stmt::Repeat { count, body, span } => {
                let count_ty = self.check_expr(count)?;
                if count_ty != DogType::Number {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("'repeat' expects a 'number' count, received '{count_ty}'."),
                        "Pass an integer expression like `repeat 5`.",
                        "Use `while` for boolean conditions.",
                        "How many times is 'text' supposed to repeat?",
                    )
                    .with_types("number", count_ty.label()));
                }
                self.loop_depth += 1;
                self.scopes.push(HashMap::new());
                let res = self.check_block(body);
                self.scopes.pop();
                self.loop_depth -= 1;
                res
            }
            Stmt::For {
                var_name,
                iterable,
                body,
                span,
            } => {
                let iter_ty = self.check_expr(iterable)?;
                let item_ty = match iter_ty {
                    DogType::List(inner) => *inner,
                    other => {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch detected.",
                            span.clone(),
                            format!("Cannot iterate over '{other}' in 'for' loop."),
                            "Iterate over a `list<T>`, e.g. `for item in [1, 2, 3]`.",
                            "For strings, split() produces a `list<text>`.",
                            "That's not a list you can walk through.",
                        )
                        .with_types("list<T>", other.label()));
                    }
                };
                self.loop_depth += 1;
                self.scopes.push(HashMap::new());
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(
                        var_name.clone(),
                        BindingInfo {
                            kind: BindingKind::Let,
                            ty: item_ty,
                        },
                    );
                }
                let res = self.check_block(body);
                self.scopes.pop();
                self.loop_depth -= 1;
                res
            }
            Stmt::FnDef {
                name,
                params,
                return_type,
                body,
                span,
            } => {
                if self.current_return_type.is_some() {
                    return Err(DogDiagnostic::new(
                        "E004",
                        "Nested 'fn' declarations are not supported.",
                        span.clone(),
                        format!("Function '{name}' was declared inside another function."),
                        "Move function definitions to the top level of your .dg file.",
                        "Top-level functions can still call each other freely.",
                        "Keep your functions out in the open.",
                    ));
                }
                let ret_ty = DogType::from_ref(return_type);
                self.current_return_type = Some(ret_ty.clone());
                self.scopes.push(HashMap::new());
                for param in params {
                    let scope = self.scopes.last_mut().expect("fn scope");
                    if scope.contains_key(&param.name) {
                        self.scopes.pop();
                        self.current_return_type = None;
                        return Err(DogDiagnostic::new(
                            "E011",
                            format!("Duplicate parameter name '{}'.", param.name),
                            param.span.clone(),
                            format!("Parameter '{}' appears more than once in '{name}()'.", param.name),
                            "Give each parameter a distinct name.",
                            "Unique parameter names keep calls unambiguous.",
                            "Two parameters, one name, zero sense.",
                        ));
                    }
                    scope.insert(
                        param.name.clone(),
                        BindingInfo {
                            kind: BindingKind::Let,
                            ty: DogType::from_ref(&param.ty),
                        },
                    );
                }
                let res = self.check_block(body);
                self.scopes.pop();
                self.current_return_type = None;
                res?;

                if ret_ty != DogType::Void && !guarantees_return(body) {
                    return Err(DogDiagnostic::new(
                        "E026",
                        format!("Function '{name}' does not return a '{ret_ty}' on all code paths."),
                        span.clone(),
                        format!("'{name}' declares return type '-> {ret_ty}', but a branch can reach 'end' without returning."),
                        format!("Ensure every branch ends with `return <{ret_ty}>`."),
                        "If using `if` without `otherwise`, add a fallback `return` at the end.",
                        "You promised a return value. Don't ghost the caller.",
                    ));
                }
                Ok(())
            }
            Stmt::Return { value, span } => {
                let Some(expected) = self.current_return_type.clone() else {
                    return Err(DogDiagnostic::new(
                        "E025",
                        "'return' used outside of a function.",
                        span.clone(),
                        "'return' can only be used inside a `fn ... end` body.",
                        "Remove `return` or wrap this logic inside a function.",
                        "Top-level scripts run sequentially from top to bottom.",
                        "Returning from nowhere to nowhere.",
                    ));
                };
                match (expected, value) {
                    (DogType::Void, None) => Ok(()),
                    (DogType::Void, Some(expr)) => {
                        let got = self.check_expr(expr)?;
                        Err(DogDiagnostic::new(
                            "E021",
                            "Void function cannot return a value.",
                            span.clone(),
                            format!("Function returns 'void', but `return` gave '{got}'."),
                            "Use bare `return`, or add `-> type` to the function signature.",
                            "Specify `-> text` (or another type) after `)` to return data.",
                            "No return type, no value.",
                        )
                        .with_types("void", got.label()))
                    }
                    (exp, None) => Err(DogDiagnostic::new(
                        "E021",
                        "Missing return value.",
                        span.clone(),
                        format!("Function expects a '{exp}' return value."),
                        format!("Write `return <{exp}>`."),
                        "Every return path must match the declared return type.",
                        "Empty-handed return.",
                    )
                    .with_types(exp.label(), "void")),
                    (exp, Some(expr)) => {
                        let got = self.check_expr_with_hint(expr, Some(&exp))?;
                        if got != exp {
                            return Err(DogDiagnostic::new(
                                "E021",
                                "Type mismatch detected in 'return'.",
                                span.clone(),
                                format!("Function must return '{exp}', but expression has type '{got}'."),
                                format!("Convert or return a '{exp}' value."),
                                "Return types are strictly enforced.",
                                "Wrong package at the return desk.",
                            )
                            .with_types(exp.label(), got.label()));
                        }
                        Ok(())
                    }
                }
            }
            Stmt::Break { span } => {
                if self.loop_depth == 0 {
                    return Err(DogDiagnostic::new(
                        "E025",
                        "'break' used outside of a loop.",
                        span.clone(),
                        "'break' can only exit a `while`, `repeat`, or `for` loop.",
                        "Remove `break` or move it inside a loop block.",
                        "Use `return` to exit a function early.",
                        "Breaking out of nothing.",
                    ));
                }
                Ok(())
            }
            Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    return Err(DogDiagnostic::new(
                        "E025",
                        "'continue' used outside of a loop.",
                        span.clone(),
                        "'continue' can only skip to the next iteration of a loop.",
                        "Remove `continue` or move it inside a `while`, `repeat`, or `for` block.",
                        "Check your `end` block boundaries.",
                        "Continuing what, exactly?",
                    ));
                }
                Ok(())
            }
            Stmt::ExprStmt { expr, .. } => {
                self.check_expr(expr)?;
                Ok(())
            }
        }
    }

    fn check_block(&mut self, stmts: &[Stmt]) -> Result<(), DogDiagnostic> {
        for s in stmts {
            self.check_stmt(s)?;
        }
        Ok(())
    }

    fn check_expr(&self, expr: &Expr) -> Result<DogType, DogDiagnostic> {
        self.check_expr_with_hint(expr, None)
    }

    fn check_expr_with_hint(
        &self,
        expr: &Expr,
        hint: Option<&DogType>,
    ) -> Result<DogType, DogDiagnostic> {
        match expr {
            Expr::StringLit { .. } => Ok(DogType::Text),
            Expr::NumberLit { .. } => Ok(DogType::Number),
            Expr::DecimalLit { .. } => Ok(DogType::Decimal),
            Expr::BoolLit { .. } => Ok(DogType::Bool),
            Expr::ListLit { elements, span } => {
                if elements.is_empty() {
                    if let Some(DogType::List(inner)) = hint {
                        return Ok(DogType::List(inner.clone()));
                    }
                    return Err(DogDiagnostic::new(
                        "E024",
                        "Cannot infer element type of empty list '[]'.",
                        span.clone(),
                        "Strict typing requires every list to have a known element type.",
                        "Add a type annotation, e.g. `var items: list<text> = []`.",
                        "Non-empty lists like `[1, 2, 3]` infer their type automatically.",
                        "An empty box with no label makes me nervous.",
                    ));
                }
                let inner_hint = match hint {
                    Some(DogType::List(inner)) => Some(inner.as_ref()),
                    _ => None,
                };
                let first_ty = self.check_expr_with_hint(&elements[0], inner_hint)?;
                for elem in &elements[1..] {
                    let elem_ty = self.check_expr_with_hint(elem, Some(&first_ty))?;
                    if elem_ty != first_ty {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Mixed element types in list literal.",
                            elem.span().clone(),
                            format!("First element has type '{first_ty}', but found '{elem_ty}'."),
                            "Keep all elements in a list the same type.",
                            "Convert elements explicitly with text(), number(), or decimal().",
                            "Pick one type per pack.",
                        )
                        .with_types(first_ty.label(), elem_ty.label()));
                    }
                }
                Ok(DogType::List(Box::new(first_ty)))
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                let target_ty = self.check_expr(target)?;
                let idx_ty = self.check_expr(index)?;
                if idx_ty != DogType::Number {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Index must be 'number'.",
                        index.span().clone(),
                        format!("Indexing expects 'number', received '{idx_ty}'."),
                        "Pass an integer expression inside `[...]`.",
                        "Lists and text are zero-indexed.",
                        "Words aren't array indices.",
                    )
                    .with_types("number", idx_ty.label()));
                }
                match target_ty {
                    DogType::List(inner) => Ok(*inner),
                    DogType::Text => Ok(DogType::Text),
                    other => Err(DogDiagnostic::new(
                        "E021",
                        "Cannot index into value.",
                        span.clone(),
                        format!("Type '{other}' does not support `[...]` indexing."),
                        "Index only into `list<T>` or `text` values.",
                        "Check the expression before `[`.",
                        "There's nothing inside that to index.",
                    )
                    .with_types("list<T> | text", other.label())),
                }
            }
            Expr::Identifier { name, span } => self
                .lookup(name)
                .map(|b| b.ty.clone())
                .ok_or_else(|| {
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{name}'."),
                        span.clone(),
                        format!("No variable named '{name}' exists in the current scope."),
                        format!("Declare `{name}` with `let`, `var`, or `const` before reading it."),
                        "Check spelling and scope boundaries.",
                        "I sniffed everywhere and couldn't find that variable.",
                    )
                }),
            Expr::Unary { op, expr, span } => {
                let inner = self.check_expr(expr)?;
                match op {
                    UnaryOp::Neg => match inner {
                        DogType::Number | DogType::Decimal => Ok(inner),
                        _ => Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch detected.",
                            span.clone(),
                            format!("Cannot negate a value of type '{inner}'."),
                            "Unary '-' only applies to 'number' or 'decimal'.",
                            "Convert text first using number() or decimal().",
                            "What does negative text even mean?",
                        )
                        .with_types("number | decimal", inner.label())),
                    },
                    UnaryOp::Not => {
                        if inner == DogType::Bool {
                            Ok(DogType::Bool)
                        } else {
                            Err(DogDiagnostic::new(
                                "E021",
                                "Type mismatch detected.",
                                span.clone(),
                                format!("Operator 'not' requires 'bool', received '{inner}'."),
                                "Apply `not` to a boolean expression like `not (age < 18)`.",
                                "DOG never coerces values to boolean implicitly.",
                                "What is the opposite of a number?",
                            )
                            .with_types("bool", inner.label()))
                        }
                    }
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                let lt = self.check_expr(left)?;
                let rt = self.check_expr(right)?;
                self.check_binary(lt, *op, rt, span)
            }
            Expr::Call { callee, args, span } => self.check_call(callee, args, span),
        }
    }

    fn check_binary(
        &self,
        lt: DogType,
        op: BinOp,
        rt: DogType,
        span: &Span,
    ) -> Result<DogType, DogDiagnostic> {
        match op {
            BinOp::Add => {
                if lt == DogType::Text && rt == DogType::Text {
                    Ok(DogType::Text)
                } else if (lt == DogType::Number && rt == DogType::Number)
                    || (lt == DogType::Decimal && rt == DogType::Decimal)
                {
                    Ok(lt)
                } else {
                    Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("You're trying to add {rt} to {lt}. DOG forbids implicit conversions."),
                        "Use number(), decimal(), or text() so both sides match.",
                        "Keep your types consistent.",
                        "Math wasn't supposed to be this difficult.",
                    )
                    .with_types(lt.label(), rt.label()))
                }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div => {
                if (lt == DogType::Number && rt == DogType::Number)
                    || (lt == DogType::Decimal && rt == DogType::Decimal)
                {
                    Ok(lt)
                } else {
                    Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("Arithmetic requires matching numeric types, found '{lt}' and '{rt}'."),
                        "Convert operands using number() or decimal() so both match.",
                        "Strict typing prevents silent precision surprises.",
                        "Apples minus oranges equals a type error.",
                    )
                    .with_types(lt.label(), rt.label()))
                }
            }
            BinOp::Mod => {
                if lt == DogType::Number && rt == DogType::Number {
                    Ok(DogType::Number)
                } else {
                    Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("Operator '%' requires 'number' operands, got '{lt}' and '{rt}'."),
                        "Use whole 'number' values with '%'.",
                        "Modulo returns the integer remainder.",
                        "Remainders belong to whole numbers.",
                    )
                    .with_types("number", format!("{lt} % {rt}")))
                }
            }
            BinOp::Eq | BinOp::NotEq => {
                if lt == rt && lt != DogType::Void {
                    Ok(DogType::Bool)
                } else {
                    Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("Cannot compare '{lt}' with '{rt}'."),
                        "Convert one side explicitly so both sides have identical types.",
                        "Equality in DOG never performs sneaky type coercion.",
                        "Comparing text and numbers? Bold strategy.",
                    )
                    .with_types(lt.label(), rt.label()))
                }
            }
            BinOp::Gt | BinOp::Lt | BinOp::GtEq | BinOp::LtEq => {
                if (lt == DogType::Number && rt == DogType::Number)
                    || (lt == DogType::Decimal && rt == DogType::Decimal)
                {
                    Ok(DogType::Bool)
                } else {
                    Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("Ordering comparisons require matching 'number' or 'decimal' operands, got '{lt}' and '{rt}'."),
                        "Use number() or decimal() before comparing.",
                        "Convert user input from answer() with number() first.",
                        "Text doesn't know how big it is.",
                    )
                    .with_types("number | decimal", format!("{lt} vs {rt}")))
                }
            }
            BinOp::Or | BinOp::And => {
                let op_name = if op == BinOp::Or { "or" } else { "and" };
                if lt == DogType::Bool && rt == DogType::Bool {
                    Ok(DogType::Bool)
                } else {
                    Err(DogDiagnostic::new(
                        "E023",
                        format!("Operator '{op_name}' requires boolean operands."),
                        span.clone(),
                        format!("Found '{lt}' and '{rt}' around '{op_name}'."),
                        format!("Write explicit comparisons on both sides around '{op_name}'."),
                        "Each side of logical operators must evaluate to true or false.",
                        "Logical operators still need logical types.",
                    )
                    .with_types("bool", format!("{lt} {op_name} {rt}")))
                }
            }
        }
    }

    fn check_call(&self, callee: &str, args: &[Expr], span: &Span) -> Result<DogType, DogDiagnostic> {
        // Special-case mutating list built-ins so we enforce `var` binding!
        if callee == "push" {
            if args.len() != 2 {
                return Err(arity_error(callee, 2, args.len(), span));
            }
            let inner_ty = self.require_mutable_list_arg(&args[0], callee)?;
            let item_ty = self.check_expr_with_hint(&args[1], Some(&inner_ty))?;
            if item_ty != inner_ty {
                return Err(DogDiagnostic::new(
                    "E021",
                    "Type mismatch detected.",
                    args[1].span().clone(),
                    format!("Cannot push '{item_ty}' into 'list<{inner_ty}>'."),
                    format!("Pass a value of type '{inner_ty}'."),
                    "Lists in DOG are strictly homogeneous.",
                    "Don't sneak cats into the dog pack.",
                )
                .with_types(inner_ty.label(), item_ty.label()));
            }
            return Ok(DogType::Void);
        }
        if callee == "pop" {
            if args.len() != 1 {
                return Err(arity_error(callee, 1, args.len(), span));
            }
            return self.require_mutable_list_arg(&args[0], callee);
        }

        let mut arg_types = Vec::with_capacity(args.len());
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        match callee {
            "say" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] == DogType::Void {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Cannot pass 'void' to say().",
                        span.clone(),
                        "say() requires a printable value.",
                        "Pass text, number, decimal, bool, or list to say().",
                        "Do not nest say(say(...)).",
                        "Saying nothing out loud is called silence.",
                    )
                    .with_types("printable value", "void"));
                }
                Ok(DogType::Void)
            }
            "answer" => {
                if arg_types.len() > 1 {
                    return Err(DogDiagnostic::new(
                        "E014",
                        "Too many arguments to answer().",
                        span.clone(),
                        format!("answer() takes 0 or 1 text prompt, received {}.", arg_types.len()),
                        "Call `answer(\"Prompt: \")` or `answer()`.",
                        "Keep prompts concise.",
                        "One question at a time!",
                    ));
                }
                if let Some(t) = arg_types.first() {
                    if *t != DogType::Text {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch detected.",
                            span.clone(),
                            format!("answer() prompt must be 'text', received '{t}'."),
                            "Pass a quoted string prompt to answer().",
                            "answer() always returns 'text'.",
                            "Ask your question in words.",
                        )
                        .with_types("text", t.label()));
                    }
                }
                Ok(DogType::Text)
            }
            "lower" | "upper" | "trim" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("{callee}() expects 'text', received '{}'.", arg_types[0]),
                        format!("Pass a 'text' value to {callee}()."),
                        "String helpers only apply to text.",
                        "Numbers don't have whitespace or uppercase.",
                    )
                    .with_types("text", arg_types[0].label()));
                }
                Ok(DogType::Text)
            }
            "contains" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        "contains() expects two 'text' arguments: contains(haystack, needle).",
                        "Convert values with text() if needed.",
                        "Returns bool.",
                        "Searching text inside non-text?",
                    )
                    .with_types("text, text", format!("{}, {}", arg_types[0], arg_types[1])));
                }
                Ok(DogType::Bool)
            }
            "split" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        "split() expects (text, text).",
                        "Pass the source text and separator text.",
                        "Returns list<text>.",
                        "Can't split what isn't text.",
                    )
                    .with_types("text, text", format!("{}, {}", arg_types[0], arg_types[1])));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "join" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let expected_list = DogType::List(Box::new(DogType::Text));
                if arg_types[0] != expected_list || arg_types[1] != DogType::Text {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        "join() expects (list<text>, text).",
                        "Pass a list of text and a separator string.",
                        "Returns text.",
                        "Join works on text lists.",
                    )
                    .with_types("list<text>, text", format!("{}, {}", arg_types[0], arg_types[1])));
                }
                Ok(DogType::Text)
            }
            "len" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Text | DogType::List(_) => Ok(DogType::Number),
                    other => Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("len() expects 'text' or 'list<T>', received '{other}'."),
                        "Pass text or a list to len().",
                        "len() returns a whole number.",
                        "How long is a boolean?",
                    )
                    .with_types("text | list<T>", other.label())),
                }
            }
            "text" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] == DogType::Void {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Cannot convert 'void' using text().",
                        span.clone(),
                        "void carries no value to convert.",
                        "Pass text, number, decimal, bool, or list<T> to text().",
                        "Explicit conversion makes string concatenation safe.",
                        "Nothingness doesn't stringify well.",
                    )
                    .with_types("non-void", "void"));
                }
                Ok(DogType::Text)
            }
            "number" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Text | DogType::Number | DogType::Decimal => Ok(DogType::Number),
                    other => Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("Cannot convert '{other}' using number()."),
                        "Pass text, number, or decimal to number().",
                        "Booleans and void cannot become numbers.",
                        "Nice try, still not a number.",
                    )
                    .with_types("text | number | decimal", other.label())),
                }
            }
            "decimal" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Text | DogType::Number | DogType::Decimal => Ok(DogType::Decimal),
                    other => Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("Cannot convert '{other}' using decimal()."),
                        "Pass text, number, or decimal to decimal().",
                        "Use decimal() when you need fractional precision.",
                        "Can't turn that into a decimal.",
                    )
                    .with_types("text | number | decimal", other.label())),
                }
            }
            "abs" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Number => Ok(DogType::Number),
                    DogType::Decimal => Ok(DogType::Decimal),
                    other => Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("abs() expects 'number' or 'decimal', received '{other}'."),
                        "Pass a numeric value to abs().",
                        "Returns the same numeric type.",
                        "Absolute text isn't a thing.",
                    )
                    .with_types("number | decimal", other.label())),
                }
            }
            "min" | "max" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] == arg_types[1]
                    && (arg_types[0] == DogType::Number || arg_types[0] == DogType::Decimal)
                {
                    Ok(arg_types[0].clone())
                } else {
                    Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("{callee}() expects two matching 'number' or 'decimal' values."),
                        "Ensure both arguments have identical numeric types.",
                        "Convert with number() or decimal() first.",
                        "Pick one numeric type to compare.",
                    )
                    .with_types("T, T (number | decimal)", format!("{}, {}", arg_types[0], arg_types[1])))
                }
            }
            "assert" => {
                if arg_types.is_empty() || arg_types.len() > 2 {
                    return Err(DogDiagnostic::new(
                        "E014",
                        "Incorrect argument count for 'assert()'.",
                        span.clone(),
                        format!("assert() takes 1 or 2 arguments (cond, msg?), got {}.", arg_types.len()),
                        "Write `assert(x == 5)` or `assert(x == 5, \"x must be 5\")`.",
                        "Used in tests and invariants.",
                        "Assert yourself properly.",
                    ));
                }
                if arg_types[0] != DogType::Bool {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch detected.",
                        span.clone(),
                        format!("assert() condition must be 'bool', received '{}'.", arg_types[0]),
                        "Pass a boolean comparison as the first argument.",
                        "Example: `assert(age >= 18)`.",
                        "Assert true or false, not guesses.",
                    )
                    .with_types("bool", arg_types[0].label()));
                }
                if let Some(msg_ty) = arg_types.get(1) {
                    if *msg_ty != DogType::Text {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch detected.",
                            span.clone(),
                            format!("assert() message must be 'text', received '{msg_ty}'."),
                            "Pass a string message as the second argument.",
                            "Use text() to format values in assertion messages.",
                            "Explain failures in plain text.",
                        )
                        .with_types("text", msg_ty.label()));
                    }
                }
                Ok(DogType::Void)
            }
            other => {
                let Some(sig) = self.functions.get(other) else {
                    return Err(DogDiagnostic::new(
                        "E013",
                        format!("Unknown function '{callee}'."),
                        span.clone(),
                        format!("No built-in or declared function named '{callee}' exists."),
                        "Check spelling or define the function with `fn`.",
                        "Top-level functions are visible throughout the file.",
                        "I looked everywhere for that function. Nobody home.",
                    ));
                };
                if args.len() != sig.params.len() {
                    return Err(arity_error(callee, sig.params.len(), args.len(), span));
                }
                for (idx, (got, exp)) in arg_types.iter().zip(sig.params.iter()).enumerate() {
                    if got != exp {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Type mismatch in function call.",
                            args[idx].span().clone(),
                            format!("Argument {} of '{callee}()' expects '{exp}', received '{got}'.", idx + 1),
                            format!("Pass a '{exp}' value here."),
                            "DOG checks every function argument strictly.",
                            "Wrong bone for that trick.",
                        )
                        .with_types(exp.label(), got.label()));
                    }
                }
                Ok(sig.return_type.clone())
            }
        }
    }

    fn require_mutable_list_arg(&self, expr: &Expr, callee: &str) -> Result<DogType, DogDiagnostic> {
        let Expr::Identifier { name, span } = expr else {
            return Err(DogDiagnostic::new(
                "E004",
                format!("First argument to '{callee}()' must be a mutable list variable."),
                expr.span().clone(),
                format!("'{callee}()' mutates a `var` list variable in place."),
                format!("Declare `var items = [...]` and pass `items` to {callee}()."),
                "Temporary list literals cannot be mutated in place.",
                "Give that list a `var` name before modifying it.",
            ));
        };
        let info = self.lookup(name).ok_or_else(|| {
            DogDiagnostic::new(
                "E010",
                format!("Undefined variable '{name}'."),
                span.clone(),
                format!("No variable named '{name}' exists in scope."),
                format!("Declare `var {name} = [...]` first."),
                "Check spelling.",
                "Where is that list?",
            )
        })?;
        if info.kind != BindingKind::Var {
            return Err(DogDiagnostic::new(
                "E012",
                format!("Cannot call '{callee}()' on immutable binding '{name}'."),
                span.clone(),
                format!("'{name}' was not declared with `var`."),
                format!("Change `{name}` to `var {name} = ...` to mutate it."),
                "`let` and `const` lists stay fixed.",
                "Immutable lists refuse to grow or shrink.",
            ));
        }
        match &info.ty {
            DogType::List(inner) => Ok((**inner).clone()),
            other => Err(DogDiagnostic::new(
                "E021",
                "Type mismatch detected.",
                span.clone(),
                format!("'{callee}()' expects a 'list<T>' variable, received '{other}'."),
                format!("Pass a `var` list to {callee}()."),
                "Only lists support push() and pop().",
                "Can't push items onto a scalar.",
            )
            .with_types("list<T>", other.label())),
        }
    }

    fn lookup(&self, name: &str) -> Option<&BindingInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }
}

fn guarantees_return(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Return { .. } => return true,
            Stmt::If {
                then_branch,
                otherwise_branch: Some(else_branch),
                ..
            } => {
                if guarantees_return(then_branch) && guarantees_return(else_branch) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn arity_error(callee: &str, expected: usize, got: usize, span: &Span) -> DogDiagnostic {
    DogDiagnostic::new(
        "E014",
        format!("Incorrect argument count for '{callee}()'."),
        span.clone(),
        format!("'{callee}()' expects {expected} argument(s), but received {got}."),
        format!("Pass exactly {expected} argument(s) to {callee}()."),
        "Check commas and parentheses around arguments.",
        "Count on your paws and try again.",
    )
}
