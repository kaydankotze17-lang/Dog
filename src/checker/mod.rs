use crate::ast::{BinOp, BindingKind, Expr, InterpPart, Program, Span, Stmt, TypeRef, UnaryOp};
use crate::diagnostics::{suggest_similar, DogDiagnostic};
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
    min_args: usize,
    return_type: DogType,
}

const BUILTIN_NAMES: &[&str] = &[
    "say", "print", "answer", "ask", "confirm", "choose", "password", "read_key", "progress", "table",
    "lower", "upper", "trim", "contains", "split", "join", "len", "text", "number", "decimal",
    "push", "pop", "abs", "min", "max", "assert", "fail", "map_get", "map_set", "map_has",
    "map_keys", "map_remove", "file_read", "file_write", "file_append", "file_exists",
    "file_delete", "dir_list", "mkdir", "cwd", "args", "env_get", "env_set", "env_load",
    "time_now_ms", "time_format", "sleep_ms", "json_parse", "json_stringify", "json_read",
    "json_write", "toml_parse", "hex_encode", "hex_decode", "base64_encode", "base64_decode",
    "sha256", "hmac_sha256", "sha1", "http_get", "http_post", "http_put", "http_delete",
    "http_request", "http_request_timeout", "http_listen", "http_port", "http_accept",
    "http_try_accept", "http_reply", "http_reply_headers", "http_redirect", "http_serve_file",
    "http_close", "tcp_listen", "tcp_port", "tcp_accept", "tcp_connect", "tcp_send", "tcp_recv",
    "tcp_set_timeout", "tcp_remote_addr", "tcp_close", "url_encode", "url_decode", "url_parse",
    "query_parse", "query_stringify", "cookie_parse", "html_escape", "http_route_match",
    "ws_upgrade", "ws_connect", "ws_send", "ws_recv", "ws_close", "udp_bind", "udp_send_to",
    "udp_recv_from", "udp_close", "dns_lookup", "port_is_open", "csv_parse", "kv_set", "kv_get",
    "kv_has", "kv_delete", "kv_keys", "exec_cmd", "os_name", "os_arch", "uuid_v4", "log_info",
    "log_warn", "log_error", "cli_info", "cli_success", "cli_warn", "cli_error", "cli_debug",
    "cli_progress", "cli_spinner", "cli_theme", "cli_prefix", "cli_timestamps",
    "cli_supports_color", "cli_is_tty", "replace", "starts_with", "ends_with", "slice",
    "index_of", "str_repeat", "pad_left", "pad_right", "round", "floor", "ceil", "sqrt", "pow",
    "clamp", "sin", "cos", "log", "rand_int", "range", "sort", "reverse", "list_contains",
    "list_index_of", "list_slice", "insert_at", "remove_at", "clear", "map_clear", "map_merge",
    "map_values", "glob_match", "time_utc", "json_pretty", "file_copy", "file_size", "is_file",
    "is_dir", "path_join", "char_at", "last_index_of", "count_matches", "remove_prefix",
    "remove_suffix", "title_case", "reverse_text", "lines", "tan", "atan2", "exp", "log10",
    "pow_int", "sign", "gcd", "lcm", "math_pi", "math_e", "list_unique", "list_take",
    "list_skip", "list_flatten", "list_sum", "list_avg", "list_concat", "map_from_lists",
    "map_get_or", "csv_stringify", "json_has_key", "path_dirname", "path_basename", "path_ext",
    "file_move", "rmdir", "temp_dir", "sha512", "hmac_sha256_verify", "base64_url_encode",
    "base64_url_decode", "rand_bytes_hex", "ws_ping", "local_ip", "http_status_text",
    "dog_css", "css_rule", "css_vars", "css_media", "css_keyframes", "css_theme", "css_bundle",
    "html_el", "html_page", "web_nav", "web_card", "web_grid", "web_button", "web_input",
    "dom_mount", "dom_set_text", "dom_set_html", "dom_get_value", "dom_set_value",
    "dom_add_class", "dom_remove_class", "dom_toggle_class", "dom_set_style", "dom_on",
    "web_title", "web_storage_set", "web_storage_get", "web_route", "web_navigate",
];

