use crate::ast::{BinOp, BindingKind, Expr, InterpPart, Program, Stmt, TypeRef, UnaryOp};
use crate::checker::TypeChecker;
use crate::crypto::sha256_hex;
use crate::diagnostics::DogDiagnostic;
use crate::interpreter::{check_source, resolve_program, Interpreter};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::stdlib::InterpreterIo;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 6] = b"DOG0\x00\x01";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct DgbBundle {
    pub version: u32,
    pub built_at_secs: u64,
    pub sha256: String,
    pub entry_file: String,
    pub source: String,
}

pub fn is_dgb_path(path: &str) -> bool {
    path.ends_with(".dgb")
}

pub fn build_bundle(
    input_path: &str,
    out_path: Option<&str>,
) -> Result<(PathBuf, DgbBundle), String> {
    let source = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read '{input_path}': {e}"))?;

    // Bundle all imported .dg files cleanly or verify single/multi-file source
    let flattened = flatten_imports(input_path, &source)
        .map_err(|d| format!("{}", d))?;

    // Strict check before emitting bytecode artifact
    check_source(input_path, &source).map_err(|d| format!("{}", d))?;

    let digest = sha256_hex(flattened.as_bytes());
    let built_at_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let bundle = DgbBundle {
        version: FORMAT_VERSION,
        built_at_secs,
        sha256: digest,
        entry_file: Path::new(input_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(input_path)
            .to_string(),
        source: flattened,
    };

    let target = match out_path {
        Some(p) => PathBuf::from(p),
        None => {
            let p = Path::new(input_path);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
            match p.parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.join(format!("{stem}.dgb")),
                _ => PathBuf::from(format!("{stem}.dgb")),
            }
        }
    };

    let bytes = encode_bundle(&bundle);
    fs::write(&target, bytes)
        .map_err(|e| format!("Failed to write bundle '{}': {e}", target.display()))?;

    Ok((target, bundle))
}

