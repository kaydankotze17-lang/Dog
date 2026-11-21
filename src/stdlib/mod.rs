use crate::ast::Span;
use crate::diagnostics::DogDiagnostic;
use crate::runtime::Value;
use std::io::{self, BufRead, Write};

pub trait InterpreterIo {
    fn write_stdout(&mut self, text: &str);
    fn writeln_stdout(&mut self, text: &str);
    fn read_line(&mut self) -> Result<String, String>;
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
            _ => unreachable!("Type checker validates lower() argument"),
        },
        "upper" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Text(s.to_uppercase())),
            _ => unreachable!("Type checker validates upper() argument"),
        },
        "trim" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Text(s.trim().to_string())),
            _ => unreachable!("Type checker validates trim() argument"),
        },
        "contains" => {
            let mut iter = args.into_iter();
            match (iter.next(), iter.next()) {
                (Some(Value::Text(hay)), Some(Value::Text(needle))) => {
                    Ok(Value::Bool(hay.contains(&needle)))
                }
                _ => unreachable!("Type checker validates contains() arguments"),
            }
        }
        "split" => {
            let mut iter = args.into_iter();
            match (iter.next(), iter.next()) {
                (Some(Value::Text(src)), Some(Value::Text(sep))) => {
                    let parts = if sep.is_empty() {
                        src.chars().map(|c| Value::Text(c.to_string())).collect()
                    } else {
                        src.split(&sep)
                            .map(|s| Value::Text(s.to_string()))
                            .collect()
                    };
                    Ok(Value::List(parts))
                }
                _ => unreachable!("Type checker validates split() arguments"),
            }
        }
        "join" => {
            let mut iter = args.into_iter();
            match (iter.next(), iter.next()) {
                (Some(Value::List(items)), Some(Value::Text(sep))) => {
                    let strings: Vec<String> = items
                        .into_iter()
                        .map(|v| match v {
                            Value::Text(s) => s,
                            other => other.to_string(),
                        })
                        .collect();
                    Ok(Value::Text(strings.join(&sep)))
                }
                _ => unreachable!("Type checker validates join() arguments"),
            }
        }
        "len" => match args.into_iter().next() {
            Some(Value::Text(s)) => Ok(Value::Number(s.chars().count() as i64)),
            Some(Value::List(items)) => Ok(Value::Number(items.len() as i64)),
            _ => unreachable!("Type checker validates len() argument"),
        },
        "text" => match args.into_iter().next() {
            Some(v) => Ok(Value::Text(v.to_string())),
            None => unreachable!("Type checker validates text() argument"),
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
                        "Validate user input before calling number().",
                        "That doesn't look like a number to me.",
                    )
                    .with_types("number", format!("text(\"{trimmed}\")"))
                })?;
                Ok(Value::Number(parsed))
            }
            _ => unreachable!("Type checker validates number() argument"),
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
                        "Avoid stray letters or units inside numeric strings.",
                        "Decimals need digits, not poetry.",
                    )
                    .with_types("decimal", format!("text(\"{trimmed}\")"))
                })?;
                Ok(Value::Decimal(parsed))
            }
            _ => unreachable!("Type checker validates decimal() argument"),
        },
        "abs" => match args.into_iter().next() {
            Some(Value::Number(n)) => Ok(Value::Number(n.saturating_abs())),
            Some(Value::Decimal(d)) => Ok(Value::Decimal(d.abs())),
            _ => unreachable!("Type checker validates abs() argument"),
        },
        "min" => {
            let mut iter = args.into_iter();
            match (iter.next(), iter.next()) {
                (Some(Value::Number(a)), Some(Value::Number(b))) => Ok(Value::Number(a.min(b))),
                (Some(Value::Decimal(a)), Some(Value::Decimal(b))) => Ok(Value::Decimal(a.min(b))),
                _ => unreachable!("Type checker validates min() arguments"),
            }
        }
        "max" => {
            let mut iter = args.into_iter();
            match (iter.next(), iter.next()) {
                (Some(Value::Number(a)), Some(Value::Number(b))) => Ok(Value::Number(a.max(b))),
                (Some(Value::Decimal(a)), Some(Value::Decimal(b))) => Ok(Value::Decimal(a.max(b))),
                _ => unreachable!("Type checker validates max() arguments"),
            }
        }
        "assert" => {
            let mut iter = args.into_iter();
            let cond = iter.next();
            let msg = iter.next();
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
        _ => unreachable!("Type checker validates built-in function names"),
    }
}
