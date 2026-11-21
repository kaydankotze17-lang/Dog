use crate::ast::Span;
use crate::crypto::{
    base64_decode, base64_encode, hex_decode, hex_encode, hmac_sha256_hex, sha256_hex,
};
use crate::diagnostics::DogDiagnostic;
use crate::json::{parse_json, JsonValue};
use crate::runtime::Value;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
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
            http_get_plain(&url).map(Value::Text).map_err(|msg| {
                DogDiagnostic::new(
                    "E040",
                    format!("HTTP request failed for '{url}'."),
                    span.clone(),
                    msg,
                    "Ensure host is reachable over `http://` (TLS/HTTPS requires an external TLS crate).",
                    "Use `try ... catch err` around network calls.",
                    "Network barked back with an error.",
                )
            })
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
        _ => unreachable!("Type checker validates built-in function names"),
    }
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

fn http_get_plain(url: &str) -> Result<String, String> {
    if url.starts_with("https://") {
        return Err("https:// requires TLS; v0.1 std-only build supports plain http:// directly.".to_string());
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "URL must start with 'http://'".to_string())?;
    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, addr_str) = if host_port.contains(':') {
        (host_port.split(':').next().unwrap_or(host_port), host_port.to_string())
    } else {
        (host_port, format!("{host_port}:80"))
    };

    let mut addrs = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {e}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| "No resolved socket address".to_string())?;

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: dog-lang/0.1\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("Write failed: {e}"))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("Read failed: {e}"))?;
    let text = String::from_utf8_lossy(&raw);
    if let Some((_, body)) = text.split_once("\r\n\r\n") {
        Ok(body.to_string())
    } else {
        Ok(text.to_string())
    }
}
