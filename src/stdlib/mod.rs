use crate::ast::Span;
use crate::crypto::{
    base64_decode, base64_encode, hex_decode, hex_encode, hmac_sha256_hex, sha256_hex,
};
use crate::diagnostics::DogDiagnostic;
use crate::json::{parse_json, JsonValue};
use crate::runtime::Value;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static RNG_COUNTER: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

pub trait InterpreterIo {
    fn write_stdout(&mut self, text: &str);
    fn writeln_stdout(&mut self, text: &str);
    fn read_line(&mut self) -> Result<String, String>;
    fn script_args(&self) -> Vec<String> {
        Vec::new()
    }
}

pub struct StdIo;

impl InterpreterIo for StdIo {
    fn write_stdout(&mut self, text: &str) {
        print!("{text}");
        let _ = io::stdout().flush();
    }

    fn writeln_stdout(&mut self, text: &str) {
        println!("{text}");
    }

    fn read_line(&mut self) -> Result<String, String> {
        let mut buf = String::new();
        io::stdin()
            .lock()
            .read_line(&mut buf)
            .map_err(|e| e.to_string())?;
        Ok(buf.trim_end_matches(&['\r', '\n'][..]).to_string())
    }
}

pub struct StdIoWithArgs {
    pub args: Vec<String>,
}

impl InterpreterIo for StdIoWithArgs {
    fn write_stdout(&mut self, text: &str) {
        StdIo.write_stdout(text);
    }

    fn writeln_stdout(&mut self, text: &str) {
        StdIo.writeln_stdout(text);
    }

    fn read_line(&mut self) -> Result<String, String> {
        StdIo.read_line()
    }

    fn script_args(&self) -> Vec<String> {
        self.args.clone()
    }
}