pub fn builtin_names() -> &'static [&'static str] {
    BUILTIN_NAMES
}

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

    pub fn infer_single_expr(&mut self, expr: &Expr) -> Result<DogType, DogDiagnostic> {
        self.check_expr(expr)
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
                let mut min_args = 0usize;
                let mut saw_default = false;
                for p in params {
                    if p.default.is_some() {
                        saw_default = true;
                    } else if saw_default {
                        return Err(DogDiagnostic::new(
                            "E004",
                            format!("Required parameter '{}' cannot follow an optional parameter with a default value.", p.name),
                            p.span.clone(),
                            "Parameters with default values must appear at the end of the parameter list.",
                            "Move required parameters before parameters with `= default`.",
                            "Example: `fn greet(name: text, prefix: text = \"Hi\") -> text`.",
                            "Required ingredients go in the bowl first.",
                        ));
                    } else {
                        min_args += 1;
                    }
                    param_tys.push(self.resolve_type_ref(&p.ty, &p.span)?);
                }
                let ret_ty = self.resolve_type_ref(return_type, span)?;
                self.functions.insert(
                    name.clone(),
                    FnSig {
                        params: param_tys,
                        min_args,
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
            let mut fix = "Define the struct/enum first or use text, number, decimal, bool, list<T>, map<K, V>.".to_string();
            if let TypeRef::Named(raw) = tr {
                let prims = ["text", "number", "decimal", "bool", "void"];
                let candidates = prims
                    .iter()
                    .copied()
                    .chain(self.structs.keys().map(String::as_str))
                    .chain(self.enums.keys().map(String::as_str));
                if let Some(hint) = suggest_similar(raw, candidates) {
                    fix = format!("Did you mean '{hint}'? {fix}");
                }
            }
            DogDiagnostic::new(
                "E004",
                msg,
                span.clone(),
                "Referenced type name is not a built-in, struct, or enum.",
                fix,
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
                    let fix = self.suggest_var_fix(name, format!("Declare it first with `var {name} = ...`."));
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{name}'."),
                        span.clone(),
                        format!("No variable named '{name}' is in scope at this location."),
                        fix,
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
            Stmt::CompoundAssign {
                name,
                op,
                value,
                span,
            } => {
                let existing = self.lookup(name).cloned().ok_or_else(|| {
                    let fix = self.suggest_var_fix(name, format!("Declare it first with `var {name} = ...`."));
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{name}'."),
                        span.clone(),
                        format!("No variable named '{name}' is in scope at this location."),
                        fix,
                        "Variables must be declared before compound assignment.",
                        "Can't modify a variable that doesn't exist.",
                    )
                })?;
                if existing.kind != BindingKind::Var {
                    return Err(DogDiagnostic::new(
                        "E012",
                        format!("Cannot mutate immutable binding '{name}'."),
                        span.clone(),
                        format!("'{name}' was not declared with `var`."),
                        format!("Declare `var {name} = ...` to allow compound assignment."),
                        "Prefer `let` for constants and `var` for mutating accumulators.",
                        "Immutable means immutable.",
                    ));
                }
                let val_ty = self.check_expr_with_hint(value, Some(&existing.ty))?;
                let res_ty = self.check_binary(existing.ty.clone(), *op, val_ty.clone(), span)?;
                if res_ty != existing.ty {
                    return Err(type_err(
                        &existing.ty.label(),
                        &res_ty.label(),
                        span,
                        "Compound assignment must preserve variable type.",
                    ));
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
                    let fix = self.suggest_var_fix(target, format!("Declare it with `var {target} = [...]`."));
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{target}'."),
                        span.clone(),
                        format!("No list named '{target}' is in scope."),
                        fix,
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
                    let mut fix = "Check the struct definition spelling.".to_string();
                    if let Some(hint) = suggest_similar(field, fields.keys().map(String::as_str)) {
                        fix = format!("Did you mean '.{hint}'? {fix}");
                    }
                    return Err(DogDiagnostic::new(
                        "E010",
                        format!("Unknown field '{field}' on struct '{struct_name}'."),
                        span.clone(),
                        format!("Struct '{struct_name}' has no field named '{field}'."),
                        fix,
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
                let mut resolved_params = Vec::with_capacity(params.len());
                for param in params {
                    let p_ty = self.resolve_type_ref(&param.ty, &param.span)?;
                    if let Some(def_expr) = &param.default {
                        let got = self.check_expr_with_hint(def_expr, Some(&p_ty))?;
                        if got != p_ty {
                            return Err(DogDiagnostic::new(
                                "E021",
                                format!("Default value for parameter '{}' does not match declared type.", param.name),
                                def_expr.span().clone(),
                                format!("Parameter '{}' expects '{p_ty}', default value has type '{got}'.", param.name),
                                format!("Provide a '{p_ty}' default value."),
                                "Default parameter expressions must match their parameter type.",
                                "Default value clashes with its own collar.",
                            )
                            .with_types(p_ty.label(), got.label()));
                        }
                    }
                    resolved_params.push((param, p_ty));
                }
                self.current_return_type = Some(ret_ty.clone());
                self.scopes.push(HashMap::new());
                for (param, p_ty) in resolved_params {
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
            Stmt::StyleDef {
                selector,
                rules,
                variants,
                span,
            } => {
                let sel_ty = self.check_expr(selector)?;
                if sel_ty != DogType::Text {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "CSS selector in 'style' block must be text.",
                        span.clone(),
                        format!("Expected 'text' selector, got '{sel_ty}'."),
                        "Write e.g. `style \".card\" ... end` or `style \"#hero\" ... end`.",
                        "Style selectors identify DOM targets using CSS selector strings.",
                        "A CSS selector needs to be text.",
                    )
                    .with_types("text", sel_ty.label()));
                }
                for rule in rules {
                    let val_ty = self.check_expr(&rule.value)?;
                    if val_ty == DogType::Void {
                        return Err(DogDiagnostic::new(
                            "E021",
                            format!("Style property '{}' cannot use a 'void' expression.", rule.prop),
                            rule.span.clone(),
                            "Every style rule value must produce a concrete printable value.",
                            "Provide a text, number, decimal, or bool value.",
                            "Example: `bg: \"#FFFFFF\"`.",
                            "Cannot paint pixels with void.",
                        )
                        .with_types("value", "void"));
                    }
                }
                for var in variants {
                    for rule in &var.rules {
                        let val_ty = self.check_expr(&rule.value)?;
                        if val_ty == DogType::Void {
                            return Err(DogDiagnostic::new(
                                "E021",
                                format!("Style property '{}' cannot use a 'void' expression.", rule.prop),
                                rule.span.clone(),
                                "Every style rule value must produce a concrete printable value.",
                                "Provide a text, number, decimal, or bool value.",
                                "Example: `bg: \"#F8FAFC\"`.",
                                "Cannot paint pixels with void.",
                            )
                            .with_types("value", "void"));
                        }
                    }
                }
                Ok(())
            }
            Stmt::StyleDef {
                selector,
                rules,
                variants,
                span,
            } => {
                let sel_ty = self.check_expr(selector)?;
                if sel_ty != DogType::Text {
                    return Err(DogDiagnostic::new(
                        "E021",
                        "CSS selector in 'style' block must be text.",
                        span.clone(),
                        format!("Expected 'text' selector, got '{sel_ty}'."),
                        "Write e.g. `style \".card\" ... end` or `style \"#hero\" ... end`.",
                        "Style selectors identify DOM targets using CSS selector strings.",
                        "A CSS selector needs to be text.",
                    )
                    .with_types("text", sel_ty.label()));
                }
                for rule in rules {
                    let val_ty = self.check_expr(&rule.value)?;
                    if val_ty == DogType::Void {
                        return Err(DogDiagnostic::new(
                            "E021",
                            format!("Style property '{}' cannot use a 'void' expression.", rule.prop),
                            rule.span.clone(),
                            "Every style rule value must produce a concrete printable value.",
                            "Provide a text, number, decimal, or bool value.",
                            "Example: `bg: \"#FFFFFF\"`.",
                            "Cannot paint pixels with void.",
                        )
                        .with_types("value", "void"));
                    }
                }
                for var in variants {
                    for rule in &var.rules {
                        let val_ty = self.check_expr(&rule.value)?;
                        if val_ty == DogType::Void {
                            return Err(DogDiagnostic::new(
                                "E021",
                                format!("Style property '{}' cannot use a 'void' expression.", rule.prop),
                                rule.span.clone(),
                                "Every style rule value must produce a concrete printable value.",
                                "Provide a text, number, decimal, or bool value.",
                                "Example: `bg: \"#F8FAFC\"`.",
                                "Cannot paint pixels with void.",
                            )
                            .with_types("value", "void"));
                        }
                    }
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
            Expr::InterpString { parts, .. } => {
                for part in parts {
                    if let InterpPart::Expr(inner) = part {
                        let ty = self.check_expr(inner)?;
                        if ty == DogType::Void {
                            return Err(DogDiagnostic::new(
                                "E021",
                                "Cannot interpolate a 'void' expression inside $\"...\".",
                                inner.span().clone(),
                                "Interpolated expressions must evaluate to a printable value.",
                                "Pass a text, number, decimal, bool, list, map, struct, or enum expression inside `{...}`.",
                                "Void-returning calls like `say()` produce no value.",
                                "Can't stitch silence into a string.",
                            )
                            .with_types("value", "void"));
                        }
                    }
                }
                Ok(DogType::Text)
            }
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
                    let mut fix = format!("Define `struct {name} ... end` before constructing it.");
                    if let Some(hint) = suggest_similar(name, self.structs.keys().map(String::as_str)) {
                        fix = format!("Did you mean struct '{hint}'? {fix}");
                    }
                    return Err(DogDiagnostic::new(
                        "E010",
                        format!("Unknown struct '{name}'."),
                        span.clone(),
                        format!("No struct named '{name}' has been defined."),
                        fix,
                        "Struct names are case-sensitive.",
                        "Never seen that blueprint.",
                    ));
                };
                let mut seen = HashMap::new();
                for (fname, fexpr) in fields {
                    let Some(expected_ty) = def_fields.get(fname) else {
                        let mut fix = "Check field spelling against the struct definition.".to_string();
                        if let Some(hint) = suggest_similar(fname, def_fields.keys().map(String::as_str)) {
                            fix = format!("Did you mean field '{hint}'? {fix}");
                        }
                        return Err(DogDiagnostic::new(
                            "E010",
                            format!("Unknown field '{fname}' on struct '{name}'."),
                            fexpr.span().clone(),
                            format!("Struct '{name}' does not have a field named '{fname}'."),
                            fix,
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
                        let mut fix = format!(
                            "Use a valid variant like `{name}.{}`.",
                            variants.first().map(String::as_str).unwrap_or("Variant")
                        );
                        if let Some(hint) = suggest_similar(field, variants.iter().map(String::as_str)) {
                            fix = format!("Did you mean `{name}.{hint}`? {fix}");
                        }
                        return Err(DogDiagnostic::new(
                            "E010",
                            format!("Unknown variant '{field}' on enum '{name}'."),
                            span.clone(),
                            format!("Enum '{name}' has variants: {}.", variants.join(", ")),
                            fix,
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
                            let mut fix = "Check the struct definition.".to_string();
                            if let Some(hint) = suggest_similar(field, fields.keys().map(String::as_str)) {
                                fix = format!("Did you mean '.{hint}'? {fix}");
                            }
                            DogDiagnostic::new(
                                "E010",
                                format!("Unknown field '{field}' on struct '{struct_name}'."),
                                span.clone(),
                                format!("Struct '{struct_name}' has no field named '{field}'."),
                                fix,
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
                    let fix = self.suggest_var_fix(
                        name,
                        format!("Declare `{name}` with `let`, `var`, or `const` before reading it."),
                    );
                    DogDiagnostic::new(
                        "E010",
                        format!("Undefined variable '{name}'."),
                        span.clone(),
                        format!("No variable named '{name}' exists in the current scope."),
                        fix,
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
            Expr::NamespaceCall {
                module,
                func,
                args,
                span,
            } => match crate::stdlib::modules::resolve_std_call(module, func) {
                Ok(canonical) => self.check_call(canonical, args, span),
                Err(msg) => Err(DogDiagnostic::new(
                    "E013",
                    msg,
                    span.clone(),
                    "Standard library module calls use `std.<module>.<fn>(...)`.",
                    "Run `dog docs std` to inspect valid standard modules and functions.",
                    "Valid modules include std.io, std.text, std.math, std.files, std.net, std.system, std.data, std.crypto, std.cli, and std.collections.",
                    "Wrong aisle in the standard library.",
                )),
            },
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
                        "Use `$\"text {value}\"` for string interpolation, or convert explicitly with `text()`, `number()`, or `decimal()`.",
                        "Keep your types consistent (`$\"...\"` is easiest when mixing words and numbers).",
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

        if callee == "sort" {
            if args.len() != 1 {
                return Err(arity_error(callee, 1, args.len(), span));
            }
            let inner_ty = self.require_mutable_list_arg(&args[0], callee)?;
            if !matches!(
                inner_ty,
                DogType::Text | DogType::Number | DogType::Decimal
            ) {
                return Err(type_err(
                    "list<text | number | decimal>",
                    &format!("list<{}>", inner_ty.label()),
                    span,
                    "sort() only supports lists of text, number, or decimal.",
                ));
            }
            return Ok(DogType::Void);
        }
        if callee == "reverse" {
            if args.len() != 1 {
                return Err(arity_error(callee, 1, args.len(), span));
            }
            self.require_mutable_list_arg(&args[0], callee)?;
            return Ok(DogType::Void);
        }
        if callee == "list_contains" {
            if args.len() != 2 {
                return Err(arity_error(callee, 2, args.len(), span));
            }
            let list_ty = self.check_expr(&args[0])?;
            let DogType::List(inner) = list_ty else {
                return Err(type_err(
                    "list<T>",
                    &list_ty.label(),
                    args[0].span(),
                    "list_contains() expects a list as first argument.",
                ));
            };
            let item_ty = self.check_expr_with_hint(&args[1], Some(&inner))?;
            if item_ty != *inner {
                return Err(type_err(
                    &inner.label(),
                    &item_ty.label(),
                    args[1].span(),
                    "list_contains() search item type must match the list element type.",
                ));
            }
            return Ok(DogType::Bool);
        }
        if callee == "list_index_of" {
            if args.len() != 2 {
                return Err(arity_error(callee, 2, args.len(), span));
            }
            let list_ty = self.check_expr(&args[0])?;
            let DogType::List(inner) = list_ty else {
                return Err(type_err(
                    "list<T>",
                    &list_ty.label(),
                    args[0].span(),
                    "list_index_of() expects a list as first argument.",
                ));
            };
            let item_ty = self.check_expr_with_hint(&args[1], Some(&inner))?;
            if item_ty != *inner {
                return Err(type_err(
                    &inner.label(),
                    &item_ty.label(),
                    args[1].span(),
                    "list_index_of() search item type must match the list element type.",
                ));
            }
            return Ok(DogType::Number);
        }
        if callee == "list_slice" {
            if args.len() != 3 {
                return Err(arity_error(callee, 3, args.len(), span));
            }
            let list_ty = self.check_expr(&args[0])?;
            let DogType::List(inner) = list_ty else {
                return Err(type_err(
                    "list<T>",
                    &list_ty.label(),
                    args[0].span(),
                    "list_slice() expects a list as first argument.",
                ));
            };
            let start_ty = self.check_expr(&args[1])?;
            let end_ty = self.check_expr(&args[2])?;
            if start_ty != DogType::Number || end_ty != DogType::Number {
                return Err(type_err(
                    "number, number",
                    &format!("{start_ty}, {end_ty}"),
                    span,
                    "list_slice() start and end indices must be 'number'.",
                ));
            }
            return Ok(DogType::List(inner));
        }
        if callee == "insert_at" {
            if args.len() != 3 {
                return Err(arity_error(callee, 3, args.len(), span));
            }
            let inner_ty = self.require_mutable_list_arg(&args[0], callee)?;
            let idx_ty = self.check_expr(&args[1])?;
            if idx_ty != DogType::Number {
                return Err(type_err(
                    "number",
                    &idx_ty.label(),
                    args[1].span(),
                    "insert_at() index must be 'number'.",
                ));
            }
            let item_ty = self.check_expr_with_hint(&args[2], Some(&inner_ty))?;
            if item_ty != inner_ty {
                return Err(type_err(
                    &inner_ty.label(),
                    &item_ty.label(),
                    args[2].span(),
                    "insert_at() value type must match the list element type.",
                ));
            }
            return Ok(DogType::Void);
        }
        if callee == "remove_at" {
            if args.len() != 2 {
                return Err(arity_error(callee, 2, args.len(), span));
            }
            let inner_ty = self.require_mutable_list_arg(&args[0], callee)?;
            let idx_ty = self.check_expr(&args[1])?;
            if idx_ty != DogType::Number {
                return Err(type_err(
                    "number",
                    &idx_ty.label(),
                    args[1].span(),
                    "remove_at() index must be 'number'.",
                ));
            }
            return Ok(inner_ty);
        }
        if callee == "clear" {
            if args.is_empty() {
                return Ok(DogType::Void);
            }
            if args.len() != 1 {
                return Err(arity_error(callee, 1, args.len(), span));
            }
            self.require_mutable_list_arg(&args[0], callee)?;
            return Ok(DogType::Void);
        }
        if callee == "map_clear" {
            if args.len() != 1 {
                return Err(arity_error(callee, 1, args.len(), span));
            }
            self.require_mutable_map_arg(&args[0], callee)?;
            return Ok(DogType::Void);
        }
        if callee == "map_merge" {
            if args.len() != 2 {
                return Err(arity_error(callee, 2, args.len(), span));
            }
            let (k_ty, v_ty) = self.require_mutable_map_arg(&args[0], callee)?;
            let expected_map = DogType::Map(Box::new(k_ty), Box::new(v_ty));
            let src_ty = self.check_expr_with_hint(&args[1], Some(&expected_map))?;
            if src_ty != expected_map {
                return Err(type_err(
                    &expected_map.label(),
                    &src_ty.label(),
                    args[1].span(),
                    "map_merge() source map must have matching key and value types.",
                ));
            }
            return Ok(DogType::Void);
        }

        let mut arg_types = Vec::with_capacity(args.len());
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        match callee {
            "say" | "print" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] == DogType::Void {
                    return Err(DogDiagnostic::new(
                        "E021",
                        format!("Cannot pass 'void' to {callee}()."),
                        span.clone(),
                        format!("{callee}() requires a printable value."),
                        format!("Pass text, number, decimal, bool, list, map, struct, or enum to {callee}()."),
                        "Do not nest say(say(...)) or print(print(...)).",
                        "Saying nothing out loud is called silence.",
                    )
                    .with_types("printable value", "void"));
                }
                Ok(DogType::Void)
            }
            "answer" | "ask" | "password" => {
                if arg_types.len() > 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if let Some(t) = arg_types.first() {
                    if *t != DogType::Text {
                        return Err(type_err("text", &t.label(), span, &format!("{callee}() prompt must be 'text'.")));
                    }
                }
                Ok(DogType::Text)
            }
            "confirm" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "confirm() prompt must be 'text'."));
                }
                Ok(DogType::Bool)
            }
            "choose" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::List(Box::new(DogType::Text))
                {
                    return Err(type_err(
                        "text, list<text>",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "choose() expects (prompt: text, options: list<text>).",
                    ));
                }
                Ok(DogType::Text)
            }
            "read_key" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Text)
            }
            "table" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let expected_headers = DogType::List(Box::new(DogType::Text));
                let expected_rows = DogType::List(Box::new(DogType::List(Box::new(DogType::Text))));
                if arg_types[0] != expected_headers || arg_types[1] != expected_rows {
                    return Err(type_err(
                        "list<text>, list<list<text>>",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "table() expects (headers: list<text>, rows: list<list<text>>).",
                    ));
                }
                Ok(DogType::Void)
            }
            "lower" | "upper" | "trim" | "hex_encode" | "hex_decode" | "base64_encode"
            | "base64_decode" | "sha256" | "file_read" | "http_get" | "url_encode"
            | "url_decode" => {
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
            "replace" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "replace() expects (text, text, text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "starts_with" | "ends_with" | "hmac_sha256" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects two 'text' arguments."),
                    ));
                }
                if callee == "hmac_sha256" {
                    Ok(DogType::Text)
                } else {
                    Ok(DogType::Bool)
                }
            }
            "slice" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Number
                    || arg_types[2] != DogType::Number
                {
                    return Err(type_err(
                        "text, number, number",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "slice() expects (text, number, number).",
                    ));
                }
                Ok(DogType::Text)
            }
            "round" | "floor" | "ceil" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Decimal && arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "decimal | number",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'decimal' or 'number'."),
                    ));
                }
                Ok(DogType::Number)
            }
            "sqrt" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Decimal && arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "decimal | number",
                        &arg_types[0].label(),
                        span,
                        "sqrt() expects 'decimal' or 'number'.",
                    ));
                }
                Ok(DogType::Decimal)
            }
            "pow" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if (arg_types[0] != DogType::Decimal && arg_types[0] != DogType::Number)
                    || (arg_types[1] != DogType::Decimal && arg_types[1] != DogType::Number)
                {
                    return Err(type_err(
                        "decimal, decimal",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "pow() expects two numeric arguments.",
                    ));
                }
                Ok(DogType::Decimal)
            }
            "rand_int" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "rand_int() expects (min: number, max: number).",
                    ));
                }
                Ok(DogType::Number)
            }
            "range" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "range() expects (start: number, end: number).",
                    ));
                }
                Ok(DogType::List(Box::new(DogType::Number)))
            }
            "file_append" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "file_append() expects (path: text, content: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "mkdir" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "mkdir() expects directory path 'text'.",
                    ));
                }
                Ok(DogType::Void)
            }
            "cwd" | "os_name" | "os_arch" | "uuid_v4" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Text)
            }
            "args" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "http_listen" | "tcp_listen" | "tcp_connect" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, &format!("{callee}() expects address 'text' (e.g. \"127.0.0.1:8080\").")));
                }
                Ok(DogType::Number)
            }
            "http_port" | "tcp_port" | "tcp_accept" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err("number", &arg_types[0].label(), span, &format!("{callee}() expects handle 'number'.")));
                }
                Ok(DogType::Number)
            }
            "http_close" | "tcp_close" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err("number", &arg_types[0].label(), span, &format!("{callee}() expects handle 'number'.")));
                }
                Ok(DogType::Void)
            }
            "http_accept" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err("number", &arg_types[0].label(), span, "http_accept() expects server handle 'number'."));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "http_reply" => {
                if arg_types.len() != 4 {
                    return Err(arity_error(callee, 4, arg_types.len(), span));
                }
                let req_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != req_map
                    || arg_types[1] != DogType::Number
                    || arg_types[2] != DogType::Text
                    || arg_types[3] != DogType::Text
                {
                    return Err(type_err(
                        "map<text, text>, number, text, text",
                        &format!("{}, {}, {}, {}", arg_types[0], arg_types[1], arg_types[2], arg_types[3]),
                        span,
                        "http_reply() expects (req: map<text, text>, status: number, content_type: text, body: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "http_serve_file" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let req_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != req_map || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "map<text, text>, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "http_serve_file() expects (req: map<text, text>, path: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "http_post" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "http_post() expects (url: text, body: text, content_type: text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "http_request" => {
                if arg_types.len() != 4 {
                    return Err(arity_error(callee, 4, arg_types.len(), span));
                }
                let hdr_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != hdr_map
                    || arg_types[3] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, map<text, text>, text",
                        &format!("{}, {}, {}, {}", arg_types[0], arg_types[1], arg_types[2], arg_types[3]),
                        span,
                        "http_request() expects (method: text, url: text, headers: map<text, text>, body: text).",
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "tcp_send" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "number, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "tcp_send() expects (conn_id: number, data: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "tcp_recv" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "tcp_recv() expects (conn_id: number, max_bytes: number).",
                    ));
                }
                Ok(DogType::Text)
            }
            "csv_parse" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "csv_parse() expects CSV 'text'."));
                }
                Ok(DogType::List(Box::new(DogType::List(Box::new(DogType::Text)))))
            }
            "kv_set" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "kv_set() expects (db_path: text, key: text, val: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "kv_get" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "kv_get() expects (db_path: text, key: text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "kv_has" | "kv_delete" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects (db_path: text, key: text)."),
                    ));
                }
                Ok(DogType::Bool)
            }
            "kv_keys" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "kv_keys() expects db_path 'text'."));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "exec_cmd" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err("text", &arg_types[0].label(), span, "exec_cmd() expects command 'text'."));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "cli_info" | "cli_success" | "cli_warn" | "cli_error" | "cli_debug" | "cli_theme"
            | "cli_prefix" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Void)
            }
            "cli_timestamps" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Bool {
                    return Err(type_err(
                        "bool",
                        &arg_types[0].label(),
                        span,
                        "cli_timestamps() expects 'bool'.",
                    ));
                }
                Ok(DogType::Void)
            }
            "cli_progress" | "progress" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Number
                    || arg_types[2] != DogType::Number
                {
                    return Err(type_err(
                        "text, number, number",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "cli_progress() expects (label: text, current: number, total: number).",
                    ));
                }
                Ok(DogType::Void)
            }
            "cli_spinner" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "number, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "cli_spinner() expects (step: number, label: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "cli_supports_color" | "cli_is_tty" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Bool)
            }
            "sha1" | "html_escape" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Text)
            }
            "index_of" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "index_of() expects (haystack: text, needle: text).",
                    ));
                }
                Ok(DogType::Number)
            }
            "glob_match" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "glob_match() expects (pattern: text, text_val: text).",
                    ));
                }
                Ok(DogType::Bool)
            }
            "str_repeat" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "text, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "str_repeat() expects (s: text, count: number).",
                    ));
                }
                Ok(DogType::Text)
            }
            "pad_left" | "pad_right" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Number
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, number, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        &format!("{callee}() expects (s: text, width: number, fill: text)."),
                    ));
                }
                Ok(DogType::Text)
            }
            "clamp" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] == arg_types[1]
                    && arg_types[1] == arg_types[2]
                    && (arg_types[0] == DogType::Number || arg_types[0] == DogType::Decimal)
                {
                    Ok(arg_types[0].clone())
                } else {
                    Err(type_err(
                        "T, T, T (number | decimal)",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "clamp() expects matching numeric (val, min, max) arguments.",
                    ))
                }
            }
            "sin" | "cos" | "log" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Decimal && arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "decimal | number",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'decimal' or 'number'."),
                    ));
                }
                Ok(DogType::Decimal)
            }
            "map_values" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let DogType::Map(_, v) = &arg_types[0] else {
                    return Err(type_err(
                        "map<K, V>",
                        &arg_types[0].label(),
                        span,
                        "map_values() expects a map.",
                    ));
                };
                Ok(DogType::List(v.clone()))
            }
            "time_utc" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Text)
            }
            "json_pretty" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] == DogType::Void {
                    return Err(type_err(
                        "non-void",
                        "void",
                        span,
                        "Cannot format 'void' as JSON.",
                    ));
                }
                Ok(DogType::Text)
            }
            "file_copy" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "file_copy() expects (src: text, dst: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "file_size" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "file_size() expects file path 'text'.",
                    ));
                }
                Ok(DogType::Number)
            }
            "is_file" | "is_dir" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects path 'text'."),
                    ));
                }
                Ok(DogType::Bool)
            }
            "path_join" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "path_join() expects (base: text, child: text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "url_parse" | "query_parse" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "query_stringify" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let expected = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != expected {
                    return Err(type_err(
                        "map<text, text>",
                        &arg_types[0].label(),
                        span,
                        "query_stringify() expects map<text, text>.",
                    ));
                }
                Ok(DogType::Text)
            }
            "http_route_match" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "http_route_match() expects (pattern: text, path: text).",
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "http_try_accept" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "http_try_accept() expects (server_id: number, timeout_ms: number).",
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "http_reply_headers" => {
                if arg_types.len() != 4 {
                    return Err(arity_error(callee, 4, arg_types.len(), span));
                }
                let text_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != text_map
                    || arg_types[1] != DogType::Number
                    || arg_types[2] != text_map
                    || arg_types[3] != DogType::Text
                {
                    return Err(type_err(
                        "map<text, text>, number, map<text, text>, text",
                        &format!(
                            "{}, {}, {}, {}",
                            arg_types[0], arg_types[1], arg_types[2], arg_types[3]
                        ),
                        span,
                        "http_reply_headers() expects (req, status, headers_map, body).",
                    ));
                }
                Ok(DogType::Void)
            }
            "http_redirect" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                let req_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != req_map
                    || arg_types[1] != DogType::Number
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "map<text, text>, number, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "http_redirect() expects (req, status, location).",
                    ));
                }
                Ok(DogType::Void)
            }
            "http_request_timeout" => {
                if arg_types.len() != 5 {
                    return Err(arity_error(callee, 5, arg_types.len(), span));
                }
                let hdr_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != hdr_map
                    || arg_types[3] != DogType::Text
                    || arg_types[4] != DogType::Number
                {
                    return Err(type_err(
                        "text, text, map<text, text>, text, number",
                        &format!(
                            "{}, {}, {}, {}, {}",
                            arg_types[0], arg_types[1], arg_types[2], arg_types[3], arg_types[4]
                        ),
                        span,
                        "http_request_timeout() expects (method, url, headers, body, timeout_ms).",
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "http_put" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "http_put() expects (url: text, body: text, content_type: text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "http_delete" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "http_delete() expects url 'text'.",
                    ));
                }
                Ok(DogType::Text)
            }
            "ws_upgrade" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let req_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != req_map {
                    return Err(type_err(
                        "map<text, text>",
                        &arg_types[0].label(),
                        span,
                        "ws_upgrade() expects request map<text, text>.",
                    ));
                }
                Ok(DogType::Number)
            }
            "ws_connect" | "udp_bind" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Number)
            }
            "ws_send" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "number, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "ws_send() expects (ws_id: number, message: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "ws_recv" | "tcp_remote_addr" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "number",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects handle 'number'."),
                    ));
                }
                Ok(DogType::Text)
            }
            "ws_close" | "udp_close" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "number",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects handle 'number'."),
                    ));
                }
                Ok(DogType::Void)
            }
            "udp_send_to" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "number, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "udp_send_to() expects (sock_id: number, addr: text, data: text).",
                    ));
                }
                Ok(DogType::Number)
            }
            "udp_recv_from" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "udp_recv_from() expects (sock_id: number, max_bytes: number).",
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "dns_lookup" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "dns_lookup() expects hostname 'text'.",
                    ));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "tcp_set_timeout" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "tcp_set_timeout() expects (conn_id: number, timeout_ms: number).",
                    ));
                }
                Ok(DogType::Void)
            }
            "port_is_open" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "text, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "port_is_open() expects (addr: text, timeout_ms: number).",
                    ));
                }
                Ok(DogType::Bool)
            }
            "char_at" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "text, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "char_at() expects (s: text, idx: number).",
                    ));
                }
                Ok(DogType::Text)
            }
            "last_index_of" | "count_matches" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects (haystack: text, needle: text)."),
                    ));
                }
                Ok(DogType::Number)
            }
            "remove_prefix" | "remove_suffix" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects (s: text, fix: text)."),
                    ));
                }
                Ok(DogType::Text)
            }
            "title_case" | "reverse_text" | "path_dirname" | "path_basename" | "path_ext"
            | "sha512" | "base64_url_encode" | "base64_url_decode" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Text)
            }
            "lines" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "lines() expects 'text'.",
                    ));
                }
                Ok(DogType::List(Box::new(DogType::Text)))
            }
            "tan" | "exp" | "log10" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Decimal && arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "decimal | number",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'decimal' or 'number'."),
                    ));
                }
                Ok(DogType::Decimal)
            }
            "atan2" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let ok0 = arg_types[0] == DogType::Decimal || arg_types[0] == DogType::Number;
                let ok1 = arg_types[1] == DogType::Decimal || arg_types[1] == DogType::Number;
                if !ok0 || !ok1 {
                    return Err(type_err(
                        "decimal | number, decimal | number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "atan2() expects two numeric arguments (y, x).",
                    ));
                }
                Ok(DogType::Decimal)
            }
            "pow_int" | "gcd" | "lcm" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number, number",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects two 'number' arguments."),
                    ));
                }
                Ok(DogType::Number)
            }
            "sign" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number && arg_types[0] != DogType::Decimal {
                    return Err(type_err(
                        "number | decimal",
                        &arg_types[0].label(),
                        span,
                        "sign() expects 'number' or 'decimal'.",
                    ));
                }
                Ok(DogType::Number)
            }
            "math_pi" | "math_e" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Decimal)
            }
            "temp_dir" | "local_ip" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Text)
            }
            "list_unique" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let DogType::List(_) = &arg_types[0] else {
                    return Err(type_err(
                        "list<T>",
                        &arg_types[0].label(),
                        span,
                        "list_unique() expects a list.",
                    ));
                };
                Ok(arg_types[0].clone())
            }
            "list_take" | "list_skip" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let DogType::List(_) = &arg_types[0] else {
                    return Err(type_err(
                        "list<T>",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects (list: list<T>, n: number)."),
                    ));
                };
                if arg_types[1] != DogType::Number {
                    return Err(type_err(
                        "number",
                        &arg_types[1].label(),
                        span,
                        &format!("{callee}() count must be 'number'."),
                    ));
                }
                Ok(arg_types[0].clone())
            }
            "list_flatten" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let DogType::List(outer) = &arg_types[0] else {
                    return Err(type_err(
                        "list<list<T>>",
                        &arg_types[0].label(),
                        span,
                        "list_flatten() expects list<list<T>>.",
                    ));
                };
                let DogType::List(_) = outer.as_ref() else {
                    return Err(type_err(
                        "list<list<T>>",
                        &arg_types[0].label(),
                        span,
                        "list_flatten() expects nested lists list<list<T>>.",
                    ));
                };
                Ok((**outer).clone())
            }
            "list_sum" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let DogType::List(inner) = &arg_types[0] else {
                    return Err(type_err(
                        "list<number | decimal>",
                        &arg_types[0].label(),
                        span,
                        "list_sum() expects list<number> or list<decimal>.",
                    ));
                };
                if **inner != DogType::Number && **inner != DogType::Decimal {
                    return Err(type_err(
                        "list<number | decimal>",
                        &arg_types[0].label(),
                        span,
                        "list_sum() only supports numeric lists.",
                    ));
                }
                Ok((**inner).clone())
            }
            "list_avg" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let DogType::List(inner) = &arg_types[0] else {
                    return Err(type_err(
                        "list<number | decimal>",
                        &arg_types[0].label(),
                        span,
                        "list_avg() expects list<number> or list<decimal>.",
                    ));
                };
                if **inner != DogType::Number && **inner != DogType::Decimal {
                    return Err(type_err(
                        "list<number | decimal>",
                        &arg_types[0].label(),
                        span,
                        "list_avg() only supports numeric lists.",
                    ));
                }
                Ok(DogType::Decimal)
            }
            "list_concat" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let DogType::List(_) = &arg_types[0] else {
                    return Err(type_err(
                        "list<T>, list<T>",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "list_concat() expects two lists of identical element type.",
                    ));
                };
                if arg_types[0] != arg_types[1] {
                    return Err(type_err(
                        &arg_types[0].label(),
                        &arg_types[1].label(),
                        span,
                        "list_concat() requires matching list element types.",
                    ));
                }
                Ok(arg_types[0].clone())
            }
            "map_from_lists" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let (DogType::List(k_ty), DogType::List(v_ty)) = (&arg_types[0], &arg_types[1])
                else {
                    return Err(type_err(
                        "list<K>, list<V>",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "map_from_lists() expects (keys: list<K>, values: list<V>).",
                    ));
                };
                Ok(DogType::Map(k_ty.clone(), v_ty.clone()))
            }
            "map_get_or" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                let DogType::Map(k_ty, v_ty) = &arg_types[0] else {
                    return Err(type_err(
                        "map<K, V>",
                        &arg_types[0].label(),
                        span,
                        "map_get_or() expects a map as its first argument.",
                    ));
                };
                if arg_types[1] != **k_ty || arg_types[2] != **v_ty {
                    return Err(type_err(
                        &format!("map<{}, {}>, {}, {}", k_ty, v_ty, k_ty, v_ty),
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "map_get_or() expects (map, key, fallback) matching the map's types.",
                    ));
                }
                Ok((**v_ty).clone())
            }
            "csv_stringify" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let expected = DogType::List(Box::new(DogType::List(Box::new(DogType::Text))));
                if arg_types[0] != expected {
                    return Err(type_err(
                        "list<list<text>>",
                        &arg_types[0].label(),
                        span,
                        "csv_stringify() expects list<list<text>>.",
                    ));
                }
                Ok(DogType::Text)
            }
            "json_has_key" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "json_has_key() expects (raw_json: text, key: text).",
                    ));
                }
                Ok(DogType::Bool)
            }
            "file_move" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "file_move() expects (src: text, dst: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "rmdir" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "rmdir() expects directory path 'text'.",
                    ));
                }
                Ok(DogType::Void)
            }
            "hmac_sha256_verify" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "hmac_sha256_verify() expects (key: text, msg: text, sig_hex: text).",
                    ));
                }
                Ok(DogType::Bool)
            }
            "rand_bytes_hex" | "http_status_text" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number {
                    return Err(type_err(
                        "number",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'number'."),
                    ));
                }
                Ok(DogType::Text)
            }
            "ws_ping" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "number, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "ws_ping() expects (ws_id: number, msg: text).",
                    ));
                }
                Ok(DogType::Void)
            }
            "json_read" | "toml_parse" | "cookie_parse" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text)))
            }
            "json_write" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "json_write() expects file path 'text' as first argument.",
                    ));
                }
                if arg_types[1] == DogType::Void {
                    return Err(type_err(
                        "non-void",
                        "void",
                        span,
                        "Cannot serialize 'void' to JSON file.",
                    ));
                }
                Ok(DogType::Void)
            }
            "time_format" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Number || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "number, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "time_format() expects (epoch_ms: number, fmt: text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "env_load" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        "env_load() expects `.env` file path 'text'.",
                    ));
                }
                Ok(DogType::Number)
            }
            "dog_css" | "dom_get_value" | "web_storage_get" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Text)
            }
            "web_route" => {
                if !arg_types.is_empty() {
                    return Err(arity_error(callee, 0, arg_types.len(), span));
                }
                Ok(DogType::Text)
            }
            "web_title" | "web_navigate" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text {
                    return Err(type_err(
                        "text",
                        &arg_types[0].label(),
                        span,
                        &format!("{callee}() expects 'text'."),
                    ));
                }
                Ok(DogType::Void)
            }
            "css_vars" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let expected = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != expected {
                    return Err(type_err(
                        "map<text, text>",
                        &arg_types[0].label(),
                        span,
                        "css_vars() expects map<text, text>.",
                    ));
                }
                Ok(DogType::Text)
            }
            "css_rule" | "css_keyframes" | "web_nav" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let expected_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != DogType::Text || arg_types[1] != expected_map {
                    return Err(type_err(
                        "text, map<text, text>",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects (text, map<text, text>)."),
                    ));
                }
                Ok(DogType::Text)
            }
            "css_media" | "web_card" | "web_button" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects (text, text)."),
                    ));
                }
                Ok(DogType::Text)
            }
            "css_bundle" => {
                if arg_types.len() != 1 {
                    return Err(arity_error(callee, 1, arg_types.len(), span));
                }
                let expected = DogType::List(Box::new(DogType::Text));
                if arg_types[0] != expected {
                    return Err(type_err(
                        "list<text>",
                        &arg_types[0].label(),
                        span,
                        "css_bundle() expects list<text>.",
                    ));
                }
                Ok(DogType::Text)
            }
            "css_theme" => {
                if arg_types.len() != 5 {
                    return Err(arity_error(callee, 5, arg_types.len(), span));
                }
                for t in &arg_types {
                    if *t != DogType::Text {
                        return Err(type_err(
                            "text",
                            &t.label(),
                            span,
                            "css_theme() expects (bg, surface, ink, accent, font) all as 'text'.",
                        ));
                    }
                }
                Ok(DogType::Text)
            }
            "html_el" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                let expected_map = DogType::Map(Box::new(DogType::Text), Box::new(DogType::Text));
                if arg_types[0] != DogType::Text
                    || arg_types[1] != expected_map
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, map<text, text>, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        "html_el() expects (tag: text, attrs: map<text, text>, content: text).",
                    ));
                }
                Ok(DogType::Text)
            }
            "html_page" | "web_input" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        &format!("{callee}() expects three 'text' arguments."),
                    ));
                }
                Ok(DogType::Text)
            }
            "web_grid" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                let expected_list = DogType::List(Box::new(DogType::Text));
                if arg_types[0] != DogType::Number || arg_types[1] != expected_list {
                    return Err(type_err(
                        "number, list<text>",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "web_grid() expects (cols: number, items: list<text>).",
                    ));
                }
                Ok(DogType::Text)
            }
            "dom_mount" | "dom_set_text" | "dom_set_html" | "dom_set_value" | "dom_add_class"
            | "dom_remove_class" | "web_storage_set" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        &format!("{callee}() expects two 'text' arguments."),
                    ));
                }
                Ok(DogType::Void)
            }
            "dom_toggle_class" => {
                if arg_types.len() != 2 {
                    return Err(arity_error(callee, 2, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text || arg_types[1] != DogType::Text {
                    return Err(type_err(
                        "text, text",
                        &format!("{}, {}", arg_types[0], arg_types[1]),
                        span,
                        "dom_toggle_class() expects (selector: text, cls: text).",
                    ));
                }
                Ok(DogType::Bool)
            }
            "dom_set_style" | "dom_on" => {
                if arg_types.len() != 3 {
                    return Err(arity_error(callee, 3, arg_types.len(), span));
                }
                if arg_types[0] != DogType::Text
                    || arg_types[1] != DogType::Text
                    || arg_types[2] != DogType::Text
                {
                    return Err(type_err(
                        "text, text, text",
                        &format!("{}, {}, {}", arg_types[0], arg_types[1], arg_types[2]),
                        span,
                        &format!("{callee}() expects three 'text' arguments."),
                    ));
                }
                Ok(DogType::Void)
            }
            other => {
                let Some(sig) = self.functions.get(other) else {
                    let mut fix = "Check spelling or define the function with `fn`.".to_string();
                    let all_fns = BUILTIN_NAMES
                        .iter()
                        .copied()
                        .chain(self.functions.keys().map(String::as_str));
                    if let Some(hint) = suggest_similar(callee, all_fns) {
                        fix = format!("Did you mean '{hint}()'? {fix}");
                    }
                    return Err(DogDiagnostic::new(
                        "E013",
                        format!("Unknown function '{callee}'."),
                        span.clone(),
                        format!("No built-in or declared function named '{callee}' exists."),
                        fix,
                        "Top-level functions are visible throughout the file.",
                        "I looked everywhere for that function. Nobody home.",
                    ));
                };
                if args.len() < sig.min_args || args.len() > sig.params.len() {
                    if sig.min_args == sig.params.len() {
                        return Err(arity_error(callee, sig.params.len(), args.len(), span));
                    } else {
                        return Err(DogDiagnostic::new(
                            "E014",
                            format!("Incorrect argument count for '{callee}()'."),
                            span.clone(),
                            format!(
                                "'{callee}()' expects {} to {} argument(s), but received {}.",
                                sig.min_args,
                                sig.params.len(),
                                args.len()
                            ),
                            format!(
                                "Pass between {} and {} argument(s) to {callee}().",
                                sig.min_args,
                                sig.params.len()
                            ),
                            "Trailing parameters with default values may be omitted.",
                            "Count on your paws and try again.",
                        ));
                    }
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

    fn all_visible_var_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for scope in &self.scopes {
            for k in scope.keys() {
                names.push(k.as_str());
            }
        }
        names
    }

    fn suggest_var_fix(&self, name: &str, default_fix: String) -> String {
        if let Some(hint) = suggest_similar(name, self.all_visible_var_names()) {
            format!("Did you mean '{hint}'? {default_fix}")
        } else {
            default_fix
        }
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
