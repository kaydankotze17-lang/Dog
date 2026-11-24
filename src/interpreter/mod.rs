use crate::ast::{BinOp, BindingKind, Expr, InterpPart, Param, Program, Span, Stmt, UnaryOp};
use crate::checker::TypeChecker;
use crate::diagnostics::DogDiagnostic;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::runtime::{Environment, Value};
use crate::stdlib::{call_builtin, InterpreterIo, StdIo};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_CALL_DEPTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStepDecision {
    Continue,
    Abort,
}

pub trait ExecObserver {
    fn on_before_stmt(&mut self, stmt: &Stmt, env: &Environment) -> DebugStepDecision;
}

pub fn stmt_span(stmt: &Stmt) -> &Span {
    match stmt {
        Stmt::Use { span, .. }
        | Stmt::StructDef { span, .. }
        | Stmt::EnumDef { span, .. }
        | Stmt::Binding { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::CompoundAssign { span, .. }
        | Stmt::IndexAssign { span, .. }
        | Stmt::FieldAssign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::TryCatch { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Repeat { span, .. }
        | Stmt::For { span, .. }
        | Stmt::FnDef { span, .. }
        | Stmt::StyleDef { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Break { span }
        | Stmt::Continue { span }
        | Stmt::ExprStmt { span, .. } => span,
    }
}

#[derive(Debug, Clone)]
struct FnDefRuntime {
    params: Vec<Param>,
    body: Vec<Stmt>,
}

enum ExecFlow {
    Next,
    Return(Value),
    Break,
    Continue,
}

pub struct Interpreter<'a> {
    env: Environment,
    functions: HashMap<String, FnDefRuntime>,
    enum_names: HashSet<String>,
    io: &'a mut dyn InterpreterIo,
    observer: Option<&'a mut dyn ExecObserver>,
    call_depth: usize,
    call_stack: Vec<String>,
}

impl<'a> Interpreter<'a> {
    pub fn new(io: &'a mut dyn InterpreterIo) -> Self {
        crate::term::reset_session_style_to(crate::config::DogConfig::load().term_style());
        crate::web::reset_web_runtime();
        crate::stdlib::reset_headless_web();
        Self {
            env: Environment::new(),
            functions: HashMap::new(),
            enum_names: HashSet::new(),
            io,
            observer: None,
            call_depth: 0,
            call_stack: Vec::new(),
        }
    }

    pub fn with_observer(
        io: &'a mut dyn InterpreterIo,
        observer: &'a mut dyn ExecObserver,
    ) -> Self {
        crate::term::reset_session_style_to(crate::config::DogConfig::load().term_style());
        crate::web::reset_web_runtime();
        crate::stdlib::reset_headless_web();
        Self {
            env: Environment::new(),
            functions: HashMap::new(),
            enum_names: HashSet::new(),
            io,
            observer: Some(observer),
            call_depth: 0,
            call_stack: Vec::new(),
        }
    }

    pub fn eval_program(&mut self, program: &Program) -> Result<(), DogDiagnostic> {
        for stmt in &program.statements {
            match stmt {
                Stmt::FnDef {
                    name, params, body, ..
                } => {
                    self.functions.insert(
                        name.clone(),
                        FnDefRuntime {
                            params: params.clone(),
                            body: body.clone(),
                        },
                    );
                }
                Stmt::EnumDef { name, .. } => {
                    self.enum_names.insert(name.clone());
                }
                _ => {}
            }
        }

        for stmt in &program.statements {
            if matches!(
                stmt,
                Stmt::FnDef { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } | Stmt::Use { .. }
            ) {
                continue;
            }
            match self.eval_stmt(stmt)? {
                ExecFlow::Next => {}
                ExecFlow::Return(_) | ExecFlow::Break | ExecFlow::Continue => break,
            }
        }
        Ok(())
    }

    pub fn top_symbols(&self) -> Vec<(String, String)> {
        self.env.snapshot_top_symbols()
    }

    fn eval_stmt(&mut self, stmt: &Stmt) -> Result<ExecFlow, DogDiagnostic> {
        if !matches!(
            stmt,
            Stmt::FnDef { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } | Stmt::Use { .. }
        ) {
            if let Some(obs) = self.observer.as_deref_mut() {
                if obs.on_before_stmt(stmt, &self.env) == DebugStepDecision::Abort {
                    return Err(DogDiagnostic::new(
                        "E035",
                        "Execution aborted by debugger.",
                        stmt_span(stmt).clone(),
                        "Debugger session exited before program completion.",
                        "Run `dog run` to execute normally.",
                        "Debugger stopped cleanly.",
                        "Stopped on user command.",
                    ));
                }
            }
        }
        match stmt {
            Stmt::FnDef { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } | Stmt::Use { .. } => {
                Ok(ExecFlow::Next)
            }
            Stmt::Binding {
                kind, name, value, ..
            } => {
                let val = self.eval_expr(value)?;
                self.env.define(name.clone(), *kind, val);
                Ok(ExecFlow::Next)
            }
            Stmt::Assign { name, value, span } => {
                let val = self.eval_expr(value)?;
                self.env
                    .assign(name, val)
                    .map_err(|_| immutable_err(name, span))?;
                Ok(ExecFlow::Next)
            }
            Stmt::CompoundAssign {
                name,
                op,
                value,
                span,
            } => {
                let rv = self.eval_expr(value)?;
                let cur = self
                    .env
                    .get_mut_var(name)
                    .map_err(|_| immutable_err(name, span))?;
                match (op, cur, rv) {
                    (BinOp::Add, Value::Text(a), Value::Text(b)) => a.push_str(&b),
                    (BinOp::Add, Value::Number(a), Value::Number(b)) => *a = a.wrapping_add(b),
                    (BinOp::Add, Value::Decimal(a), Value::Decimal(b)) => *a += b,
                    (BinOp::Sub, Value::Number(a), Value::Number(b)) => *a = a.wrapping_sub(b),
                    (BinOp::Sub, Value::Decimal(a), Value::Decimal(b)) => *a -= b,
                    (BinOp::Mul, Value::Number(a), Value::Number(b)) => *a = a.wrapping_mul(b),
                    (BinOp::Mul, Value::Decimal(a), Value::Decimal(b)) => *a *= b,
                    (BinOp::Div, Value::Number(_), Value::Number(0)) => return Err(div_zero(span)),
                    (BinOp::Div, Value::Number(a), Value::Number(b)) => *a /= b,
                    (BinOp::Div, Value::Decimal(_), Value::Decimal(b)) if b == 0.0 => {
                        return Err(div_zero(span));
                    }
                    (BinOp::Div, Value::Decimal(a), Value::Decimal(b)) => *a /= b,
                    (BinOp::Mod, Value::Number(_), Value::Number(0)) => return Err(div_zero(span)),
                    (BinOp::Mod, Value::Number(a), Value::Number(b)) => *a %= b,
                    _ => unreachable!(),
                }
                Ok(ExecFlow::Next)
            }
            Stmt::IndexAssign {
                target,
                index,
                value,
                span,
            } => {
                let idx_val = self.eval_expr(index)?;
                let val = self.eval_expr(value)?;
                let Value::Number(raw_idx) = idx_val else {
                    unreachable!();
                };
                let list_val = self
                    .env
                    .get_mut_var(target)
                    .map_err(|_| immutable_err(target, span))?;
                let Value::List(items) = list_val else {
                    unreachable!();
                };
                if raw_idx < 0 || (raw_idx as usize) >= items.len() {
                    return Err(index_bounds_err(raw_idx, items.len(), span));
                }
                items[raw_idx as usize] = val;
                Ok(ExecFlow::Next)
            }
            Stmt::FieldAssign {
                target,
                field,
                value,
                span,
            } => {
                let val = self.eval_expr(value)?;
                let struct_val = self
                    .env
                    .get_mut_var(target)
                    .map_err(|_| immutable_err(target, span))?;
                let Value::Struct { fields, .. } = struct_val else {
                    unreachable!();
                };
                fields.insert(field.clone(), val);
                Ok(ExecFlow::Next)
            }
            Stmt::If {
                condition,
                then_branch,
                otherwise_branch,
                ..
            } => {
                let cond = self.eval_expr(condition)?;
                if matches!(cond, Value::Bool(true)) {
                    self.env.push_scope();
                    let res = self.eval_block(then_branch);
                    self.env.pop_scope();
                    res
                } else if let Some(else_stmts) = otherwise_branch {
                    self.env.push_scope();
                    let res = self.eval_block(else_stmts);
                    self.env.pop_scope();
                    res
                } else {
                    Ok(ExecFlow::Next)
                }
            }
            Stmt::Match {
                subject,
                cases,
                otherwise_branch,
                ..
            } => {
                let subj_val = self.eval_expr(subject)?;
                for case in cases {
                    let pat_val = self.eval_expr(&case.pattern)?;
                    if pat_val == subj_val {
                        self.env.push_scope();
                        let res = self.eval_block(&case.body);
                        self.env.pop_scope();
                        return res;
                    }
                }
                if let Some(else_stmts) = otherwise_branch {
                    self.env.push_scope();
                    let res = self.eval_block(else_stmts);
                    self.env.pop_scope();
                    return res;
                }
                Ok(ExecFlow::Next)
            }
            Stmt::TryCatch {
                try_body,
                err_name,
                catch_body,
                ..
            } => {
                self.env.push_scope();
                let res = self.eval_block(try_body);
                self.env.pop_scope();
                match res {
                    Ok(flow) => Ok(flow),
                    Err(diag) => {
                        self.env.push_scope();
                        self.env
                            .define(err_name.clone(), BindingKind::Let, Value::Text(diag.title));
                        let catch_res = self.eval_block(catch_body);
                        self.env.pop_scope();
                        catch_res
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                loop {
                    let cond = self.eval_expr(condition)?;
                    if !matches!(cond, Value::Bool(true)) {
                        break;
                    }
                    self.env.push_scope();
                    let flow = self.eval_block(body);
                    self.env.pop_scope();
                    match flow? {
                        ExecFlow::Next | ExecFlow::Continue => continue,
                        ExecFlow::Break => break,
                        ret @ ExecFlow::Return(_) => return Ok(ret),
                    }
                }
                Ok(ExecFlow::Next)
            }
            Stmt::Repeat { count, body, span } => {
                let Value::Number(n) = self.eval_expr(count)? else {
                    unreachable!();
                };
                if n < 0 {
                    return Err(DogDiagnostic::new(
                        "E030",
                        format!("Negative repeat count ({n})."),
                        span.clone(),
                        "A 'repeat' loop count cannot be negative.",
                        "Use a count >= 0.",
                        "Zero skips the loop cleanly.",
                        "I can't bark backwards three times.",
                    ));
                }
                for _ in 0..n {
                    self.env.push_scope();
                    let flow = self.eval_block(body);
                    self.env.pop_scope();
                    match flow? {
                        ExecFlow::Next | ExecFlow::Continue => continue,
                        ExecFlow::Break => break,
                        ret @ ExecFlow::Return(_) => return Ok(ret),
                    }
                }
                Ok(ExecFlow::Next)
            }
            Stmt::For {
                var_name,
                iterable,
                body,
                ..
            } => {
                let Value::List(items) = self.eval_expr(iterable)? else {
                    unreachable!();
                };
                for item in items {
                    self.env.push_scope();
                    self.env.define(var_name.clone(), BindingKind::Let, item);
                    let flow = self.eval_block(body);
                    self.env.pop_scope();
                    match flow? {
                        ExecFlow::Next | ExecFlow::Continue => continue,
                        ExecFlow::Break => break,
                        ret @ ExecFlow::Return(_) => return Ok(ret),
                    }
                }
                Ok(ExecFlow::Next)
            }
            Stmt::StyleDef {
                selector,
                rules,
                variants,
                ..
            } => {
                let sel = self.eval_expr(selector)?.to_string();
                let mut pairs = Vec::with_capacity(rules.len());
                for r in rules {
                    let v = self.eval_expr(&r.value)?.to_string();
                    pairs.push((r.prop.clone(), v));
                }
                crate::web::register_style_rule(None, &sel, pairs);
                for var in variants {
                    let mut v_pairs = Vec::with_capacity(var.rules.len());
                    for r in &var.rules {
                        let v = self.eval_expr(&r.value)?.to_string();
                        v_pairs.push((r.prop.clone(), v));
                    }
                    if let Some(q) = var.condition.strip_prefix("@media ") {
                        crate::web::register_style_rule(Some(q.to_string()), &sel, v_pairs);
                    } else if var.condition.starts_with(':') {
                        let target_sel = format!("{sel}{}", var.condition);
                        crate::web::register_style_rule(None, &target_sel, v_pairs);
                    } else {
                        crate::web::register_style_rule(None, &sel, v_pairs);
                    }
                }
                Ok(ExecFlow::Next)
            }
            Stmt::Return { value, .. } => {
                let val = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Void,
                };
                Ok(ExecFlow::Return(val))
            }
            Stmt::Break { .. } => Ok(ExecFlow::Break),
            Stmt::Continue { .. } => Ok(ExecFlow::Continue),
            Stmt::ExprStmt { expr, .. } => {
                self.eval_expr(expr)?;
                Ok(ExecFlow::Next)
            }
        }
    }

    fn eval_block(&mut self, stmts: &[Stmt]) -> Result<ExecFlow, DogDiagnostic> {
        for stmt in stmts {
            let flow = self.eval_stmt(stmt)?;
            if !matches!(flow, ExecFlow::Next) {
                return Ok(flow);
            }
        }
        Ok(ExecFlow::Next)
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, DogDiagnostic> {
        match expr {
            Expr::StringLit { value, .. } => Ok(Value::Text(value.clone())),
            Expr::InterpString { parts, .. } => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        InterpPart::Literal(s) => out.push_str(s),
                        InterpPart::Expr(inner) => {
                            let val = self.eval_expr(inner)?;
                            out.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::Text(out))
            }
            Expr::NumberLit { value, .. } => Ok(Value::Number(*value)),
            Expr::DecimalLit { value, .. } => Ok(Value::Decimal(*value)),
            Expr::BoolLit { value, .. } => Ok(Value::Bool(*value)),
            Expr::ListLit { elements, .. } => {
                let mut items = Vec::with_capacity(elements.len());
                for elem in elements {
                    items.push(self.eval_expr(elem)?);
                }
                Ok(Value::List(items))
            }
            Expr::MapLit { entries, .. } => {
                let mut map = Vec::with_capacity(entries.len());
                for (k, v) in entries {
                    let kv = self.eval_expr(k)?;
                    let vv = self.eval_expr(v)?;
                    if let Some((_, slot)) = map.iter_mut().find(|(ek, _)| ek == &kv) {
                        *slot = vv;
                    } else {
                        map.push((kv, vv));
                    }
                }
                Ok(Value::Map(map))
            }
            Expr::StructInit { name, fields, .. } => {
                let mut map = BTreeMap::new();
                for (fname, fexpr) in fields {
                    map.insert(fname.clone(), self.eval_expr(fexpr)?);
                }
                Ok(Value::Struct {
                    type_name: name.clone(),
                    fields: map,
                })
            }
            Expr::FieldAccess { target, field, .. } => {
                if let Expr::Identifier { name, .. } = target.as_ref() {
                    if self.enum_names.contains(name) && self.env.get(name).is_none() {
                        return Ok(Value::Enum {
                            type_name: name.clone(),
                            variant: field.clone(),
                        });
                    }
                }
                let val = self.eval_expr(target)?;
                match val {
                    Value::Struct { fields, .. } => Ok(fields.get(field).cloned().unwrap_or(Value::Void)),
                    _ => unreachable!("Type checker validates field access"),
                }
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                let target_val = self.eval_expr(target)?;
                let idx_val = self.eval_expr(index)?;
                let Value::Number(idx) = idx_val else {
                    unreachable!();
                };
                match target_val {
                    Value::List(items) => {
                        if idx < 0 || (idx as usize) >= items.len() {
                            return Err(index_bounds_err(idx, items.len(), span));
                        }
                        Ok(items[idx as usize].clone())
                    }
                    Value::Text(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        if idx < 0 || (idx as usize) >= chars.len() {
                            return Err(index_bounds_err(idx, chars.len(), span));
                        }
                        Ok(Value::Text(chars[idx as usize].to_string()))
                    }
                    _ => unreachable!(),
                }
            }
            Expr::Identifier { name, span } => self.env.get(name).cloned().ok_or_else(|| {
                DogDiagnostic::new(
                    "E010",
                    format!("Undefined variable '{name}'."),
                    span.clone(),
                    "Variable was not found in runtime environment.",
                    "Declare it before use.",
                    "Check variable scope.",
                    "Gone missing.",
                )
            }),
            Expr::Unary { op, expr, .. } => {
                let v = self.eval_expr(expr)?;
                match (op, v) {
                    (UnaryOp::Neg, Value::Number(n)) => Ok(Value::Number(n.wrapping_neg())),
                    (UnaryOp::Neg, Value::Decimal(d)) => Ok(Value::Decimal(-d)),
                    (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    _ => unreachable!(),
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                if *op == BinOp::Or {
                    let lv = self.eval_expr(left)?;
                    if matches!(lv, Value::Bool(true)) {
                        return Ok(Value::Bool(true));
                    }
                    return self.eval_expr(right);
                }
                if *op == BinOp::And {
                    let lv = self.eval_expr(left)?;
                    if matches!(lv, Value::Bool(false)) {
                        return Ok(Value::Bool(false));
                    }
                    return self.eval_expr(right);
                }

                let lv = self.eval_expr(left)?;
                let rv = self.eval_expr(right)?;
                match (op, lv, rv) {
                    (BinOp::Add, Value::Text(a), Value::Text(b)) => Ok(Value::Text(format!("{a}{b}"))),
                    (BinOp::Add, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.wrapping_add(b))),
                    (BinOp::Add, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a + b)),
                    (BinOp::Sub, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.wrapping_sub(b))),
                    (BinOp::Sub, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a - b)),
                    (BinOp::Mul, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.wrapping_mul(b))),
                    (BinOp::Mul, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a * b)),
                    (BinOp::Div, Value::Number(_), Value::Number(0)) => Err(div_zero(span)),
                    (BinOp::Div, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a / b)),
                    (BinOp::Div, Value::Decimal(_), Value::Decimal(b)) if b == 0.0 => Err(div_zero(span)),
                    (BinOp::Div, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a / b)),
                    (BinOp::Mod, Value::Number(_), Value::Number(0)) => Err(div_zero(span)),
                    (BinOp::Mod, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a % b)),
                    (BinOp::Eq, a, b) => Ok(Value::Bool(a == b)),
                    (BinOp::NotEq, a, b) => Ok(Value::Bool(a != b)),
                    (BinOp::Gt, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),
                    (BinOp::Gt, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Bool(a > b)),
                    (BinOp::Lt, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),
                    (BinOp::Lt, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Bool(a < b)),
                    (BinOp::GtEq, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)),
                    (BinOp::GtEq, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Bool(a >= b)),
                    (BinOp::LtEq, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)),
                    (BinOp::LtEq, Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Bool(a <= b)),
                    _ => unreachable!(),
                }
            }
            Expr::Call { callee, args, span } => self.eval_call(callee, args, span),
            Expr::NamespaceCall {
                module,
                func,
                args,
                span,
            } => {
                let canonical = crate::stdlib::modules::resolve_std_call(module, func).map_err(|msg| {
                    DogDiagnostic::new(
                        "E013",
                        msg,
                        span.clone(),
                        "Standard library module calls use `std.<module>.<fn>(...)`.",
                        "Run `dog docs std` to inspect valid standard modules.",
                        "Check module and function name spelling.",
                        "Wrong aisle in the standard library.",
                    )
                })?;
                self.eval_call(canonical, args, span)
            }
        }
    }

    fn eval_call(&mut self, callee: &str, args: &[Expr], span: &Span) -> Result<Value, DogDiagnostic> {
                if callee == "push" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let item = self.eval_expr(&args[1])?;
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    items.push(item);
                    return Ok(Value::Void);
                }
                if callee == "pop" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    return items.pop().ok_or_else(|| {
                        DogDiagnostic::new(
                            "E034",
                            format!("Cannot pop() from empty list '{name}'."),
                            span.clone(),
                            "The list has zero elements.",
                            "Check `if len(items) > 0` before calling `pop(items)`.",
                            "Guard empty collections explicitly.",
                            "Nothing left in the treat jar.",
                        )
                    });
                }
                if callee == "map_set" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let k = self.eval_expr(&args[1])?;
                    let v = self.eval_expr(&args[2])?;
                    let map_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::Map(entries) = map_val else {
                        unreachable!();
                    };
                    if let Some((_, slot)) = entries.iter_mut().find(|(ek, _)| ek == &k) {
                        *slot = v;
                    } else {
                        entries.push((k, v));
                    }
                    return Ok(Value::Void);
                }
                if callee == "map_remove" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let k = self.eval_expr(&args[1])?;
                    let map_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::Map(entries) = map_val else {
                        unreachable!();
                    };
                    let before = entries.len();
                    entries.retain(|(ek, _)| ek != &k);
                    return Ok(Value::Bool(entries.len() < before));
                }
                if callee == "sort" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    items.sort_by(|a, b| match (a, b) {
                        (Value::Number(x), Value::Number(y)) => x.cmp(y),
                        (Value::Decimal(x), Value::Decimal(y)) => {
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Value::Text(x), Value::Text(y)) => x.cmp(y),
                        _ => a.to_string().cmp(&b.to_string()),
                    });
                    return Ok(Value::Void);
                }
                if callee == "reverse" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    items.reverse();
                    return Ok(Value::Void);
                }
                if callee == "insert_at" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let Value::Number(idx) = self.eval_expr(&args[1])? else {
                        unreachable!();
                    };
                    let item = self.eval_expr(&args[2])?;
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    if idx < 0 || (idx as usize) > items.len() {
                        return Err(index_bounds_err(idx, items.len() + 1, span));
                    }
                    items.insert(idx as usize, item);
                    return Ok(Value::Void);
                }
                if callee == "remove_at" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let Value::Number(idx) = self.eval_expr(&args[1])? else {
                        unreachable!();
                    };
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    if idx < 0 || (idx as usize) >= items.len() {
                        return Err(index_bounds_err(idx, items.len(), span));
                    }
                    return Ok(items.remove(idx as usize));
                }
                if callee == "clear" {
                    if args.is_empty() {
                        self.io.clear_screen();
                        return Ok(Value::Void);
                    }
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let list_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::List(items) = list_val else {
                        unreachable!();
                    };
                    items.clear();
                    return Ok(Value::Void);
                }
                if callee == "map_clear" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let map_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::Map(entries) = map_val else {
                        unreachable!();
                    };
                    entries.clear();
                    return Ok(Value::Void);
                }
                if callee == "map_merge" {
                    let Expr::Identifier { name, .. } = &args[0] else {
                        unreachable!();
                    };
                    let Value::Map(src_entries) = self.eval_expr(&args[1])? else {
                        unreachable!();
                    };
                    let map_val = self
                        .env
                        .get_mut_var(name)
                        .map_err(|_| immutable_err(name, span))?;
                    let Value::Map(entries) = map_val else {
                        unreachable!();
                    };
                    for (k, v) in src_entries {
                        if let Some((_, slot)) = entries.iter_mut().find(|(ek, _)| ek == &k) {
                            *slot = v;
                        } else {
                            entries.push((k, v));
                        }
                    }
                    return Ok(Value::Void);
                }

                let mut evaluated = Vec::with_capacity(args.len());
                for arg in args {
                    evaluated.push(self.eval_expr(arg)?);
                }

                if let Some(fn_def) = self.functions.get(callee).cloned() {
                    if self.call_depth >= MAX_CALL_DEPTH {
                        return Err(DogDiagnostic::new(
                            "E032",
                            format!("Maximum call depth ({MAX_CALL_DEPTH}) exceeded in '{callee}()'."),
                            span.clone(),
                            "Recursion went too deep without reaching a base case.",
                            "Ensure recursive functions converge to a terminating base case.",
                            "Check your stopping condition.",
                            "Chasing my own tail in an infinite loop.",
                        )
                        .with_stack_trace(self.call_stack.clone()));
                    }
                    let mut bound = Vec::with_capacity(fn_def.params.len());
                    for (idx, param) in fn_def.params.iter().enumerate() {
                        let val = if idx < evaluated.len() {
                            evaluated[idx].clone()
                        } else {
                            let def_expr = param.default.as_ref().expect("type checker guarantees default");
                            self.eval_expr(def_expr)?
                        };
                        bound.push((param.name.clone(), val));
                    }

                    self.call_depth += 1;
                    self.call_stack
                        .push(format!("{callee}() at {}:{}", span.file, span.line));
                    self.env.push_scope();
                    for (pname, val) in bound {
                        self.env.define(pname, BindingKind::Let, val);
                    }
                    let flow = self.eval_block(&fn_def.body);
                    self.env.pop_scope();
                    self.call_depth -= 1;
                    let res = match flow {
                        Ok(ExecFlow::Return(v)) => Ok(v),
                        Ok(ExecFlow::Next | ExecFlow::Break | ExecFlow::Continue) => Ok(Value::Void),
                        Err(mut diag) => {
                            if diag.stack_trace.is_empty() {
                                diag.stack_trace = self.call_stack.clone();
                            }
                            Err(diag)
                        }
                    };
                    self.call_stack.pop();
                    return res;
                }

                call_builtin(callee, evaluated, span, self.io)
    }
}