pub fn call_builtin(
    name: &str,
    args: Vec<Value>,
    span: &Span,
    io_host: &mut dyn InterpreterIo,
) -> Result<Value, DogDiagnostic> {
    match name {
        "say" => {
            let val = args.into_iter().next().unwrap_or(Value::Void);
            io_host.writeln_stdout(&val.to_string());
            Ok(Value::Void)
        }
        "answer" => {
            if let Some(Value::Text(prompt)) = args.into_iter().next() {
                io_host.write_stdout(&prompt);
            }
            let line = io_host.read_line().map_err(|err| {
                DogDiagnostic::new(
                    "E040",
                    "Failed to read input via answer().",
                    span.clone(),
                    format!("Standard input returned an I/O error: {err}"),
                    "Ensure standard input is readable when running interactive scripts.",
                    "Pipe input or run inside an interactive terminal.",
                    "Nobody answered when I barked.",
                )
            })?;
            Ok(Value::Text(line))
        }
        "lower" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Text(s.to_lowercase())),
            _ => unreachable!(),
        },
        "upper" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Text(s.to_uppercase())),
            _ => unreachable!(),
        },
        "trim" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Text(s.trim().to_string())),
            _ => unreachable!(),
        },
        "contains" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Text(h)), Some(Value::Text(n))) => Ok(Value::Bool(h.contains(&n))),
                _ => unreachable!(),
            }
        }
        "split" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Text(s)), Some(Value::Text(sep))) => {
                    let parts = if sep.is_empty() {
                        s.chars().map(|c| Value::Text(c.to_string())).collect()
                    } else {
                        s.split(&sep).map(|p| Value::Text(p.to_string())).collect()
                    };
                    Ok(Value::List(parts))
                }
                _ => unreachable!(),
            }
        }
        "join" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::List(items)), Some(Value::Text(sep))) => {
                    let strs: Vec<String> = items.into_iter().map(|v| v.to_string()).collect();
                    Ok(Value::Text(strs.join(&sep)))
                }
                _ => unreachable!(),
            }
        }
        "len" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Number(s.chars().count() as i64)),
            Some(Value::List(items)) => Ok(Value::Number(items.len() as i64)),
            Some(Value::Map(entries)) => Ok(Value::Number(entries.len() as i64)),
            _ => unreachable!(),
        },
        "text" => match args.into_iter().next() {
            Some(v) => Ok(Value::Text(v.to_string())),
            None => unreachable!(),
        },
        "number" => match args.into_iter().next() {
            Some(Value::Number(n)) => Ok(Value::Number(n)),
            Some(Value::Decimal(d)) => Ok(Value::Number(d as i64)),
            Some(Value::Text(s)) => {
                let trimmed = s.trim();
                let parsed = trimmed.parse::<i64>().map_err(|_| {
                    DogDiagnostic::new(
                        "E030",
                        format!("Could not convert text \"{trimmed}\" to number."),
                        span.clone(),
                        format!("'{trimmed}' is not a valid whole integer."),
                        "Enter digits like \"42\", or use decimal() for fractional numbers.",
                        "Wrap untrusted parsing in `try ... catch err ... end`.",
                        "That doesn't look like a number to me.",
                    )
                    .with_types("number", format!("text(\"{trimmed}\")"))
                })?;
                Ok(Value::Number(parsed))
            }
            _ => unreachable!(),
        },
        "decimal" => match args.into_iter().next() {
            Some(Value::Decimal(d)) => Ok(Value::Decimal(d)),
            Some(Value::Number(n)) => Ok(Value::Decimal(n as f64)),
            Some(Value::Text(s)) => {
                let trimmed = s.trim();
                let parsed = trimmed.parse::<f64>().map_err(|_| {
                    DogDiagnostic::new(
                        "E030",
                        format!("Could not convert text \"{trimmed}\" to decimal."),
                        span.clone(),
                        format!("'{trimmed}' is not a valid decimal number."),
                        "Pass numeric text such as \"3.14\" or \"10\".",
                        "Wrap untrusted parsing in `try ... catch err ... end`.",
                        "Decimals need digits, not poetry.",
                    )
                    .with_types("decimal", format!("text(\"{trimmed}\")"))
                })?;
                Ok(Value::Decimal(parsed))
            }
            _ => unreachable!(),
        },
        "abs" => match args.into_iter().next() {
            Some(Value::Number(n)) => Ok(Value::Number(n.saturating_abs())),
            Some(Value::Decimal(d)) => Ok(Value::Decimal(d.abs())),
            _ => unreachable!(),
        },
        "min" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Number(a)), Some(Value::Number(b))) => Ok(Value::Number(a.min(b))),
                (Some(Value::Decimal(a)), Some(Value::Decimal(b))) => Ok(Value::Decimal(a.min(b))),
                _ => unreachable!(),
            }
        }
        "max" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Number(a)), Some(Value::Number(b))) => Ok(Value::Number(a.max(b))),
                (Some(Value::Decimal(a)), Some(Value::Decimal(b))) => Ok(Value::Decimal(a.max(b))),
                _ => unreachable!(),
            }
        }
        "assert" => {
            let mut it = args.into_iter();
            let cond = it.next();
            let msg = it.next();
            if matches!(cond, Some(Value::Bool(true))) {
                Ok(Value::Void)
            } else {
                let detail = match msg {
                    Some(Value::Text(m)) => m,
                    _ => "Assertion evaluated to false.".to_string(),
                };
                Err(DogDiagnostic::new(
                    "E033",
                    "Assertion failed.",
                    span.clone(),
                    detail,
                    "Inspect the asserted expression and expected value.",
                    "Use `dog test` to run test files cleanly.",
                    "That assertion bit back.",
                ))
            }
        }
        "fail" => {
            let msg = match args.into_iter().next() {
                Some(Value::Text(m)) => m,
                _ => "Explicit failure.".to_string(),
            };
            Err(DogDiagnostic::new(
                "E035",
                msg.clone(),
                span.clone(),
                msg,
                "Handle with `try ... catch err ... end` or resolve the failure condition.",
                "Use `fail(\"reason\")` for recoverable domain errors.",
                "Bailed out on purpose.",
            ))
        }
        "map_get" => {
            let mut it = args.into_iter();
            let Some(Value::Map(entries)) = it.next() else {
                unreachable!();
            };
            let Some(key) = it.next() else {
                unreachable!();
            };
            for (k, v) in entries {
                if k == key {
                    return Ok(v);
                }
            }
            Err(DogDiagnostic::new(
                "E034",
                format!("Key '{key}' not found in map."),
                span.clone(),
                "Attempted to read a missing key with `map_get()`.",
                "Check `if map_has(m, key)` first or wrap in `try ... catch`.",
                "Maps never return silent null values.",
                "Sniffed every key and came up empty.",
            ))
        }
        "map_has" => {
            let mut it = args.into_iter();
            let Some(Value::Map(entries)) = it.next() else {
                unreachable!();
            };
            let Some(key) = it.next() else {
                unreachable!();
            };
            Ok(Value::Bool(entries.iter().any(|(k, _)| k == &key)))
        }
        "map_keys" => match args.into_iter().next() {
            Some(Value::Map(entries)) => {
                Ok(Value::List(entries.into_iter().map(|(k, _)| k).collect()))
            }
            _ => unreachable!(),
        },
        "file_read" => {
            let Some(Value::Text(path)) = args.into_iter().next() else {
                unreachable!();
            };
            let content = fs::read_to_string(&path).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to read file '{path}'."),
                    span.clone(),
                    format!("Filesystem error: {e}"),
                    "Check that the file exists and is readable.",
                    "Use `file_exists(path)` or `try ... catch`.",
                    "File wouldn't open its door.",
                )
            })?;
            Ok(Value::Text(content))
        }
        "file_write" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(path)), Some(Value::Text(content))) = (it.next(), it.next()) else {
                unreachable!();
            };
            fs::write(&path, content).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to write file '{path}'."),
                    span.clone(),
                    format!("Filesystem error: {e}"),
                    "Ensure parent directory exists and is writable.",
                    "Check filesystem permissions.",
                    "Pen ran out of ink on disk.",
                )
            })?;
            Ok(Value::Void)
        }
        "file_exists" => {
            let Some(Value::Text(path)) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Bool(std::path::Path::new(&path).exists()))
        }
        "file_delete" => {
            let Some(Value::Text(path)) = args.into_iter().next() else {
                unreachable!();
            };
            let ok = fs::remove_file(&path).is_ok();
            Ok(Value::Bool(ok))
        }
        "dir_list" => {
            let Some(Value::Text(path)) = args.into_iter().next() else {
                unreachable!();
            };
            let entries = fs::read_dir(&path).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to list directory '{path}'."),
                    span.clone(),
                    format!("Filesystem error: {e}"),
                    "Check that the directory exists and is readable.",
                    "Use `.` for current directory.",
                    "Couldn't peek inside that folder.",
                )
            })?;
            let mut names = Vec::new();
            for entry in entries.flatten() {
                if let Some(n) = entry.file_name().to_str() {
                    names.push(Value::Text(n.to_string()));
                }
            }
            names.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
            Ok(Value::List(names))
        }
        "env_get" => {
            let Some(Value::Text(key)) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Text(std::env::var(&key).unwrap_or_default()))
        }
        "env_set" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(key)), Some(Value::Text(val))) = (it.next(), it.next()) else {
                unreachable!();
            };
            std::env::set_var(key, val);
            Ok(Value::Void)
        }
        "time_now_ms" => {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Ok(Value::Number(ms))
        }
        "sleep_ms" => {
            let Some(Value::Number(ms)) = args.into_iter().next() else {
                unreachable!();
            };
            if ms > 0 {
                std::thread::sleep(Duration::from_millis(ms as u64));
            }
            Ok(Value::Void)
        }
        "hex_encode" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Text(hex_encode(s.as_bytes())))
        }
        "hex_decode" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            let bytes = hex_decode(&s).map_err(|e| {
                DogDiagnostic::new(
                    "E030",
                    "Invalid hex input in hex_decode().",
                    span.clone(),
                    e,
                    "Pass valid hexadecimal pairs like \"646f67\".",
                    "Wrap decoding in `try ... catch` for untrusted input.",
                    "That's not valid hex.",
                )
            })?;
            let text = String::from_utf8(bytes).map_err(|_| {
                DogDiagnostic::new(
                    "E030",
                    "Decoded hex bytes are not valid UTF-8.",
                    span.clone(),
                    "DOG `text` values require valid UTF-8.",
                    "Check the encoded input.",
                    "Verify source encoding.",
                    "Looks like binary scrambled eggs.",
                )
            })?;
            Ok(Value::Text(text))
        }
        "base64_encode" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Text(base64_encode(s.as_bytes())))
        }
        "base64_decode" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            let bytes = base64_decode(&s).map_err(|e| {
                DogDiagnostic::new(
                    "E030",
                    "Invalid Base64 input in base64_decode().",
                    span.clone(),
                    e,
                    "Pass valid Base64 characters padded to a multiple of 4.",
                    "Wrap decoding in `try ... catch` for untrusted input.",
                    "Base64 doesn't look like that.",
                )
            })?;
            let text = String::from_utf8(bytes).map_err(|_| {
                DogDiagnostic::new(
                    "E030",
                    "Decoded Base64 bytes are not valid UTF-8.",
                    span.clone(),
                    "DOG `text` values require valid UTF-8.",
                    "Check input payload.",
                    "Verify source encoding.",
                    "Decoded bytes aren't UTF-8.",
                )
            })?;
            Ok(Value::Text(text))
        }
        "sha256" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Text(sha256_hex(s.as_bytes())))
        }
        "json_parse" => {
            let Some(Value::Text(raw)) = args.into_iter().next() else {
                unreachable!();
            };
            let parsed = parse_json(&raw).map_err(|e| {
                DogDiagnostic::new(
                    "E030",
                    "Invalid JSON in json_parse().",
                    span.clone(),
                    e,
                    "Pass a valid JSON object string like `{\"name\": \"dog\"}`.",
                    "Wrap parsing in `try ... catch err ... end`.",
                    "JSON parser tripped over a bracket.",
                )
            })?;
            let JsonValue::Object(obj) = parsed else {
                return Err(DogDiagnostic::new(
                    "E030",
                    "json_parse() expects a top-level JSON object.",
                    span.clone(),
                    "Top-level JSON value was not an object `{ ... }`.",
                    "Wrap values inside a JSON object.",
                    "Returns `map<text, text>`.",
                    "Expected `{...}` at the JSON root.",
                ));
            };
            let mut entries = Vec::with_capacity(obj.len());
            for (k, v) in obj {
                let s = match v {
                    JsonValue::Str(x) => x,
                    other => other.stringify(),
                };
                entries.push((Value::Text(k), Value::Text(s)));
            }
            Ok(Value::Map(entries))
        }
        "json_stringify" => {
            let Some(val) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Text(value_to_json(&val).stringify()))
        }
        "http_get" => {
            let Some(Value::Text(url)) = args.into_iter().next() else {
                unreachable!();
            };
            crate::net::http_client_request("GET", &url, &[], "")
                .map(|r| Value::Text(r.body))
                .map_err(|msg| net_err(format!("HTTP GET failed for '{url}'."), msg, span))
        }
        "http_post" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(url)), Some(Value::Text(body)), Some(Value::Text(ctype))) =
                (it.next(), it.next(), it.next())
            else {
                unreachable!();
            };
            let hdrs = vec![("Content-Type".to_string(), ctype)];
            crate::net::http_client_request("POST", &url, &hdrs, &body)
                .map(|r| Value::Text(r.body))
                .map_err(|msg| net_err(format!("HTTP POST failed for '{url}'."), msg, span))
        }
        "http_request" => {
            let mut it = args.into_iter();
            let (
                Some(Value::Text(method)),
                Some(Value::Text(url)),
                Some(Value::Map(hdr_entries)),
                Some(Value::Text(body)),
            ) = (it.next(), it.next(), it.next(), it.next())
            else {
                unreachable!();
            };
            let hdrs: Vec<(String, String)> = hdr_entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let resp = crate::net::http_client_request(&method, &url, &hdrs, &body)
                .map_err(|msg| net_err(format!("HTTP {method} failed for '{url}'."), msg, span))?;
            Ok(Value::Map(vec![
                (Value::Text("status".to_string()), Value::Text(resp.status.to_string())),
                (Value::Text("body".to_string()), Value::Text(resp.body)),
            ]))
        }
        "http_listen" => {
            let Some(Value::Text(addr)) = args.into_iter().next() else {
                unreachable!();
            };
            let id = crate::net::http_listen(&addr)
                .map_err(|msg| net_err(format!("Failed to bind HTTP server on '{addr}'."), msg, span))?;
            Ok(Value::Number(id))
        }
        "http_port" => {
            let Some(Value::Number(id)) = args.into_iter().next() else {
                unreachable!();
            };
            let port = crate::net::http_port(id)
                .map_err(|msg| net_err("Failed to query HTTP port.".to_string(), msg, span))?;
            Ok(Value::Number(port))
        }
        "http_accept" => {
            let Some(Value::Number(id)) = args.into_iter().next() else {
                unreachable!();
            };
            let pairs = crate::net::http_accept(id)
                .map_err(|msg| net_err("Failed accepting HTTP request.".to_string(), msg, span))?;
            Ok(Value::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| (Value::Text(k), Value::Text(v)))
                    .collect(),
            ))
        }
        "http_reply" => {
            let mut it = args.into_iter();
            let (
                Some(Value::Map(req_entries)),
                Some(Value::Number(status)),
                Some(Value::Text(ctype)),
                Some(Value::Text(body)),
            ) = (it.next(), it.next(), it.next(), it.next())
            else {
                unreachable!();
            };
            let req_id = extract_req_id(&req_entries, span)?;
            crate::net::http_reply(&req_id, status, &ctype, &body)
                .map_err(|msg| net_err("Failed sending HTTP reply.".to_string(), msg, span))?;
            Ok(Value::Void)
        }
        "http_serve_file" => {
            let mut it = args.into_iter();
            let (Some(Value::Map(req_entries)), Some(Value::Text(path))) = (it.next(), it.next())
            else {
                unreachable!();
            };
            let req_id = extract_req_id(&req_entries, span)?;
            crate::net::http_serve_file(&req_id, &path)
                .map_err(|msg| net_err(format!("Failed serving file '{path}'."), msg, span))?;
            Ok(Value::Void)
        }
        "http_close" => {
            let Some(Value::Number(id)) = args.into_iter().next() else {
                unreachable!();
            };
            crate::net::http_close(id)
                .map_err(|msg| net_err("Failed closing HTTP server.".to_string(), msg, span))?;
            Ok(Value::Void)
        }
        "tcp_listen" => {
            let Some(Value::Text(addr)) = args.into_iter().next() else {
                unreachable!();
            };
            let id = crate::net::tcp_listen(&addr)
                .map_err(|msg| net_err(format!("Failed to bind TCP listener on '{addr}'."), msg, span))?;
            Ok(Value::Number(id))
        }
        "tcp_port" => {
            let Some(Value::Number(id)) = args.into_iter().next() else {
                unreachable!();
            };
            let port = crate::net::tcp_port(id)
                .map_err(|msg| net_err("Failed querying TCP port.".to_string(), msg, span))?;
            Ok(Value::Number(port))
        }
        "tcp_accept" => {
            let Some(Value::Number(id)) = args.into_iter().next() else {
                unreachable!();
            };
            let conn = crate::net::tcp_accept(id)
                .map_err(|msg| net_err("Failed accepting TCP connection.".to_string(), msg, span))?;
            Ok(Value::Number(conn))
        }
        "tcp_connect" => {
            let Some(Value::Text(addr)) = args.into_iter().next() else {
                unreachable!();
            };
            let conn = crate::net::tcp_connect(&addr)
                .map_err(|msg| net_err(format!("TCP connect failed for '{addr}'."), msg, span))?;
            Ok(Value::Number(conn))
        }
        "tcp_send" => {
            let mut it = args.into_iter();
            let (Some(Value::Number(id)), Some(Value::Text(data))) = (it.next(), it.next()) else {
                unreachable!();
            };
            crate::net::tcp_send(id, &data)
                .map_err(|msg| net_err("TCP send failed.".to_string(), msg, span))?;
            Ok(Value::Void)
        }
        "tcp_recv" => {
            let mut it = args.into_iter();
            let (Some(Value::Number(id)), Some(Value::Number(max_bytes))) = (it.next(), it.next())
            else {
                unreachable!();
            };
            let data = crate::net::tcp_recv(id, max_bytes)
                .map_err(|msg| net_err("TCP recv failed.".to_string(), msg, span))?;
            Ok(Value::Text(data))
        }
        "tcp_close" => {
            let Some(Value::Number(id)) = args.into_iter().next() else {
                unreachable!();
            };
            crate::net::tcp_close(id)
                .map_err(|msg| net_err("Failed closing TCP handle.".to_string(), msg, span))?;
            Ok(Value::Void)
        }
        "log_info" => {
            let Some(Value::Text(msg)) = args.into_iter().next() else {
                unreachable!();
            };
            io_host.writeln_stdout(&format!("[INFO] {msg}"));
            Ok(Value::Void)
        }
        "log_warn" => {
            let Some(Value::Text(msg)) = args.into_iter().next() else {
                unreachable!();
            };
            io_host.writeln_stdout(&format!("[WARN] {msg}"));
            Ok(Value::Void)
        }
        "log_error" => {
            let Some(Value::Text(msg)) = args.into_iter().next() else {
                unreachable!();
            };
            io_host.writeln_stdout(&format!("[ERROR] {msg}"));
            Ok(Value::Void)
        }
        "replace" => {
            let mut it = args.into_iter();
            match (it.next(), it.next(), it.next()) {
                (Some(Value::Text(s)), Some(Value::Text(from)), Some(Value::Text(to))) => {
                    Ok(Value::Text(s.replace(&from, &to)))
                }
                _ => unreachable!(),
            }
        }
        "starts_with" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Text(s)), Some(Value::Text(prefix))) => {
                    Ok(Value::Bool(s.starts_with(&prefix)))
                }
                _ => unreachable!(),
            }
        }
        "ends_with" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Text(s)), Some(Value::Text(suffix))) => {
                    Ok(Value::Bool(s.ends_with(&suffix)))
                }
                _ => unreachable!(),
            }
        }
        "slice" => {
            let mut it = args.into_iter();
            match (it.next(), it.next(), it.next()) {
                (Some(Value::Text(s)), Some(Value::Number(start)), Some(Value::Number(end))) => {
                    let chars: Vec<char> = s.chars().collect();
                    let len = chars.len() as i64;
                    let s_idx = start.clamp(0, len) as usize;
                    let e_idx = end.clamp(0, len) as usize;
                    if s_idx >= e_idx {
                        Ok(Value::Text(String::new()))
                    } else {
                        let out: String = chars[s_idx..e_idx].iter().collect();
                        Ok(Value::Text(out))
                    }
                }
                _ => unreachable!(),
            }
        }
        "round" => match args.into_iter().next() {
            Some(Value::Decimal(d)) => Ok(Value::Number(d.round() as i64)),
            Some(Value::Number(n)) => Ok(Value::Number(n)),
            _ => unreachable!(),
        },
        "floor" => match args.into_iter().next() {
            Some(Value::Decimal(d)) => Ok(Value::Number(d.floor() as i64)),
            Some(Value::Number(n)) => Ok(Value::Number(n)),
            _ => unreachable!(),
        },
        "ceil" => match args.into_iter().next() {
            Some(Value::Decimal(d)) => Ok(Value::Number(d.ceil() as i64)),
            Some(Value::Number(n)) => Ok(Value::Number(n)),
            _ => unreachable!(),
        },
        "sqrt" => {
            let val = match args.into_iter().next() {
                Some(Value::Decimal(d)) => d,
                Some(Value::Number(n)) => n as f64,
                _ => unreachable!(),
            };
            if val < 0.0 {
                return Err(DogDiagnostic::new(
                    "E031",
                    "Cannot take sqrt() of a negative number.",
                    span.clone(),
                    format!("Received {val}, which is negative."),
                    "Pass a non-negative number >= 0.",
                    "Guard negative inputs with `if x >= 0`.",
                    "Imaginary numbers live in another kennel.",
                ));
            }
            Ok(Value::Decimal(val.sqrt()))
        }
        "pow" => {
            let mut it = args.into_iter();
            let base = match it.next() {
                Some(Value::Decimal(d)) => d,
                Some(Value::Number(n)) => n as f64,
                _ => unreachable!(),
            };
            let exp = match it.next() {
                Some(Value::Decimal(d)) => d,
                Some(Value::Number(n)) => n as f64,
                _ => unreachable!(),
            };
            Ok(Value::Decimal(base.powf(exp)))
        }
        "rand_int" => {
            let mut it = args.into_iter();
            let (Some(Value::Number(min_v)), Some(Value::Number(max_v))) = (it.next(), it.next())
            else {
                unreachable!();
            };
            if min_v > max_v {
                return Err(DogDiagnostic::new(
                    "E032",
                    "Invalid range in rand_int().",
                    span.clone(),
                    format!("min ({min_v}) cannot be greater than max ({max_v})."),
                    "Ensure min <= max.",
                    "Swap the bounds if needed.",
                    "Can't roll dice backwards.",
                ));
            }
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1);
            let ctr = RNG_COUNTER.fetch_add(1, Ordering::Relaxed);
            let mut x = nanos ^ ctr.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
            x ^= x >> 31;
            let span_u = (max_v as i128 - min_v as i128 + 1) as u64;
            let picked = min_v as i128 + (x % span_u) as i128;
            Ok(Value::Number(picked as i64))
        }
        "range" => {
            let mut it = args.into_iter();
            let (Some(Value::Number(start)), Some(Value::Number(end))) = (it.next(), it.next())
            else {
                unreachable!();
            };
            if end <= start {
                return Ok(Value::List(Vec::new()));
            }
            let count = (end as i128 - start as i128) as usize;
            if count > 1_000_000 {
                return Err(DogDiagnostic::new(
                    "E032",
                    "range() exceeds maximum size (1,000,000).",
                    span.clone(),
                    format!("Requested {count} elements in range({start}, {end})."),
                    "Use a `while` loop for huge numeric ranges.",
                    "range() materializes a list<number>.",
                    "Even a golden retriever can't fetch a million sticks at once.",
                ));
            }
            let items = (start..end).map(Value::Number).collect();
            Ok(Value::List(items))
        }
        "list_contains" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::List(items)), Some(target)) => {
                    Ok(Value::Bool(items.iter().any(|v| v == &target)))
                }
                _ => unreachable!(),
            }
        }
        "file_append" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(path)), Some(Value::Text(content))) = (it.next(), it.next())
            else {
                unreachable!();
            };
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| {
                    DogDiagnostic::new(
                        "E040",
                        format!("Failed to append to file '{path}'."),
                        span.clone(),
                        format!("Filesystem error: {e}"),
                        "Ensure parent directory exists and file is writable.",
                        "Use `mkdir()` first if needed.",
                        "Couldn't append to that file.",
                    )
                })?;
            file.write_all(content.as_bytes()).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to write appended bytes to '{path}'."),
                    span.clone(),
                    format!("Filesystem error: {e}"),
                    "Check disk space and permissions.",
                    "Verify path.",
                    "Disk write stalled.",
                )
            })?;
            Ok(Value::Void)
        }
        "mkdir" => {
            let Some(Value::Text(path)) = args.into_iter().next() else {
                unreachable!();
            };
            fs::create_dir_all(&path).map_err(|e| {
                DogDiagnostic::new(
                    "E040",
                    format!("Failed to create directory '{path}'."),
                    span.clone(),
                    format!("Filesystem error: {e}"),
                    "Check parent path and permissions.",
                    "mkdir() creates nested directories automatically.",
                    "Could not dig that folder.",
                )
            })?;
            Ok(Value::Void)
        }
        "cwd" => {
            let dir = std::env::current_dir()
                .map(|p| p.display().to_string())
                .map_err(|e| {
                    DogDiagnostic::new(
                        "E040",
                        "Failed to read current working directory.",
                        span.clone(),
                        format!("OS error: {e}"),
                        "Check current directory permissions.",
                        "Ensure current directory has not been deleted.",
                        "Lost track of where we're standing.",
                    )
                })?;
            Ok(Value::Text(dir))
        }
        "args" => {
            let items = io_host
                .script_args()
                .into_iter()
                .map(Value::Text)
                .collect();
            Ok(Value::List(items))
        }
        "hmac_sha256" => {
            let mut it = args.into_iter();
            match (it.next(), it.next()) {
                (Some(Value::Text(key)), Some(Value::Text(msg))) => {
                    Ok(Value::Text(hmac_sha256_hex(key.as_bytes(), msg.as_bytes())))
                }
                _ => unreachable!(),
            }
        }
        "url_encode" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            Ok(Value::Text(crate::net::url_encode(&s)))
        }
        "url_decode" => {
            let Some(Value::Text(s)) = args.into_iter().next() else {
                unreachable!();
            };
            let decoded = crate::net::url_decode(&s).map_err(|e| {
                DogDiagnostic::new(
                    "E030",
                    "Invalid URL-encoded input in url_decode().",
                    span.clone(),
                    e,
                    "Pass valid percent-encoded text like \"hello%20dog\".",
                    "Use url_encode() to escape query parameters.",
                    "Those percent escapes look mangled.",
                )
            })?;
            Ok(Value::Text(decoded))
        }
        "csv_parse" => {
            let Some(Value::Text(raw)) = args.into_iter().next() else {
                unreachable!();
            };
            let rows = crate::net::csv_parse(&raw)
                .into_iter()
                .map(|r| Value::List(r.into_iter().map(Value::Text).collect()))
                .collect();
            Ok(Value::List(rows))
        }
        "kv_set" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(db)), Some(Value::Text(key)), Some(Value::Text(val))) =
                (it.next(), it.next(), it.next())
            else {
                unreachable!();
            };
            crate::net::kv_set(&db, &key, &val)
                .map_err(|msg| net_err(format!("KV write failed for '{db}'."), msg, span))?;
            Ok(Value::Void)
        }
        "kv_get" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(db)), Some(Value::Text(key))) = (it.next(), it.next()) else {
                unreachable!();
            };
            let found = crate::net::kv_get(&db, &key)
                .map_err(|msg| net_err(format!("KV read failed for '{db}'."), msg, span))?;
            match found {
                Some(v) => Ok(Value::Text(v)),
                None => Err(DogDiagnostic::new(
                    "E034",
                    format!("Key '{key}' not found in KV store '{db}'."),
                    span.clone(),
                    "Attempted to read a missing key with `kv_get()`.",
                    "Check `if kv_has(db, key)` first or wrap in `try ... catch`.",
                    "KV stores never return silent nulls.",
                    "Nothing buried under that key.",
                )),
            }
        }
        "kv_has" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(db)), Some(Value::Text(key))) = (it.next(), it.next()) else {
                unreachable!();
            };
            let has = crate::net::kv_has(&db, &key)
                .map_err(|msg| net_err(format!("KV lookup failed for '{db}'."), msg, span))?;
            Ok(Value::Bool(has))
        }
        "kv_delete" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(db)), Some(Value::Text(key))) = (it.next(), it.next()) else {
                unreachable!();
            };
            let removed = crate::net::kv_delete(&db, &key)
                .map_err(|msg| net_err(format!("KV delete failed for '{db}'."), msg, span))?;
            Ok(Value::Bool(removed))
        }
        "kv_keys" => {
            let Some(Value::Text(db)) = args.into_iter().next() else {
                unreachable!();
            };
            let keys = crate::net::kv_keys(&db)
                .map_err(|msg| net_err(format!("KV key listing failed for '{db}'."), msg, span))?;
            Ok(Value::List(keys.into_iter().map(Value::Text).collect()))
        }
        "exec_cmd" => {
            let Some(Value::Text(cmd)) = args.into_iter().next() else {
                unreachable!();
            };
            let (code, stdout, stderr) = crate::net::exec_shell_cmd(&cmd)
                .map_err(|msg| net_err(format!("Failed running shell command '{cmd}'."), msg, span))?;
            Ok(Value::Map(vec![
                (Value::Text("code".to_string()), Value::Text(code.to_string())),
                (Value::Text("stdout".to_string()), Value::Text(stdout)),
                (Value::Text("stderr".to_string()), Value::Text(stderr)),
            ]))
        }
        "os_name" => Ok(Value::Text(std::env::consts::OS.to_string())),
        "os_arch" => Ok(Value::Text(std::env::consts::ARCH.to_string())),
        "uuid_v4" => {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1);
            let ctr = RNG_COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id() as u64;
            let seed_a = nanos ^ (pid << 32) ^ ctr.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let seed_b = ctr.wrapping_add(nanos.rotate_left(17));
            Ok(Value::Text(crate::net::format_uuid_v4(seed_a, seed_b)))
        }
        "cli_info" | "cli_success" | "cli_warn" | "cli_error" | "cli_debug" => {
            let Some(Value::Text(msg)) = args.into_iter().next() else {
                unreachable!();
            };
            let messenger = crate::term::session_messenger();
            let level = match name {
                "cli_info" => crate::term::MsgLevel::Info,
                "cli_success" => crate::term::MsgLevel::Success,
                "cli_warn" => crate::term::MsgLevel::Warning,
                "cli_error" => crate::term::MsgLevel::Error,
                _ => crate::term::MsgLevel::Debug,
            };
            if level != crate::term::MsgLevel::Debug || messenger.style.debug {
                io_host.writeln_stdout(&messenger.render_line(level, &msg, None));
            }
            Ok(Value::Void)
        }
        "cli_progress" => {
            let mut it = args.into_iter();
            let (Some(Value::Text(label)), Some(Value::Number(cur)), Some(Value::Number(tot))) =
                (it.next(), it.next(), it.next())
            else {
                unreachable!();
            };
            let messenger = crate::term::session_messenger();
            let mut bar = crate::term::ProgressBar::new(
                label,
                tot.max(0) as u64,
                crate::term::ProgressUnit::Items,
                messenger,
            );
            bar.set(cur.max(0) as u64);
            io_host.writeln_stdout(&bar.render());
            Ok(Value::Void)
        }
        "cli_spinner" => {
            let mut it = args.into_iter();
            let (Some(Value::Number(step)), Some(Value::Text(label))) = (it.next(), it.next())
            else {
                unreachable!();
            };
            let messenger = crate::term::session_messenger();
            let spinner = crate::term::Spinner::new(label, messenger);
            io_host.writeln_stdout(&spinner.render_step(step.max(0) as usize, ""));
            Ok(Value::Void)
        }
        "cli_theme" => {
            let Some(Value::Text(theme_name)) = args.into_iter().next() else {
                unreachable!();
            };
            crate::term::set_session_theme(&theme_name).map_err(|msg| {
                DogDiagnostic::new(
                    "E030",
                    format!("Unknown CLI theme '{theme_name}'."),
                    span.clone(),
                    msg,
                    "Pass one of: \"default\", \"kali\", \"dracula\", \"solar\", or \"mono\".",
                    "Use `dog config theme <name>` to set a persistent default theme.",
                    "My fur clashes with that palette.",
                )
            })?;
            Ok(Value::Void)
        }
        "cli_prefix" => {
            let Some(Value::Text(prefix)) = args.into_iter().next() else {
                unreachable!();
            };
            crate::term::set_session_prefix(&prefix);
            Ok(Value::Void)
        }
        "cli_timestamps" => {
            let Some(Value::Bool(enabled)) = args.into_iter().next() else {
                unreachable!();
            };
            crate::term::set_session_timestamps(enabled);
            Ok(Value::Void)
        }
        "cli_supports_color" => Ok(Value::Bool(crate::term::session_messenger().use_color())),
        "cli_is_tty" => Ok(Value::Bool(crate::term::session_messenger().caps.is_tty)),
        _ => unreachable!("Type checker validates built-in function names"),
    }
}

