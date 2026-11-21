use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn stringify(&self) -> String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => b.to_string(),
            JsonValue::Int(n) => n.to_string(),
            JsonValue::Float(f) => {
                if f.is_finite() {
                    format!("{f}")
                } else {
                    "null".to_string()
                }
            }
            JsonValue::Str(s) => escape_json_str(s),
            JsonValue::Array(items) => {
                let parts: Vec<String> = items.iter().map(JsonValue::stringify).collect();
                format!("[{}]", parts.join(", "))
            }
            JsonValue::Object(map) => {
                let parts: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", escape_json_str(k), v.stringify()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
        }
    }
}

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0usize;
    let val = parse_val(&chars, &mut pos)?;
    skip_ws(&chars, &mut pos);
    if pos < chars.len() {
        return Err(format!("Unexpected trailing character '{}' at position {pos}.", chars[pos]));
    }
    Ok(val)
}

fn skip_ws(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

fn parse_val(chars: &[char], pos: &mut usize) -> Result<JsonValue, String> {
    skip_ws(chars, pos);
    let Some(&ch) = chars.get(*pos) else {
        return Err("Unexpected end of JSON input.".to_string());
    };
    match ch {
        'n' => {
            expect_lit(chars, pos, "null")?;
            Ok(JsonValue::Null)
        }
        't' => {
            expect_lit(chars, pos, "true")?;
            Ok(JsonValue::Bool(true))
        }
        'f' => {
            expect_lit(chars, pos, "false")?;
            Ok(JsonValue::Bool(false))
        }
        '"' => Ok(JsonValue::Str(parse_str(chars, pos)?)),
        '[' => parse_arr(chars, pos),
        '{' => parse_obj(chars, pos),
        '-' | '0'..='9' => parse_num(chars, pos),
        other => Err(format!("Invalid JSON token starting with '{other}'.")),
    }
}

fn expect_lit(chars: &[char], pos: &mut usize, lit: &str) -> Result<(), String> {
    for expected in lit.chars() {
        if chars.get(*pos) != Some(&expected) {
            return Err(format!("Expected '{lit}' in JSON."));
        }
        *pos += 1;
    }
    Ok(())
}

fn parse_str(chars: &[char], pos: &mut usize) -> Result<String, String> {
    *pos += 1; // opening "
    let mut out = String::new();
    while *pos < chars.len() {
        let ch = chars[*pos];
        *pos += 1;
        if ch == '"' {
            return Ok(out);
        }
        if ch == '\\' {
            let Some(&esc) = chars.get(*pos) else {
                return Err("Unterminated JSON string escape.".to_string());
            };
            *pos += 1;
            match esc {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000c}'),
                other => out.push(other),
            }
        } else {
            out.push(ch);
        }
    }
    Err("Unterminated JSON string.".to_string())
}

fn parse_num(chars: &[char], pos: &mut usize) -> Result<JsonValue, String> {
    let start = *pos;
    if chars.get(*pos) == Some(&'-') {
        *pos += 1;
    }
    let mut is_float = false;
    while let Some(&c) = chars.get(*pos) {
        if c.is_ascii_digit() {
            *pos += 1;
        } else if c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-' {
            if c == '-' && *pos == start {
                *pos += 1;
            } else {
                is_float = true;
                *pos += 1;
            }
        } else {
            break;
        }
    }
    let raw: String = chars[start..*pos].iter().collect();
    if is_float {
        let f = raw
            .parse::<f64>()
            .map_err(|_| format!("Invalid JSON float '{raw}'."))?;
        Ok(JsonValue::Float(f))
    } else {
        let i = raw
            .parse::<i64>()
            .map_err(|_| format!("Invalid JSON integer '{raw}'."))?;
        Ok(JsonValue::Int(i))
    }
}

fn parse_arr(chars: &[char], pos: &mut usize) -> Result<JsonValue, String> {
    *pos += 1; // [
    let mut items = Vec::new();
    skip_ws(chars, pos);
    if chars.get(*pos) == Some(&']') {
        *pos += 1;
        return Ok(JsonValue::Array(items));
    }
    loop {
        items.push(parse_val(chars, pos)?);
        skip_ws(chars, pos);
        match chars.get(*pos) {
            Some(',') => *pos += 1,
            Some(']') => {
                *pos += 1;
                return Ok(JsonValue::Array(items));
            }
            _ => return Err("Expected ',' or ']' in JSON array.".to_string()),
        }
    }
}

fn parse_obj(chars: &[char], pos: &mut usize) -> Result<JsonValue, String> {
    *pos += 1; // {
    let mut map = BTreeMap::new();
    skip_ws(chars, pos);
    if chars.get(*pos) == Some(&'}') {
        *pos += 1;
        return Ok(JsonValue::Object(map));
    }
    loop {
        skip_ws(chars, pos);
        if chars.get(*pos) != Some(&'"') {
            return Err("Expected string key in JSON object.".to_string());
        }
        let key = parse_str(chars, pos)?;
        skip_ws(chars, pos);
        if chars.get(*pos) != Some(&':') {
            return Err("Expected ':' after JSON object key.".to_string());
        }
        *pos += 1;
        let val = parse_val(chars, pos)?;
        map.insert(key, val);
        skip_ws(chars, pos);
        match chars.get(*pos) {
            Some(',') => *pos += 1,
            Some('}') => {
                *pos += 1;
                return Ok(JsonValue::Object(map));
            }
            _ => return Err("Expected ',' or '}' in JSON object.".to_string()),
        }
    }
}