pub fn encode_bundle(bundle: &DgbBundle) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&bundle.version.to_le_bytes());
    out.extend_from_slice(&bundle.built_at_secs.to_le_bytes());

    let hash_bytes = bundle.sha256.as_bytes();
    out.extend_from_slice(&(hash_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(hash_bytes);

    let name_bytes = bundle.entry_file.as_bytes();
    out.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(name_bytes);

    let src_bytes = bundle.source.as_bytes();
    out.extend_from_slice(&(src_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(src_bytes);
    out
}

pub fn decode_bundle(bytes: &[u8]) -> Result<DgbBundle, String> {
    if bytes.len() < 18 || &bytes[..6] != MAGIC {
        return Err("Invalid .dgb file: missing DOG0 magic header.".to_string());
    }
    let mut pos = 6usize;

    let version = read_u32(bytes, &mut pos)?;
    if version != FORMAT_VERSION {
        return Err(format!("Unsupported .dgb format version: {version}"));
    }
    let built_at_secs = read_u64(bytes, &mut pos)?;
    let sha256 = read_str(bytes, &mut pos)?;
    let entry_file = read_str(bytes, &mut pos)?;
    let source = read_str(bytes, &mut pos)?;

    let computed = sha256_hex(source.as_bytes());
    if computed != sha256 {
        return Err(format!(
            "Integrity check failed for .dgb bundle (expected {sha256}, got {computed})."
        ));
    }

    Ok(DgbBundle {
        version,
        built_at_secs,
        sha256,
        entry_file,
        source,
    })
}

pub fn run_bundle_with_io(path: &str, io: &mut dyn InterpreterIo) -> Result<(), String> {
    let raw = fs::read(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    let bundle = decode_bundle(&raw)?;
    let program = check_source(&bundle.entry_file, &bundle.source)
        .map_err(|d| format!("{d}"))?;
    let mut interp = Interpreter::new(io);
    interp.eval_program(&program).map_err(|d| format!("{d}"))
}

pub fn build_native(input_path: &str, out_path: Option<&str>) -> Result<PathBuf, String> {
    let rustc_check = Command::new("rustc").arg("--version").output();
    match rustc_check {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!("`rustc` returned an error: {err}"));
        }
        Err(_) => {
            return Err(
                "`rustc` was not found on PATH. Native compilation (`--native`) requires the Rust compiler; omit `--native` to build a portable `.dgb` bundle instead.".to_string(),
            );
        }
    }

    let source = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read '{input_path}': {e}"))?;
    let program = resolve_program(input_path, &source).map_err(|d| format!("{d}"))?;
    let mut checker = TypeChecker::new();
    checker.check(&program).map_err(|d| format!("{d}"))?;

    let rs_code = transpile_program_to_rust(&program)?;

    let target = match out_path {
        Some(p) => PathBuf::from(p),
        None => {
            let p = Path::new(input_path);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("dog_out");
            match p.parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.join(stem),
                _ => PathBuf::from(stem),
            }
        }
    };

    let tmp_rs = env::temp_dir().join(format!(
        "dog_native_{}.rs",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&tmp_rs, rs_code)
        .map_err(|e| format!("Failed to write temporary Rust source: {e}"))?;

    let output = Command::new("rustc")
        .arg("-O")
        .arg(&tmp_rs)
        .arg("-o")
        .arg(&target)
        .output();
    let _ = fs::remove_file(&tmp_rs);

    let out = output.map_err(|e| format!("Failed to invoke `rustc`: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Native `rustc` compilation failed:\n{stderr}"));
    }

    Ok(target)
}

fn flatten_imports(entry_file: &str, source: &str) -> Result<String, DogDiagnostic> {
    let base_dir = Path::new(entry_file)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut visited = HashSet::new();
    let mut active = HashSet::new();
    flatten_recursive(entry_file, source, &base_dir, &mut visited, &mut active)
}

fn flatten_recursive(
    file: &str,
    source: &str,
    base_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    active: &mut HashSet<PathBuf>,
) -> Result<String, DogDiagnostic> {
    let tokens = Lexer::new(file, source).tokenize()?;
    let program = Parser::new(tokens).parse()?;

    let mut out = String::new();
    for stmt in &program.statements {
        if let Stmt::Use { path, span } = stmt {
            if path.starts_with("std.") {
                if crate::stdlib::modules::is_std_module(path) {
                    continue;
                }
                return Err(DogDiagnostic::new(
                    "E040",
                    format!("Unknown standard library module '{path}'."),
                    span.clone(),
                    format!("'{path}' is not a built-in DOG standard library module."),
                    "Inspect built-in modules with `dog docs std`.",
                    "Available std modules: std.io, std.text, std.math, std.files, std.net, std.system, std.data, std.crypto, std.cli, std.collections.",
                    "Missing standard module.",
                ));
            }
            let target = base_dir.join(path);
            let canonical = fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
            if active.contains(&canonical) {
                return Err(DogDiagnostic::new(
                    "E041",
                    format!("Circular module import detected for '{path}'."),
                    span.clone(),
                    "Circular `use` imports cannot be bundled.",
                    "Break the cycle.",
                    "Keep module dependencies acyclic.",
                    "Circular chasing.",
                ));
            }
            if visited.contains(&canonical) {
                continue;
            }
            let mod_src = fs::read_to_string(&target).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to load module '{path}'."),
                    span.clone(),
                    format!("Could not read '{}': {e}", target.display()),
                    "Verify the relative `.dg` path.",
                    "Check module file location.",
                    "Missing module.",
                )
            })?;
            active.insert(canonical.clone());
            let mod_dir = target.parent().unwrap_or(base_dir);
            let sub = flatten_recursive(
                &target.display().to_string(),
                &mod_src,
                mod_dir,
                visited,
                active,
            )?;
            active.remove(&canonical);
            visited.insert(canonical);
            out.push_str(&sub);
            out.push('\n');
        }
    }

    for line in source.lines() {
        if line.trim_start().starts_with("use ") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
    if *pos + 4 > bytes.len() {
        return Err("Truncated .dgb header (u32).".to_string());
    }
    let val = u32::from_le_bytes(bytes[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Ok(val)
}

fn read_u64(bytes: &[u8], pos: &mut usize) -> Result<u64, String> {
    if *pos + 8 > bytes.len() {
        return Err("Truncated .dgb header (u64).".to_string());
    }
    let val = u64::from_le_bytes(bytes[*pos..*pos + 8].try_into().unwrap());
    *pos += 8;
    Ok(val)
}

fn read_str(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_u32(bytes, pos)? as usize;
    if *pos + len > bytes.len() {
        return Err("Truncated .dgb payload.".to_string());
    }
    let s = String::from_utf8(bytes[*pos..*pos + len].to_vec())
        .map_err(|_| "Corrupted UTF-8 string inside .dgb bundle.".to_string())?;
    *pos += len;
    Ok(s)
}

pub fn transpile_program_to_rust(program: &Program) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("use std::io::{self, BufRead, Write};\n");
    out.push_str("use std::collections::BTreeMap;\n\n");
    out.push_str("trait DogPrint { fn dog_print(&self) -> String; }\n");
    out.push_str("impl DogPrint for String { fn dog_print(&self) -> String { self.clone() } }\n");
    out.push_str("impl DogPrint for i64 { fn dog_print(&self) -> String { self.to_string() } }\n");
    out.push_str("impl DogPrint for f64 { fn dog_print(&self) -> String { self.to_string() } }\n");
    out.push_str("impl DogPrint for bool { fn dog_print(&self) -> String { self.to_string() } }\n");
    out.push_str("impl<T: std::fmt::Debug> DogPrint for Vec<T> { fn dog_print(&self) -> String { format!(\"{self:?}\") } }\n");
    out.push_str("impl<K: std::fmt::Debug, V: std::fmt::Debug> DogPrint for BTreeMap<K, V> { fn dog_print(&self) -> String { format!(\"{self:?}\") } }\n\n");
    out.push_str("trait DogCast { fn dog_to_number(&self) -> i64; fn dog_to_decimal(&self) -> f64; }\n");
    out.push_str("impl DogCast for String {\n");
    out.push_str("    fn dog_to_number(&self) -> i64 { self.trim().parse::<i64>().expect(\"number() failed\") }\n");
    out.push_str("    fn dog_to_decimal(&self) -> f64 { self.trim().parse::<f64>().expect(\"decimal() failed\") }\n");
    out.push_str("}\n");
    out.push_str("impl DogCast for i64 {\n");
    out.push_str("    fn dog_to_number(&self) -> i64 { *self }\n");
    out.push_str("    fn dog_to_decimal(&self) -> f64 { *self as f64 }\n");
    out.push_str("}\n");
    out.push_str("impl DogCast for f64 {\n");
    out.push_str("    fn dog_to_number(&self) -> i64 { *self as i64 }\n");
    out.push_str("    fn dog_to_decimal(&self) -> f64 { *self }\n");
    out.push_str("}\n");
    out.push_str("trait DogAddAssign<R> { fn dog_add_assign(&mut self, rhs: R); }\n");
    out.push_str("impl DogAddAssign<String> for String { fn dog_add_assign(&mut self, rhs: String) { self.push_str(&rhs); } }\n");
    out.push_str("impl DogAddAssign<i64> for i64 { fn dog_add_assign(&mut self, rhs: i64) { *self = self.wrapping_add(rhs); } }\n");
    out.push_str("impl DogAddAssign<f64> for f64 { fn dog_add_assign(&mut self, rhs: f64) { *self += rhs; } }\n\n");
    out.push_str("fn dog_answer(prompt: &str) -> String {\n");
    out.push_str("    if !prompt.is_empty() { print!(\"{prompt}\"); let _ = io::stdout().flush(); }\n");
    out.push_str("    let mut buf = String::new();\n");
    out.push_str("    let _ = io::stdin().lock().read_line(&mut buf);\n");
    out.push_str("    buf.trim_end_matches(&['\\r', '\\n'][..]).to_string()\n");
    out.push_str("}\n\n");

    let mut fn_defaults: HashMap<String, Vec<Option<Expr>>> = HashMap::new();
    for stmt in &program.statements {
        if let Stmt::FnDef { name, params, .. } = stmt {
            fn_defaults.insert(
                name.clone(),
                params.iter().map(|p| p.default.clone()).collect(),
            );
        }
    }

    let mut top_stmts = Vec::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::StructDef { name, fields, .. } => {
                out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
                out.push_str(&format!("struct {name} {{\n"));
                for f in fields {
                    out.push_str(&format!("    {}: {},\n", f.name, type_ref_to_rust(&f.ty)?));
                }
                out.push_str("}\n");
                out.push_str(&format!(
                    "impl DogPrint for {name} {{ fn dog_print(&self) -> String {{ format!(\"{{self:?}}\") }} }}\n\n"
                ));
            }
            Stmt::EnumDef { name, variants, .. } => {
                out.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
                out.push_str(&format!("enum {name} {{\n"));
                for v in variants {
                    out.push_str(&format!("    {v},\n"));
                }
                out.push_str("}\n");
                out.push_str(&format!(
                    "impl DogPrint for {name} {{ fn dog_print(&self) -> String {{ format!(\"{name}.{{self:?}}\") }} }}\n\n"
                ));
            }
            Stmt::FnDef {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                let mut p_strs = Vec::new();
                for p in params {
                    p_strs.push(format!("{}: {}", p.name, type_ref_to_rust(&p.ty)?));
                }
                let ret = if matches!(return_type, TypeRef::Void) {
                    String::new()
                } else {
                    format!(" -> {}", type_ref_to_rust(return_type)?)
                };
                out.push_str(&format!("fn {name}({}){ret} {{\n", p_strs.join(", ")));
                for s in body {
                    emit_stmt(&mut out, s, 1, &fn_defaults)?;
                }
                out.push_str("}\n\n");
            }
            _ => top_stmts.push(stmt),
        }
    }

    out.push_str("fn main() {\n");
    for s in top_stmts {
        emit_stmt(&mut out, s, 1, &fn_defaults)?;
    }
    out.push_str("}\n");
    Ok(out)
}