fn net_err(title: String, msg: String, span: &Span) -> DogDiagnostic {
    DogDiagnostic::new(
        "E040",
        title,
        span.clone(),
        msg,
        "Check target address/port, permissions, or network connectivity.",
        "Wrap network or external command calls in `try ... catch err ... end`.",
        "Network barked back with an error.",
    )
}

fn extract_req_id(entries: &[(Value, Value)], span: &Span) -> Result<String, DogDiagnostic> {
    for (k, v) in entries {
        if matches!(k, Value::Text(s) if s == "id") {
            if let Value::Text(id) = v {
                return Ok(id.clone());
            }
        }
    }
    Err(DogDiagnostic::new(
        "E040",
        "Missing HTTP request 'id' in request map.",
        span.clone(),
        "Expected the map<text, text> returned by `http_accept(server)`.",
        "Pass the exact request map returned by `http_accept()`.",
        "Each accepted HTTP request carries a unique internal 'id'.",
        "Which connection am I replying to?",
    ))
}

fn value_to_json(val: &Value) -> JsonValue {
    match val {
        Value::Text(s) => JsonValue::Str(s.clone()),
        Value::Number(n) => JsonValue::Int(*n),
        Value::Decimal(d) => JsonValue::Float(*d),
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::List(items) => JsonValue::Array(items.iter().map(value_to_json).collect()),
        Value::Map(entries) => {
            let mut map = BTreeMap::new();
            for (k, v) in entries {
                map.insert(k.to_string(), value_to_json(v));
            }
            JsonValue::Object(map)
        }
        Value::Struct { fields, .. } => {
            let mut map = BTreeMap::new();
            for (k, v) in fields {
                map.insert(k.clone(), value_to_json(v));
            }
            JsonValue::Object(map)
        }
        Value::Enum { type_name, variant } => JsonValue::Str(format!("{type_name}.{variant}")),
        Value::Void => JsonValue::Null,
    }
}
