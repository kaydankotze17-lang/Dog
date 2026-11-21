use crate::ast::{BinOp, BindingKind, Expr, Program, Span, Stmt, TypeRef, UnaryOp};
use crate::diagnostics::DogDiagnostic;
use crate::types::DogType;
use std::collections::{BTreeMap, HashMap};

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
    "say", "answer", "lower", "upper", "trim", "contains", "split", "join", "len", "text",
    "number", "decimal", "push", "pop", "abs", "min", "max", "assert", "fail", "map_get",
    "map_set", "map_has", "map_keys", "map_remove", "file_read", "file_write", "file_exists",
    "file_delete", "dir_list", "env_get", "env_set", "time_now_ms", "sleep_ms", "json_parse",
    "json_stringify", "hex_encode", "hex_decode", "base64_encode", "base64_decode", "sha256",
    "http_get", "log_info", "log_warn", "log_error",
];

pub struct TypeChecker {
    scopes: Vec<HashMap<String, BindingInfo>>,
    functions: HashMap<String, FnSig>,
    structs: HashMap<String, BTreeMap<String, DogType>>,
    enums: HashMap<String, Vec<String>>,
    current_return_type: Option<DogType>,
    loop_depth: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            current_return_type: None,
            loop_depth: 0,
        }
    }

    pub fn list_symbols(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(root) = self.scopes.first() {
            for (k, v) in root {
                out.push((k.clone(), v.ty.label()));
            }
        }
        for (name, sig) in &self.functions {
            out.push((format!("fn {name}()"), sig.return_type.label()));
        }
        for name in self.structs.keys() {
            out.push((format!("struct {name}"), name.clone()));
        }
        for name in self.enums.keys() {
            out.push((format!("enum {name}"), name.clone()));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn check(&mut self, program: &Program) -> Result<(), DogDiagnostic> {
        // Pass 0: register struct and enum names first so field types & fn signatures can reference them.
        for stmt in &program.statements {
            match stmt {
                Stmt::StructDef { name, span, .. } => {
                    if self.structs.contains_key(name) || self.enums.contains_key(name) {
                        return Err(DogDiagnostic::new(
                            "E011",
                            format!("Type '{name}' is already defined."),
                            span.clone(),
                            format!("A struct or enum named '{name}' already exists."),
                            "Choose a distinct type name.",
                            "Type names must be unique.",
                            "Two blueprints, one name.",
                        ));
                    }
                    self.structs.insert(name.clone(), BTreeMap::new());
                }
                Stmt::EnumDef { name, variants, span } => {
                    if self.structs.contains_key(name) || self.enums.contains_key(name) {
                        return Err(DogDiagnostic::new(
                            "E011",
                            format!("Type '{name}' is already defined."),
                            span.clone(),
                            format!("A struct or enum named '{name}' already exists."),
                            "Choose a distinct type name.",
                            "Type names must be unique.",
                            "Duplicate enum name.",
                        ));
                    }
                    self.enums.insert(name.clone(), variants.clone());
                }
                _ => {}
            }
        }

        // Pass 1a: resolve struct fields
        for stmt in &program.statements {
            if let Stmt::StructDef { name, fields, span } = stmt {
                let mut resolved = BTreeMap::new();
                for f in fields {
                    let f_ty = self.resolve_type_ref(&f.ty, &f.span)?;
                    if resolved.contains_key(&f.name) {
                        return Err(DogDiagnostic::new(
                            "E011",
                            format!("Duplicate field '{}' in struct '{name}'.", f.name),
                            f.span.clone(),
                            "Struct field names must be unique.",
                            "Rename the duplicate field.",
                            "Check struct definition.",
                            "Two fields with the same name.",
                        ));
                    }
                    resolved.insert(f.name.clone(), f_ty);
                }
                if resolved.is_empty() {
                    return Err(DogDiagnostic::new(
                        "E004",
                        format!("Struct '{name}' must define at least one field."),
                        span.clone(),
                        "Empty structs carry no data.",
                        "Add at least one `field: type` entry.",
                        "Use an `enum` for dataless variants.",
                        "An empty doghouse with no floor.",
                    ));
                }
                self.structs.insert(name.clone(), resolved);
            }
        }

        // Pass 1b: register all top-level function signatures.
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
                let mut param_tys = Vec::with_capacity(params.len());
                for p in params {
                    param_tys.push(self.resolve_type_ref(&p.ty, &p.span)?);
                }
                let ret_ty = self.resolve_type_ref(return_type, span)?;
                self.functions.insert(
                    name.clone(),
                    FnSig {
                        params: param_tys,
                        return_type: ret_ty,
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

    fn resolve_type_ref(&self, tr: &TypeRef, span: &Span) -> Result<DogType, DogDiagnostic> {
        DogType::from_ref_with(tr, &|name| {
            if self.structs.contains_key(name) {
                Some(DogType::Struct(name.to_string()))
            } else if self.enums.contains_key(name) {
                Some(DogType::Enum(name.to_string()))
            } else {
                None
            }
        })
        .map_err(|msg| {
            DogDiagnostic::new(
                "E004",
                msg,
                span.clone(),
                "Referenced type name is not a built-in, struct, or enum.",
                "Define the struct/enum first or use text, number, decimal, bool, list<T>, map<K, V>.",
                "Type names are case-sensitive.",
                "Never heard of that type.",
            )
        })
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), DogDiagnostic> {
        match stmt {
            Stmt::Use { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } => Ok(()),
            Stmt::Binding {
                kind,
                name,
                type_ann,
                value,
                span,
            } => {
                let annotated = match type_ann {
                    Some(tr) => Some(self.resolve_type_ref(tr, span)?),
                    None => None,
                };
                let ty = self.check_expr_with_hint(value, annotated.as_ref())?;

                if ty == DogType::Void {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Cannot bind a 'void' expression to a variable.",
                        span.clone(),
                        format!("'{name}' cannot store the result of an action that returns nothing."),
                        "Assign a non-void expression.",
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

                if self.structs.contains_key(name) || self.enums.contains_key(name) {
                    return Err(DogDiagnostic::new(
                        "E011",
                        format!("Binding name '{name}' conflicts with an existing struct or enum."),
                        span.clone(),
                        format!("'{name}' is already declared as a type in this program."),
                        "Choose a distinct variable name (for example lowercase).",
                        "Use PascalCase for types and snake_case for variables.",
                        "Don't name your dog after the doghouse.",
                    ));
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
                        format!("'{target}' was not declared with `var`."),
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
                            "Use `map_set()` for maps.",
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
            Stmt::FieldAssign {
                target,
                field,
                value,
                span,
            } => {
                let existing = self.lookup(target).cloned().ok_or_else(|| {
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{target}'."),
                        span.clone(),
                        format!("No struct variable named '{target}' is in scope."),
                        format!("Declare `var {target} = ...` first."),
                        "Declare variables before mutating fields.",
                        "Where is that struct?",
                    )
                })?;
                if existing.kind != BindingKind::Var {
                    return Err(DogDiagnostic::new(
                        "E012",
                        format!("Cannot mutate field on immutable binding '{target}'."),
                        span.clone(),
                        format!("'{target}' was declared immutable."),
                        format!("Declare `{target}` with `var` to mutate fields."),
                        "Immutability applies deeply to bindings.",
                        "Read-only collar.",
                    ));
                }
                let DogType::Struct(struct_name) = existing.ty else {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Field assignment requires a struct.",
                        span.clone(),
                        format!("'{target}' is not a struct."),
                        "Only struct values have assignable fields.",
                        "Check variable type.",
                        "That doesn't have fields.",
                    ));
                };
                let fields = self.structs.get(&struct_name).cloned().unwrap_or_default();
                let Some(field_ty) = fields.get(field) else {
                    return Err(DogDiagnostic::new(
                        "E010",
                        format!("Unknown field '{field}' on struct '{struct_name}'."),
                        span.clone(),
                        format!("Struct '{struct_name}' has no field named '{field}'."),
                        "Check the struct definition spelling.",
                        "Inspect struct fields carefully.",
                        "Sniffing for a field that isn't there.",
                    ));
                };
                let val_ty = self.check_expr_with_hint(value, Some(field_ty))?;
                if val_ty != *field_ty {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "Type mismatch in field assignment.",
                        span.clone(),
                        format!("Field '{struct_name}.{field}' expects '{field_ty}', got '{val_ty}'."),
                        format!("Assign a '{field_ty}' value."),
                        "Struct fields are strictly typed.",
                        "Wrong type for that slot.",
                    )
                    .with_types(field_ty.label(), val_ty.label()));
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
                self.check_block(then_branch)?;
                self.scopes.pop();

                if let Some(else_stmts) = otherwise_branch {
                    self.scopes.push(HashMap::new());
                    self.check_block(else_stmts)?;
                    self.scopes.pop();
                }
                Ok(())
            }
            Stmt::Match {
                subject,
                cases,
                otherwise_branch,
                ..
            } => {
                let subj_ty = self.check_expr(subject)?;
                for case in cases {
                    let pat_ty = self.check_expr(&case.pattern)?;
                    if pat_ty != subj_ty {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Case pattern type mismatch in 'match'.",
                            case.span.clone(),
                            format!("Match subject has type '{subj_ty}', but case pattern has type '{pat_ty}'."),
                            format!("Use a '{subj_ty}' pattern in every `case`."),
                            "All `case` patterns must match the `match` subject type.",
                            "Comparing apples to dog treats.",
                        )
                        .with_types(subj_ty.label(), pat_ty.label()));
                    }
                    self.scopes.push(HashMap::new());
                    self.check_block(&case.body)?;
                    self.scopes.pop();
                }
                if let Some(else_stmts) = otherwise_branch {
                    self.scopes.push(HashMap::new());
                    self.check_block(else_stmts)?;
                    self.scopes.pop();
                }
                Ok(())
            }
            Stmt::TryCatch {
                try_body,
                err_name,
                catch_body,
                ..
            } => {
                self.scopes.push(HashMap::new());
                self.check_block(try_body)?;
                self.scopes.pop();

                self.scopes.push(HashMap::new());
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(
                        err_name.clone(),
                        BindingInfo {
                            kind: BindingKind::Let,
                            ty: DogType::Text,
                        },
                    );
                }
                self.check_block(catch_body)?;
                self.scopes.pop();
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
                let ret_ty = self.resolve_type_ref(return_type, span)?;
                self.current_return_type = Some(ret_ty.clone());
                self.scopes.push(HashMap::new());
                for param in params {
                    let p_ty = self.resolve_type_ref(&param.ty, &param.span)?;
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
                            ty: p_ty,
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
            Expr::MapLit { entries, span } => {
                if entries.is_empty() {
                    if let Some(DogType::Map(k, v)) = hint {
                        return Ok(DogType::Map(k.clone(), v.clone()));
                    }
                    return Err(DogDiagnostic::new(
                        "E024",
                        "Cannot infer key/value types of empty map '{}'.",
                        span.clone(),
                        "Strict typing requires every map to know its key and value types.",
                        "Add a type annotation, e.g. `var m: map<text, number> = {}`.",
                        "Non-empty maps like `{\"a\": 1}` infer their types automatically.",
                        "An empty map with no legend.",
                    ));
                }
                let (k_hint, v_hint) = match hint {
                    Some(DogType::Map(k, v)) => (Some(k.as_ref()), Some(v.as_ref())),
                    _ => (None, None),
                };
                let first_k = self.check_expr_with_hint(&entries[0].0, k_hint)?;
                let first_v = self.check_expr_with_hint(&entries[0].1, v_hint)?;
                for (k, v) in &entries[1..] {
                    let kt = self.check_expr_with_hint(k, Some(&first_k))?;
                    let vt = self.check_expr_with_hint(v, Some(&first_v))?;
                    if kt != first_k {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Mixed key types in map literal.",
                            k.span().clone(),
                            format!("Expected map key type '{first_k}', found '{kt}'."),
                            "Use uniform key types across the map.",
                            "Maps in DOG are strictly typed.",
                            "Every key needs the same shape.",
                        )
                        .with_types(first_k.label(), kt.label()));
                    }
                    if vt != first_v {
                        return Err(DogDiagnostic::new(
                            "E021",
                            "Mixed value types in map literal.",
                            v.span().clone(),
                            format!("Expected map value type '{first_v}', found '{vt}'."),
                            "Use uniform value types across the map.",
                            "Maps in DOG are strictly typed.",
                            "Keep map values one consistent type.",
                        )
                        .with_types(first_v.label(), vt.label()));
                    }
                }
                Ok(DogType::Map(Box::new(first_k), Box::new(first_v)))
            }
            Expr::StructInit { name, fields, span } => {
                let Some(def_fields) = self.structs.get(name) else {
                    return Err(DogDiagnostic::new(
                        "E010",
                        format!("Unknown struct '{name}'."),
                        span.clone(),
                        format!("No struct named '{name}' has been defined."),
                        format!("Define `struct {name} ... end` before constructing it."),
                        "Struct names are case-sensitive.",
                        "Never seen that blueprint.",
                    ));
                };
                let mut seen = HashMap::new();
                for (fname, fexpr) in fields {
                    let Some(expected_ty) = def_fields.get(fname) else {
                        return Err(DogDiagnostic::new(
                            "E010",
                            format!("Unknown field '{fname}' on struct '{name}'."),
                            fexpr.span().clone(),
                            format!("Struct '{name}' does not have a field named '{fname}'."),
                            "Check field spelling against the struct definition.",
                            "Only declared fields can be initialized.",
                            "Inventing extra fields won't fly.",
                        ));
                    };
                    let got_ty = self.check_expr_with_hint(fexpr, Some(expected_ty))?;
                    if got_ty != *expected_ty {
                        return Err(DogDiagnostic::new(
                            "E021",
                            format!("Type mismatch for field '{name}.{fname}'."),
                            fexpr.span().clone(),
                            format!("Field '{fname}' expects '{expected_ty}', got '{got_ty}'."),
                            format!("Pass a '{expected_ty}' value."),
                            "Struct fields are strictly typed.",
                            "Wrong value in that slot.",
                        )
                        .with_types(expected_ty.label(), got_ty.label()));
                    }
                    seen.insert(fname.clone(), true);
                }
                for req_field in def_fields.keys() {
                    if !seen.contains_key(req_field) {
                        return Err(DogDiagnostic::new(
                            "E004",
                            format!("Missing field '{req_field}' when initializing '{name}'."),
                            span.clone(),
                            format!("Struct '{name}' requires every field to be initialized."),
                            format!("Provide `{req_field}: ...` inside `{name} {{ ... }}`."),
                            "DOG avoids uninitialized fields by design.",
                            "Forgot a piece of the puzzle.",
                        ));
                    }
                }
                Ok(DogType::Struct(name.clone()))
            }
            Expr::FieldAccess {
                target,
                field,
                span,
            } => {
                if let Expr::Identifier { name, .. } = target.as_ref() {
                    if let Some(variants) = self.enums.get(name) {
                        if variants.contains(field) {
                            return Ok(DogType::Enum(name.clone()));
                        }
                        return Err(DogDiagnostic::new(
                            "E010",
                            format!("Unknown variant '{field}' on enum '{name}'."),
                            span.clone(),
                            format!("Enum '{name}' has variants: {}.", variants.join(", ")),
                            format!("Use a valid variant like `{name}.{}`.", variants.first().map(String::as_str).unwrap_or("Variant")),
                            "Enum variants are case-sensitive.",
                            "That variant isn't in the pack.",
                        ));
                    }
                }
                let target_ty = self.check_expr(target)?;
                match target_ty {
                    DogType::Struct(struct_name) => {
                        let fields = self.structs.get(&struct_name).expect("struct exists");
                        fields.get(field).cloned().ok_or_else(|| {
                            DogDiagnostic::new(
                                "E010",
                                format!("Unknown field '{field}' on struct '{struct_name}'."),
                                span.clone(),
                                format!("Struct '{struct_name}' has no field named '{field}'."),
                                "Check the struct definition.",
                                "Inspect available field names.",
                                "No such field.",
                            )
                        })
                    }
                    other => Err(DogDiagnostic::new(
                        "E021",
                        format!("Cannot access field '.{field}' on type '{other}'."),
                        span.clone(),
                        "Dot field access applies to struct values or Enum.Variant.",
                        "Use a struct instance or enum name.",
                        "For maps, use `map_get(m, key)`.",
                        "Only structs and enums respond to dot syntax.",
                    )),
                }
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
                        "For maps, use `map_get(m, key)`.",
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
        if callee == "map_set" {
            if args.len() != 3 {
                return Err(arity_error(callee, 3, args.len(), span));
            }
            let (k_ty, v_ty) = self.require_mutable_map_arg(&args[0], callee)?;
            let got_k = self.check_expr_with_hint(&args[1], Some(&k_ty))?;
            let got_v = self.check_expr_with_hint(&args[2], Some(&v_ty))?;
            if got_k != k_ty || got_v != v_ty {
                return Err(DogDiagnostic::new(
                    "E021",
                    "Type mismatch in map_set().",
                    span.clone(),
                    format!("Expected ({k_ty}, {v_ty}), received ({got_k}, {got_v})."),
                    "Match the map's key and value types.",
                    "Maps are strictly typed.",
                    "Wrong key or value type for this map.",
                )
                .with_types(format!("{k_ty}, {v_ty}"), format!("{got_k}, {got_v}")));
            }
            return Ok(DogType::Void);
        }
        if callee == "map_remove" {
            if args.len() != 2 {
                return Err(arity_error(callee, 2, args.len(), span));
            }
            let (k_ty, _) = self.require_mutable_map_arg(&args[0], callee)?;
            let got_k = self.check_expr_with_hint(&args[1], Some(&k_ty))?;
            if got_k != k_ty {
                return Err(DogDiagnostic::new(
                    "E021",
                    "Type mismatch in map_remove().",
                    args[1].span().clone(),
                    format!("Expected key type '{k_ty}', got '{got_k}'."),
                    "Pass a matching key type.",
                    "Check key types.",
                    "Wrong key type.",
                )
                .with_types(k_ty.label(), got_k.label()));
            }
            return Ok(DogType::Bool);
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
                        "Pass text, number, decimal, bool, list, map, struct, or enum to say().",
                        "Do not nest say(say(...)).",
                        "Saying nothing out loud is called silence.",
                    )
                    .with_types("printable value", "void"));
                }
                Ok(DogType::Void)
            }
            "answer" => {
                if arg_types.len() > 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if let Some(t) = arg_types.first() {
                    if *t != DogType::Text {
                        return Err(type_err("text", &t.label(), span, "answer() prompt must be 'text'."));
                    }
                }
                Ok(DogType::Text)
            }
            "lower" | "upper" | "trim" | "hex_encode" | "hex_decode" | "base64_encode"
            | "base64_decode" | "sha256" | "file_read" | "http_get" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, &format!("{callee}() expects 'text'.")));
                }
                Ok(DogType::Text)
            }
            "file_exists" | "file_delete" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, &format!("{callee}() expects path 'text'.")));
                }
                Ok(DogType::Bool)
            }
            "file_write" | "env_set" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err("text, text", &format!("{}, {}", arg_types[0], arg_types[1]), span, &format!("{callee}() expects two 'text' arguments.")));
                }
                Ok(DogType::Void)
            }
            "dir_list" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "dir_list() expects directory path 'text'."));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "env_get" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "env_get() expects variable name 'text'."));
                }
                Ok(DogType::Text)
            }
            "time_now_ms" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Number)
            }
            "sleep_ms" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err("number", &arg_types[0].label(), span, "sleep_ms() expects milliseconds as 'number'."));
                }
                Ok(DogType::Void)
            }
            "json_parse" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "json_parse() expects JSON 'text'."));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "json_stringify" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] == DogType::Void {
                    return Err(type_err("non-void", "void", span, "Cannot serialize 'void' to JSON."));
                }
                Ok(DogType::Text)
            }
            "log_info" | "log_warn" | "log_error" | "fail" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, &format!("{callee}() expects 'text'.")));
                }
                Ok(DogType::Void)
            }
            "map_get" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let DogType::Map(k, v) = &arg_types[0] else {
                    return Err(type_err("map<K, V>", &arg_types[0].label(), span, "map_get() expects a map as first argument."));
                };
                if &arg_types[1] != k.as_ref() {
                    return Err(type_err(&k.label(), &arg_types[1].label(), span, "map_get() key type mismatch."));
                }
                Ok((**v).clone())
            }
            "map_has" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let DogType::Map(k, _) = &arg_types[0] else {
                    return Err(type_err("map<K, V>", &arg_types[0].label(), span, "map_has() expects a map as first argument."));
                };
                if &arg_types[1] != k.as_ref() {
                    return Err(type_err(&k.label(), &arg_types[1].label(), span, "map_has() key type mismatch."));
                }
                Ok(DogType::Bool)
            }
            "map_keys" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let DogType::Map(k, _) = &arg_types[0] else {
                    return Err(type_err("map<K, V>", &arg_types[0].label(), span, "map_keys() expects a map."));
                };
                Ok(DogType::List(k.clone()))
            }
            "contains" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err("text, text", &format!("{}, {}", arg_types[0], arg_types[1]), span, "contains() expects two 'text' arguments."));
                }
                Ok(DogType::Bool)
            }
            "split" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err("text, text", &format!("{}, {}", arg_types[0], arg_types[1]), span, "split() expects (text, text)."));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "join" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let expected_list = DogType::List(Box::new(DogType::Text));
                if arg_types[0] != expected_list || arg_types[1] != DogType::Text {
                    return Err(type_err("list<text>, text", &format!("{}, {}", arg_types[0], arg_types[1]), span, "join() expects (list<text>, text)."));
                }
                Ok(DogType::Text)
            }
            "len" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Text | DogType::List(_) | DogType::Map(_, _) => Ok(DogType::Number),
                    other => Err(type_err("text | list<T> | map<K, V>", &other.label(), span, "len() expects text, list, or map.")),
                }
            }
            "text" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] == DogType::Void {
                    return Err(type_err("non-void", "void", span, "Cannot convert 'void' using text()."));
                }
                Ok(DogType::Text)
            }
            "number" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Text | DogType::Number | DogType::Decimal => Ok(DogType::Number),
                    other => Err(type_err("text | number | decimal", &other.label(), span, "Cannot convert value using number().")),
                }
            }
            "decimal" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Text | DogType::Number | DogType::Decimal => Ok(DogType::Decimal),
                    other => Err(type_err("text | number | decimal", &other.label(), span, "Cannot convert value using decimal().")),
                }
            }
            "abs" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                match &arg_types[0] {
                    DogType::Number => Ok(DogType::Number),
                    DogType::Decimal => Ok(DogType::Decimal),
                    other => Err(type_err("number | decimal", &other.label(), span, "abs() expects numeric value.")),
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
                    Err(type_err("T, T (number | decimal)", &format!("{}, {}", arg_types[0], arg_types[1]), span, &format!("{callee}() expects matching numeric arguments.")))
                }
            }
            "assert" => {
                if arg_types.is_empty() || arg_types.len() > 2 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Bool {
                    return Err(type_err("bool", &arg_types[0].label(), span, "assert() condition must be 'bool'."));
                }
                if let Some(msg_ty) = arg_types.get(1) {
                    if *msg_ty != DogType::Text {
                        return Err(type_err("text", &msg_ty.label(), span, "assert() message must be 'text'."));
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
            other => Err(type_err("list<T>", &other.label(), span, &format!("'{callee}()' expects a 'list<T>' variable."))),
        }
    }

    fn require_mutable_map_arg(
        &self,
        expr: &Expr,
        callee: &str,
    ) -> Result<(DogType, DogType), DogDiagnostic> {
        let Expr::Identifier { name, span } = expr else {
            return Err(DogDiagnostic::new(
                "E004",
                format!("First argument to '{callee}()' must be a mutable map variable."),
                expr.span().clone(),
                format!("'{callee}()' mutates a `var` map variable in place."),
                format!("Declare `var m = {{...}}` and pass `m` to {callee}()."),
                "Temporary map literals cannot be mutated in place.",
                "Give that map a `var` name first.",
            ));
        };
        let info = self.lookup(name).ok_or_else(|| {
            DogDiagnostic::new(
                "E010",
                format!("Undefined variable '{name}'."),
                span.clone(),
                format!("No variable named '{name}' exists in scope."),
                format!("Declare `var {name} = {{...}}` first."),
                "Check spelling.",
                "Where is that map?",
            )
        })?;
        if info.kind != BindingKind::Var {
            return Err(DogDiagnostic::new(
                "E012",
                format!("Cannot call '{callee}()' on immutable binding '{name}'."),
                span.clone(),
                format!("'{name}' was not declared with `var`."),
                format!("Declare `{name}` with `var` to mutate it."),
                "`let` and `const` maps stay fixed.",
                "Immutable maps stay sealed.",
            ));
        }
        match &info.ty {
            DogType::Map(k, v) => Ok(((**k).clone(), (**v).clone())),
            other => Err(type_err("map<K, V>", &other.label(), span, &format!("'{callee}()' expects a 'map<K, V>' variable."))),
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
            Stmt::Match {
                cases,
                otherwise_branch: Some(else_branch),
                ..
            } => {
                if cases.iter().all(|c| guarantees_return(&c.body))
                    && guarantees_return(else_branch)
                {
                    return true;
                }
            }
            Stmt::TryCatch {
                try_body,
                catch_body,
                ..
            } => {
                if guarantees_return(try_body) && guarantees_return(catch_body) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn type_err(expected: &str, received: &str, span: &Span, why: &str) -> DogDiagnostic {
    DogDiagnostic::new(
        "E021",
        "Type mismatch detected.",
        span.clone(),
        why,
        format!("Provide a value of type '{expected}'."),
        "Keep your types consistent.",
        "Math wasn't supposed to be this difficult.",
    )
    .with_types(expected, received)
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