fn type_ref_to_rust(tr: &TypeRef) -> Result<String, String> {
    match tr {
        TypeRef::Text => Ok("String".to_string()),
        TypeRef::Number => Ok("i64".to_string()),
        TypeRef::Decimal => Ok("f64".to_string()),
        TypeRef::Bool => Ok("bool".to_string()),
        TypeRef::Void => Ok("()".to_string()),
        TypeRef::List(inner) => Ok(format!("Vec<{}>", type_ref_to_rust(inner)?)),
        TypeRef::Map(k, v) => Ok(format!(
            "BTreeMap<{}, {}>",
            type_ref_to_rust(k)?,
            type_ref_to_rust(v)?
        )),
        TypeRef::Named(n) => Ok(n.clone()),
    }
}

fn emit_stmt(
    out: &mut String,
    stmt: &Stmt,
    indent: usize,
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<(), String> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Use { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } | Stmt::FnDef { .. } => {
            Ok(())
        }
        Stmt::Binding {
            kind, name, value, ..
        } => {
            let mut_kw = match kind {
                BindingKind::Var => "let mut",
                BindingKind::Let | BindingKind::Const => "let",
            };
            out.push_str(&format!(
                "{pad}{mut_kw} {name} = {};\n",
                emit_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::Assign { name, value, .. } => {
            out.push_str(&format!(
                "{pad}{name} = {};\n",
                emit_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::CompoundAssign {
            name, op, value, ..
        } => {
            let rhs = emit_expr(value, fn_defaults)?;
            match op {
                BinOp::Add => out.push_str(&format!("{pad}{name}.dog_add_assign({rhs});\n")),
                BinOp::Sub => out.push_str(&format!("{pad}{name} -= {rhs};\n")),
                BinOp::Mul => out.push_str(&format!("{pad}{name} *= {rhs};\n")),
                BinOp::Div => out.push_str(&format!("{pad}{name} /= {rhs};\n")),
                BinOp::Mod => out.push_str(&format!("{pad}{name} %= {rhs};\n")),
                _ => return Err("Unsupported compound assignment operator.".to_string()),
            }
            Ok(())
        }
        Stmt::FieldAssign {
            target,
            field,
            value,
            ..
        } => {
            out.push_str(&format!(
                "{pad}{target}.{field} = {};\n",
                emit_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::IndexAssign {
            target,
            index,
            value,
            ..
        } => {
            out.push_str(&format!(
                "{pad}{target}[({}) as usize] = {};\n",
                emit_expr(index, fn_defaults)?,
                emit_expr(value, fn_defaults)?
            ));
            Ok(())
        }
        Stmt::If {
            condition,
            then_branch,
            otherwise_branch,
            ..
        } => {
            out.push_str(&format!("{pad}if {} {{\n", emit_expr(condition, fn_defaults)?));
            for s in then_branch {
                emit_stmt(out, s, indent + 1, fn_defaults)?;
            }
            if let Some(else_stmts) = otherwise_branch {
                out.push_str(&format!("{pad}}} else {{\n"));
                for s in else_stmts {
                    emit_stmt(out, s, indent + 1, fn_defaults)?;
                }
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::While {
            condition, body, ..
        } => {
            out.push_str(&format!("{pad}while {} {{\n", emit_expr(condition, fn_defaults)?));
            for s in body {
                emit_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::Repeat { count, body, .. } => {
            out.push_str(&format!("{pad}for _ in 0..{} {{\n", emit_expr(count, fn_defaults)?));
            for s in body {
                emit_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::For {
            var_name,
            iterable,
            body,
            ..
        } => {
            out.push_str(&format!(
                "{pad}for {var_name} in {}.into_iter() {{\n",
                emit_expr(iterable, fn_defaults)?
            ));
            for s in body {
                emit_stmt(out, s, indent + 1, fn_defaults)?;
            }
            out.push_str(&format!("{pad}}}\n"));
            Ok(())
        }
        Stmt::Match {
            subject,
            cases,
            otherwise_branch,
            ..
        } => {
            out.push_str(&format!(
                "{pad}let __dog_match = {};\n",
                emit_expr(subject, fn_defaults)?
            ));
            for (idx, c) in cases.iter().enumerate() {
                let kw = if idx == 0 { "if" } else { "} else if" };
                out.push_str(&format!(
                    "{pad}{kw} __dog_match == {} {{\n",
                    emit_expr(&c.pattern, fn_defaults)?
                ));
                for s in &c.body {
                    emit_stmt(out, s, indent + 1, fn_defaults)?;
                }
            }
            if let Some(else_stmts) = otherwise_branch {
                if cases.is_empty() {
                    for s in else_stmts {
                        emit_stmt(out, s, indent, fn_defaults)?;
                    }
                    return Ok(());
                } else {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    for s in else_stmts {
                        emit_stmt(out, s, indent + 1, fn_defaults)?;
                    }
                }
            }
            if !cases.is_empty() {
                out.push_str(&format!("{pad}}}\n"));
            }
            Ok(())
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                out.push_str(&format!("{pad}return {};\n", emit_expr(v, fn_defaults)?));
            } else {
                out.push_str(&format!("{pad}return;\n"));
            }
            Ok(())
        }
        Stmt::Break { .. } => {
            out.push_str(&format!("{pad}break;\n"));
            Ok(())
        }
        Stmt::Continue { .. } => {
            out.push_str(&format!("{pad}continue;\n"));
            Ok(())
        }
        Stmt::ExprStmt { expr, .. } => {
            out.push_str(&format!("{pad}{};\n", emit_expr(expr, fn_defaults)?));
            Ok(())
        }
        Stmt::StyleDef { .. } => {
            out.push_str(&format!("{pad}// inline DOG style block (use `dog build --web` for browser output)\n"));
            Ok(())
        }
        Stmt::TryCatch { .. } => Err(
            "Native `--native` compilation does not yet lower `try ... catch` blocks; build a `.dgb` bundle (`dog build <file.dg>`) instead.".to_string(),
        ),
    }
}

fn emit_expr(
    expr: &Expr,
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<String, String> {
    match expr {
        Expr::StringLit { value, .. } => {
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            Ok(format!("\"{escaped}\".to_string()"))
        }
        Expr::InterpString { parts, .. } => {
            let mut body = String::from("{ let mut __s = String::new(); ");
            for p in parts {
                match p {
                    InterpPart::Literal(lit) => {
                        let escaped = lit
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\n', "\\n")
                            .replace('\r', "\\r")
                            .replace('\t', "\\t");
                        body.push_str(&format!("__s.push_str(\"{escaped}\"); "));
                    }
                    InterpPart::Expr(e) => {
                        body.push_str(&format!(
                            "__s.push_str(&({}).dog_print()); ",
                            emit_expr(e, fn_defaults)?
                        ));
                    }
                }
            }
            body.push_str("__s }");
            Ok(body)
        }
        Expr::NumberLit { value, .. } => Ok(format!("{value}i64")),
        Expr::DecimalLit { value, .. } => Ok(format!("{value:?}f64")),
        Expr::BoolLit { value, .. } => Ok(value.to_string()),
        Expr::Identifier { name, .. } => Ok(format!("{name}.clone()")),
        Expr::ListLit { elements, .. } => {
            let mut items = Vec::new();
            for e in elements {
                items.push(emit_expr(e, fn_defaults)?);
            }
            Ok(format!("vec![{}]", items.join(", ")))
        }
        Expr::MapLit { entries, .. } => {
            let mut body = String::from("{ let mut __m = BTreeMap::new(); ");
            for (k, v) in entries {
                body.push_str(&format!(
                    "__m.insert({}, {}); ",
                    emit_expr(k, fn_defaults)?,
                    emit_expr(v, fn_defaults)?
                ));
            }
            body.push_str("__m }");
            Ok(body)
        }
        Expr::StructInit { name, fields, .. } => {
            let mut parts = Vec::new();
            for (k, v) in fields {
                parts.push(format!("{k}: {}", emit_expr(v, fn_defaults)?));
            }
            Ok(format!("{name} {{ {} }}", parts.join(", ")))
        }
        Expr::FieldAccess { target, field, .. } => {
            if let Expr::Identifier { name, .. } = target.as_ref() {
                if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    return Ok(format!("{name}::{field}"));
                }
            }
            Ok(format!(
                "{}.{field}.clone()",
                emit_expr(target, fn_defaults)?
            ))
        }
        Expr::Index { target, index, .. } => Ok(format!(
            "{}[({}) as usize].clone()",
            emit_expr(target, fn_defaults)?,
            emit_expr(index, fn_defaults)?
        )),
        Expr::Unary { op, expr, .. } => match op {
            UnaryOp::Neg => Ok(format!("(-({}))", emit_expr(expr, fn_defaults)?)),
            UnaryOp::Not => Ok(format!("(!({}))", emit_expr(expr, fn_defaults)?)),
        },
        Expr::Binary {
            left, op, right, ..
        } => {
            let l = emit_expr(left, fn_defaults)?;
            let r = emit_expr(right, fn_defaults)?;
            match op {
                BinOp::Add => Ok(format!("({l} + &{r})")),
                BinOp::Sub => Ok(format!("({l} - {r})")),
                BinOp::Mul => Ok(format!("({l} * {r})")),
                BinOp::Div => Ok(format!("({l} / {r})")),
                BinOp::Mod => Ok(format!("({l} % {r})")),
                BinOp::Eq => Ok(format!("({l} == {r})")),
                BinOp::NotEq => Ok(format!("({l} != {r})")),
                BinOp::Gt => Ok(format!("({l} > {r})")),
                BinOp::Lt => Ok(format!("({l} < {r})")),
                BinOp::GtEq => Ok(format!("({l} >= {r})")),
                BinOp::LtEq => Ok(format!("({l} <= {r})")),
                BinOp::And => Ok(format!("({l} && {r})")),
                BinOp::Or => Ok(format!("({l} || {r})")),
            }
        }
        Expr::Call { callee, args, .. } => emit_call_expr(callee, args, fn_defaults),
        Expr::NamespaceCall {
            module, func, args, ..
        } => {
            let canonical = crate::stdlib::modules::resolve_std_call(module, func)?;
            emit_call_expr(canonical, args, fn_defaults)
        }
    }
}

fn emit_call_expr(
    callee: &str,
    args: &[Expr],
    fn_defaults: &HashMap<String, Vec<Option<Expr>>>,
) -> Result<String, String> {
    match callee {
        "say" | "print" => Ok(format!(
            "println!(\"{{}}\", ({}).dog_print())",
            emit_expr(&args[0], fn_defaults)?
        )),
        "answer" | "ask" => {
            if args.is_empty() {
                Ok("dog_answer(\"\")".to_string())
            } else {
                Ok(format!(
                    "dog_answer(&{})",
                    emit_expr(&args[0], fn_defaults)?
                ))
            }
        }
        "clear" => {
            if args.is_empty() {
                Ok("{ print!(\"\\x1b[2J\\x1b[H\"); let _ = io::stdout().flush(); }".to_string())
            } else if let Expr::Identifier { name, .. } = &args[0] {
                Ok(format!("{name}.clear()"))
            } else {
                Err("Expected variable in clear()".to_string())
            }
        }
        "lower" => Ok(format!(
            "({}).to_lowercase()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "upper" => Ok(format!(
            "({}).to_uppercase()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "trim" => Ok(format!(
            "({}).trim().to_string()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "contains" => Ok(format!(
            "({}).contains(&({}))",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "len" => Ok(format!(
            "(({}).len() as i64)",
            emit_expr(&args[0], fn_defaults)?
        )),
        "text" => Ok(format!("({}).dog_print()", emit_expr(&args[0], fn_defaults)?)),
        "number" => Ok(format!(
            "({}).dog_to_number()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "decimal" => Ok(format!(
            "({}).dog_to_decimal()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "abs" => Ok(format!("({}).abs()", emit_expr(&args[0], fn_defaults)?)),
        "min" => Ok(format!(
            "({}).min({})",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "max" => Ok(format!(
            "({}).max({})",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "assert" => {
            if args.len() == 2 {
                Ok(format!(
                    "assert!({}, \"{{}}\", {})",
                    emit_expr(&args[0], fn_defaults)?,
                    emit_expr(&args[1], fn_defaults)?
                ))
            } else {
                Ok(format!("assert!({})", emit_expr(&args[0], fn_defaults)?))
            }
        }
        "push" => {
            if let Expr::Identifier { name, .. } = &args[0] {
                Ok(format!(
                    "{name}.push({})",
                    emit_expr(&args[1], fn_defaults)?
                ))
            } else {
                Err("Expected variable in push()".to_string())
            }
        }
        "pop" => {
            if let Expr::Identifier { name, .. } = &args[0] {
                Ok(format!("{name}.pop().expect(\"pop() on empty list\")"))
            } else {
                Err("Expected variable in pop()".to_string())
            }
        }
        "replace" => Ok(format!(
            "({}).replace(&({}), &({}))",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?,
            emit_expr(&args[2], fn_defaults)?
        )),
        "starts_with" => Ok(format!(
            "({}).starts_with(&({}))",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "ends_with" => Ok(format!(
            "({}).ends_with(&({}))",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "round" => Ok(format!(
            "(({}) as f64).round() as i64",
            emit_expr(&args[0], fn_defaults)?
        )),
        "floor" => Ok(format!(
            "(({}) as f64).floor() as i64",
            emit_expr(&args[0], fn_defaults)?
        )),
        "ceil" => Ok(format!(
            "(({}) as f64).ceil() as i64",
            emit_expr(&args[0], fn_defaults)?
        )),
        "sqrt" => Ok(format!(
            "(({}) as f64).sqrt()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "sin" => Ok(format!(
            "(({}) as f64).sin()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "cos" => Ok(format!(
            "(({}) as f64).cos()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "log" => Ok(format!(
            "(({}) as f64).ln()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "str_repeat" => Ok(format!(
            "({}).repeat(({}).max(0) as usize)",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "clamp" => Ok(format!(
            "({}).clamp({}, {})",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?,
            emit_expr(&args[2], fn_defaults)?
        )),
        "pow" => Ok(format!(
            "(({}) as f64).powf(({}) as f64)",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "cwd" => Ok(
            "std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default()"
                .to_string(),
        ),
        "mkdir" => Ok(format!(
            "std::fs::create_dir_all(&({})).expect(\"mkdir failed\")",
            emit_expr(&args[0], fn_defaults)?
        )),
        "range" => Ok(format!(
            "(({})..({})).collect::<Vec<i64>>()",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "tan" => Ok(format!(
            "(({}) as f64).tan()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "atan2" => Ok(format!(
            "(({}) as f64).atan2(({}) as f64)",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "exp" => Ok(format!(
            "(({}) as f64).exp()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "log10" => Ok(format!(
            "(({}) as f64).log10()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "pow_int" => Ok(format!(
            "({}).saturating_pow(({}).max(0) as u32)",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "sign" => Ok(format!(
            "{{ let __v = ({}) as f64; if __v > 0.0 {{ 1i64 }} else if __v < 0.0 {{ -1i64 }} else {{ 0i64 }} }}",
            emit_expr(&args[0], fn_defaults)?
        )),
        "math_pi" => Ok("std::f64::consts::PI".to_string()),
        "math_e" => Ok("std::f64::consts::E".to_string()),
        "gcd" => Ok(format!(
            "{{ let (mut __a, mut __b) = (({}).abs(), ({}).abs()); while __b != 0 {{ let __t = __a % __b; __a = __b; __b = __t; }} __a }}",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "lcm" => Ok(format!(
            "{{ let (__x, __y) = (({}).abs(), ({}).abs()); if __x == 0 || __y == 0 {{ 0i64 }} else {{ let (mut __a, mut __b) = (__x, __y); while __b != 0 {{ let __t = __a % __b; __a = __b; __b = __t; }} (__x / __a).saturating_mul(__y) }} }}",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "reverse_text" => Ok(format!(
            "({}).chars().rev().collect::<String>()",
            emit_expr(&args[0], fn_defaults)?
        )),
        "count_matches" => Ok(format!(
            "{{ let (__h, __n) = ({}, {}); if __n.is_empty() {{ 0i64 }} else {{ __h.matches(&__n).count() as i64 }} }}",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "remove_prefix" => Ok(format!(
            "{{ let (__s, __p) = ({}, {}); __s.strip_prefix(&__p).map(|v| v.to_string()).unwrap_or(__s) }}",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "remove_suffix" => Ok(format!(
            "{{ let (__s, __p) = ({}, {}); __s.strip_suffix(&__p).map(|v| v.to_string()).unwrap_or(__s) }}",
            emit_expr(&args[0], fn_defaults)?,
            emit_expr(&args[1], fn_defaults)?
        )),
        "temp_dir" => Ok("std::env::temp_dir().display().to_string()".to_string()),
        "sort" => {
            if let Expr::Identifier { name, .. } = &args[0] {
                Ok(format!("{name}.sort()"))
            } else {
                Err("Expected variable in sort()".to_string())
            }
        }
        "reverse" => {
            if let Expr::Identifier { name, .. } = &args[0] {
                Ok(format!("{name}.reverse()"))
            } else {
                Err("Expected variable in reverse()".to_string())
            }
        }
        other => {
            let mut parts = Vec::new();
            for a in args {
                parts.push(emit_expr(a, fn_defaults)?);
            }
            if let Some(defaults) = fn_defaults.get(other) {
                for def_opt in defaults.iter().skip(args.len()) {
                    if let Some(def_expr) = def_opt {
                        parts.push(emit_expr(def_expr, fn_defaults)?);
                    }
                }
            }
            Ok(format!("{other}({})", parts.join(", ")))
        }
    }
}