fn immutable_err(name: &str, span: &Span) -> DogDiagnostic {
    DogDiagnostic::new(
        "E012",
        format!("Invalid mutation on '{name}'."),
        span.clone(),
        "Cannot mutate immutable or undefined variable.",
        "Declare mutable variables with `var`.",
        "Check mutability before modifying.",
        "No sneaking past the type checker.",
    )
}

fn index_bounds_err(idx: i64, len: usize, span: &Span) -> DogDiagnostic {
    DogDiagnostic::new(
        "E034",
        format!("Index {idx} out of bounds for length {len}."),
        span.clone(),
        format!("Valid indices are 0..{}, but got {idx}.", len.saturating_sub(1)),
        "Check `len(...)` before indexing.",
        "DOG uses 0-based indexing.",
        "You reached past the end of the fence.",
    )
}

fn div_zero(span: &Span) -> DogDiagnostic {
    DogDiagnostic::new(
        "E031",
        "Division by zero.",
        span.clone(),
        "The denominator evaluated to zero during division or modulo.",
        "Guard the divisor with an `if divisor != 0` check first.",
        "Always validate denominators from external input.",
        "Dividing by zero creates a black hole in my kennel.",
    )
}

pub fn resolve_program(file: &str, source: &str) -> Result<Program, DogDiagnostic> {
    let mut visited = HashSet::new();
    let mut active = HashSet::new();
    let base_dir = Path::new(file)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    resolve_recursive(file, source, &base_dir, &mut visited, &mut active)
}

fn resolve_recursive(
    file: &str,
    source: &str,
    base_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    active: &mut HashSet<PathBuf>,
) -> Result<Program, DogDiagnostic> {
    let tokens = Lexer::new(file, source).tokenize()?;
    let program = Parser::new(tokens).parse()?;

    let mut merged = Vec::new();
    for stmt in program.statements {
        if let Stmt::Use { path, span } = stmt {
            if path.starts_with("std.") {
                if crate::stdlib::modules::is_std_module(&path) {
                    merged.push(Stmt::Use { path, span });
                    continue;
                }
                let mut fix = "Available std modules: std.io, std.text, std.math, std.files, std.net, std.system, std.data, std.crypto, std.cli, std.collections.".to_string();
                if let Some(hint) = crate::stdlib::modules::suggest_std_module(&path) {
                    fix = format!("Did you mean `use \"{hint}\"`? {fix}");
                }
                return Err(DogDiagnostic::new(
                    "E040",
                    format!("Unknown standard library module '{path}'."),
                    span,
                    format!("'{path}' is not a built-in DOG standard library module."),
                    fix,
                    "Inspect built-in modules with `dog docs std`.",
                    "That standard aisle doesn't exist.",
                ));
            }
            let target = base_dir.join(&path);
            let canonical = fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
            if active.contains(&canonical) {
                return Err(DogDiagnostic::new(
                    "E041",
                    format!("Circular module import detected for '{path}'."),
                    span,
                    "A module cannot directly or indirectly `use` itself.",
                    "Break the cycle by extracting shared definitions into a third `.dg` file.",
                    "Keep module dependencies acyclic.",
                    "Chasing your own tail across files.",
                ));
            }
            if visited.contains(&canonical) {
                continue;
            }
            let mod_src = fs::read_to_string(&target).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to load module '{path}'."),
                    span,
                    format!("Could not read '{}': {e}", target.display()),
                    "Verify the relative `.dg` path in `use \"...\"`.",
                    "Module paths are resolved relative to the importing file.",
                    "Module went missing.",
                )
            })?;
            active.insert(canonical.clone());
            let mod_dir = target.parent().unwrap_or(base_dir);
            let sub = resolve_recursive(
                &target.display().to_string(),
                &mod_src,
                mod_dir,
                visited,
                active,
            )?;
            active.remove(&canonical);
            visited.insert(canonical);
            for sub_stmt in sub.statements {
                if matches!(
                    sub_stmt,
                    Stmt::FnDef { .. }
                        | Stmt::StructDef { .. }
                        | Stmt::EnumDef { .. }
                        | Stmt::Binding {
                            kind: BindingKind::Const,
                            ..
                        }
                ) {
                    merged.push(sub_stmt);
                }
            }
        } else {
            merged.push(stmt);
        }
    }
    Ok(Program { statements: merged })
}

pub fn check_source(file: &str, source: &str) -> Result<Program, DogDiagnostic> {
    let program = resolve_program(file, source)?;
    let mut checker = TypeChecker::new();
    checker.check(&program)?;
    Ok(program)
}

pub fn run_source_with_io(
    file: &str,
    source: &str,
    io: &mut dyn InterpreterIo,
) -> Result<(), DogDiagnostic> {
    let program = check_source(file, source)?;
    let mut interp = Interpreter::new(io);
    interp.eval_program(&program)
}

pub fn run_source(file: &str, source: &str) -> Result<(), DogDiagnostic> {
    let mut stdio = StdIo;
    run_source_with_io(file, source, &mut stdio)
}
