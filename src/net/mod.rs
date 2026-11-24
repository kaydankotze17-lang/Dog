use crate::crypto::{base64_encode, hex_encode, sha1_bytes};
use crate::json::{parse_json, JsonValue};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

fn alloc_id() -> i64 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

struct PendingHttpReq {
    stream: TcpStream,
    headers: BTreeMap<String, String>,
}

struct WsConn {
    stream: TcpStream,
    is_client: bool,
    leftover: Vec<u8>,
}

#[derive(Default)]
struct NetRegistry {
    http_servers: HashMap<i64, Arc<TcpListener>>,
    pending_http_reqs: HashMap<String, PendingHttpReq>,
    tcp_listeners: HashMap<i64, Arc<TcpListener>>,
    tcp_conns: HashMap<i64, Arc<Mutex<TcpStream>>>,
    ws_conns: HashMap<i64, Arc<Mutex<WsConn>>>,
    udp_sockets: HashMap<i64, Arc<UdpSocket>>,
}

static NET_STATE: Mutex<Option<NetRegistry>> = Mutex::new(None);

fn with_registry<T>(f: impl FnOnce(&mut NetRegistry) -> T) -> T {
    let mut guard = NET_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let reg = guard.get_or_insert_with(NetRegistry::default);
    f(reg)
}

pub fn http_listen(addr: &str) -> Result<i64, String> {
    let listener = TcpListener::bind(addr)
        .map_err(|e| format!("Failed to bind HTTP server on '{addr}': {e}"))?;
    let id = alloc_id();
    with_registry(|reg| {
        reg.http_servers.insert(id, Arc::new(listener));
    });
    Ok(id)
}

pub fn http_port(server_id: i64) -> Result<i64, String> {
    let listener = with_registry(|reg| reg.http_servers.get(&server_id).cloned())
        .ok_or_else(|| format!("Invalid HTTP server handle {server_id}."))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to query local port: {e}"))?
        .port();
    Ok(port as i64)
}

pub fn http_close(server_id: i64) -> Result<(), String> {
    let existed = with_registry(|reg| reg.http_servers.remove(&server_id).is_some());
    if existed {
        Ok(())
    } else {
        Err(format!("Invalid HTTP server handle {server_id}."))
    }
}

pub fn http_accept(server_id: i64) -> Result<Vec<(String, String)>, String> {
    let listener = with_registry(|reg| reg.http_servers.get(&server_id).cloned())
        .ok_or_else(|| format!("Invalid HTTP server handle {server_id}."))?;

    let (stream, peer) = listener
        .accept()
        .map_err(|e| format!("HTTP accept failed: {e}"))?;
    accept_http_stream(stream, peer.to_string())
}

pub fn http_try_accept(server_id: i64, timeout_ms: i64) -> Result<Vec<(String, String)>, String> {
    let listener = with_registry(|reg| reg.http_servers.get(&server_id).cloned())
        .ok_or_else(|| format!("Invalid HTTP server handle {server_id}."))?;

    let timeout = Duration::from_millis(timeout_ms.clamp(0, 60_000) as u64);
    let deadline = Instant::now() + timeout;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed setting nonblocking accept mode: {e}"))?;

    let accepted = loop {
        match listener.accept() {
            Ok((stream, peer)) => {
                let _ = listener.set_nonblocking(false);
                let _ = stream.set_nonblocking(false);
                break Some((stream, peer));
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    let _ = listener.set_nonblocking(false);
                    break None;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(err) => {
                let _ = listener.set_nonblocking(false);
                return Err(format!("HTTP accept failed: {err}"));
            }
        }
    };

    match accepted {
        None => Ok(vec![("_ok".to_string(), "false".to_string())]),
        Some((stream, peer)) => {
            let mut fields = accept_http_stream(stream, peer.to_string())?;
            fields.insert(0, ("_ok".to_string(), "true".to_string()));
            Ok(fields)
        }
    }
}

fn accept_http_stream(
    mut stream: TcpStream,
    peer_str: String,
) -> Result<Vec<(String, String)>, String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let (method, path, query, headers, body) = read_http_request(&mut stream)?;

    let req_id = format!("req_{}", alloc_id());
    let content_type = headers.get("content-type").cloned().unwrap_or_default();
    let authorization = headers.get("authorization").cloned().unwrap_or_default();
    let mut header_json_map = BTreeMap::new();
    for (k, v) in &headers {
        header_json_map.insert(k.clone(), JsonValue::Str(v.clone()));
    }
    let headers_json = JsonValue::Object(header_json_map).stringify();

    with_registry(|reg| {
        reg.pending_http_reqs
            .insert(req_id.clone(), PendingHttpReq { stream, headers });
    });

    Ok(vec![
        ("id".to_string(), req_id),
        ("method".to_string(), method),
        ("path".to_string(), path),
        ("query".to_string(), query),
        ("body".to_string(), body),
        ("remote".to_string(), peer_str),
        ("content_type".to_string(), content_type),
        ("authorization".to_string(), authorization),
        ("headers".to_string(), headers_json),
    ])
}

fn read_http_request(
    stream: &mut TcpStream,
) -> Result<(String, String, String, BTreeMap<String, String>, String), String> {
    let mut raw = Vec::with_capacity(2048);
    let mut buf = [0u8; 2048];
    let header_end;
    loop {
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("Failed to read HTTP request: {e}"))?;
        if n == 0 {
            return Err("Client closed connection before sending HTTP headers.".to_string());
        }
        raw.extend_from_slice(&buf[..n]);
        if let Some(pos) = find_double_crlf(&raw) {
            header_end = pos;
            break;
        }
        if raw.len() > 65_536 {
            return Err("HTTP request headers exceeded 64KB limit.".to_string());
        }
    }

    let head_bytes = &raw[..header_end];
    let head_str = String::from_utf8_lossy(head_bytes);
    let mut lines = head_str.split("\r\n");
    let req_line = lines.next().unwrap_or("");
    let mut req_parts = req_line.split_whitespace();
    let method = req_parts.next().unwrap_or("GET").to_ascii_uppercase();
    let target = req_parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };

    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let is_chunked = headers
        .get("transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));

    let body_start = header_end + 4;
    let body_bytes = if is_chunked {
        read_chunked_from_stream(&raw[body_start..], stream)?
    } else {
        let content_length: usize = headers
            .get("content-length")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
            .min(8 * 1024 * 1024);

        let mut acc = raw[body_start..].to_vec();
        while acc.len() < content_length {
            let remaining = content_length - acc.len();
            let take = remaining.min(buf.len());
            let n = stream
                .read(&mut buf[..take])
                .map_err(|e| format!("Failed reading HTTP request body: {e}"))?;
            if n == 0 {
                break;
            }
            acc.extend_from_slice(&buf[..n]);
        }
        if acc.len() > content_length {
            acc.truncate(content_length);
        }
        acc
    };

    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    Ok((method, path, query, headers, body))
}

fn read_chunked_from_stream(initial: &[u8], stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    const MAX_CHUNKED_BODY: usize = 8 * 1024 * 1024;
    let mut buf_data = initial.to_vec();
    let mut pos = 0usize;
    let mut out = Vec::new();
    let mut scratch = [0u8; 2048];

    loop {
        let crlf_rel = loop {
            if let Some(rel) = buf_data[pos..].windows(2).position(|w| w == b"\r\n") {
                break rel;
            }
            if buf_data.len() > MAX_CHUNKED_BODY + 65_536 {
                return Err("Chunked HTTP body exceeded 8MB limit.".to_string());
            }
            let n = stream
                .read(&mut scratch)
                .map_err(|e| format!("Failed reading chunked HTTP body: {e}"))?;
            if n == 0 {
                return Ok(out);
            }
            buf_data.extend_from_slice(&scratch[..n]);
        };

        let size_line = String::from_utf8_lossy(&buf_data[pos..pos + crlf_rel]);
        let hex_len = size_line.trim().split(';').next().unwrap_or("0").trim();
        let chunk_size = usize::from_str_radix(hex_len, 16)
            .map_err(|_| format!("Invalid HTTP chunk size '{hex_len}'."))?;
        pos += crlf_rel + 2;

        if chunk_size == 0 {
            break;
        }
        if out.len() + chunk_size > MAX_CHUNKED_BODY {
            return Err("Chunked HTTP body exceeded 8MB limit.".to_string());
        }

        while buf_data.len() < pos + chunk_size + 2 {
            let n = stream
                .read(&mut scratch)
                .map_err(|e| format!("Failed reading chunk payload: {e}"))?;
            if n == 0 {
                break;
            }
            buf_data.extend_from_slice(&scratch[..n]);
        }

        let avail_end = (pos + chunk_size).min(buf_data.len());
        out.extend_from_slice(&buf_data[pos..avail_end]);
        pos = (pos + chunk_size + 2).min(buf_data.len());
    }

    Ok(out)
}

fn find_double_crlf(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn http_reply(req_id: &str, status: i64, content_type: &str, body: &str) -> Result<(), String> {
    http_reply_bytes_with_headers(req_id, status, content_type, &[], body.as_bytes())
}

pub fn http_reply_headers(
    req_id: &str,
    status: i64,
    extra_headers: &[(String, String)],
    body: &str,
) -> Result<(), String> {
    let mut content_type = "text/plain; charset=utf-8".to_string();
    let mut filtered = Vec::new();
    for (k, v) in extra_headers {
        if k.eq_ignore_ascii_case("content-type") {
            content_type = v.clone();
        } else {
            filtered.push((k.clone(), v.clone()));
        }
    }
    http_reply_bytes_with_headers(req_id, status, &content_type, &filtered, body.as_bytes())
}

pub fn http_redirect(req_id: &str, status: i64, location: &str) -> Result<(), String> {
    let clean_loc = location.replace(['\r', '\n'], "");
    let code = if (300..=308).contains(&status) {
        status
    } else {
        302
    };
    let headers = vec![("Location".to_string(), clean_loc)];
    http_reply_bytes_with_headers(req_id, code, "text/plain; charset=utf-8", &headers, b"")
}

pub fn http_serve_file(req_id: &str, file_path: &str) -> Result<(), String> {
    let p = Path::new(file_path);
    if !p.is_file() {
        return http_reply_bytes_with_headers(
            req_id,
            404,
            "text/plain; charset=utf-8",
            &[],
            b"404 Not Found\n",
        );
    }
    let data = fs::read(p).map_err(|e| format!("Failed to read '{file_path}': {e}"))?;
    let ctype = guess_mime_type(p);
    http_reply_bytes_with_headers(req_id, 200, ctype, &[], &data)
}

fn http_reply_bytes_with_headers(
    req_id: &str,
    status: i64,
    content_type: &str,
    extra_headers: &[(String, String)],
    body: &[u8],
) -> Result<(), String> {
    let mut pending = with_registry(|reg| reg.pending_http_reqs.remove(req_id))
        .ok_or_else(|| format!("Invalid or already-replied HTTP request id '{req_id}'."))?;
    let reason = status_reason(status);
    let mut header = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n",
        body.len()
    );
    let mut has_cors = false;
    let mut has_conn = false;
    for (k, v) in extra_headers {
        let clean_k = k.replace(['\r', '\n', ':'], "");
        let clean_v = v.replace(['\r', '\n'], "");
        if clean_k.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if clean_k.eq_ignore_ascii_case("access-control-allow-origin") {
            has_cors = true;
        }
        if clean_k.eq_ignore_ascii_case("connection") {
            has_conn = true;
        }
        header.push_str(&format!("{clean_k}: {clean_v}\r\n"));
    }
    if !has_cors {
        header.push_str("Access-Control-Allow-Origin: *\r\n");
    }
    if !has_conn {
        header.push_str("Connection: close\r\n");
    }
    header.push_str("\r\n");

    pending
        .stream
        .write_all(header.as_bytes())
        .map_err(|e| format!("Failed writing HTTP response header: {e}"))?;
    pending
        .stream
        .write_all(body)
        .map_err(|e| format!("Failed writing HTTP response body: {e}"))?;
    let _ = pending.stream.flush();
    Ok(())
}

fn status_reason(code: i64) -> &'static str {
    http_status_text(code)
}

pub fn http_status_text(code: i64) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        418 => "I'm a teapot",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

pub fn guess_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "dg" | "md" | "txt" | "rs" | "toml" | "csv" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

pub fn url_parse(raw_url: &str) -> Result<Vec<(String, String)>, String> {
    let (scheme, rest) = raw_url
        .split_once("://")
        .ok_or_else(|| format!("URL '{raw_url}' must contain '://' scheme separator."))?;
    let scheme_lower = scheme.to_ascii_lowercase();
    let default_port = match scheme_lower.as_str() {
        "http" | "ws" => "80",
        "https" | "wss" => "443",
        _ => "",
    };

    let (without_frag, fragment) = match rest.split_once('#') {
        Some((before, frag)) => (before, frag.to_string()),
        None => (rest, String::new()),
    };

    let authority_end = without_frag
        .find(['/', '?'])
        .unwrap_or(without_frag.len());
    let authority = &without_frag[..authority_end];
    let path_and_query = &without_frag[authority_end..];

    let (path, query) = match path_and_query.split_once('?') {
        Some((p, q)) => (
            if p.is_empty() { "/".to_string() } else { p.to_string() },
            q.to_string(),
        ),
        None => (
            if path_and_query.is_empty() {
                "/".to_string()
            } else {
                path_and_query.to_string()
            },
            String::new(),
        ),
    };

    let (host, port) = split_host_port(authority, default_port)?;

    Ok(vec![
        ("scheme".to_string(), scheme_lower),
        ("host".to_string(), host),
        ("port".to_string(), port),
        ("path".to_string(), path),
        ("query".to_string(), query),
        ("fragment".to_string(), fragment),
        ("authority".to_string(), authority.to_string()),
    ])
}

fn split_host_port(authority: &str, default_port: &str) -> Result<(String, String), String> {
    if authority.is_empty() {
        return Err("URL is missing host.".to_string());
    }
    if let Some(after_bracket) = authority.strip_prefix('[') {
        let close = after_bracket
            .find(']')
            .ok_or_else(|| format!("Unclosed IPv6 host literal in '{authority}'."))?;
        let host = format!("[{}]", &after_bracket[..close]);
        let remainder = &after_bracket[close + 1..];
        let port = if let Some(p) = remainder.strip_prefix(':') {
            if p.is_empty() {
                default_port.to_string()
            } else {
                p.to_string()
            }
        } else {
            default_port.to_string()
        };
        Ok((host, port))
    } else if let Some((h, p)) = authority.rsplit_once(':') {
        Ok((
            h.to_string(),
            if p.is_empty() {
                default_port.to_string()
            } else {
                p.to_string()
            },
        ))
    } else {
        Ok((authority.to_string(), default_port.to_string()))
    }
}

pub fn query_parse(raw_query: &str) -> Result<Vec<(String, String)>, String> {
    let trimmed = raw_query.strip_prefix('?').unwrap_or(raw_query);
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for pair in trimmed.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_k, raw_v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        out.push((url_decode(raw_k)?, url_decode(raw_v)?));
    }
    Ok(out)
}

pub fn query_stringify(params: &[(String, String)]) -> String {
    let mut parts = Vec::with_capacity(params.len());
    for (k, v) in params {
        parts.push(format!("{}={}", url_encode(k), url_encode(v)));
    }
    parts.join("&")
}

pub fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn http_route_match(pattern: &str, path: &str) -> Vec<(String, String)> {
    let pat_segs: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut params = Vec::new();

    let mut i = 0usize;
    while i < pat_segs.len() {
        let pseg = pat_segs[i];
        if let Some(wild_name) = pseg.strip_prefix('*') {
            let key = if wild_name.is_empty() { "rest" } else { wild_name };
            let rest = path_segs[i..].join("/");
            params.push((key.to_string(), rest));
            let mut res = vec![("_matched".to_string(), "true".to_string())];
            res.extend(params);
            return res;
        }
        let Some(&actual) = path_segs.get(i) else {
            return vec![("_matched".to_string(), "false".to_string())];
        };
        if let Some(param_name) = pseg.strip_prefix(':') {
            params.push((param_name.to_string(), actual.to_string()));
        } else if pseg != actual {
            return vec![("_matched".to_string(), "false".to_string())];
        }
        i += 1;
    }

    if path_segs.len() != pat_segs.len() {
        return vec![("_matched".to_string(), "false".to_string())];
    }

    let mut res = vec![("_matched".to_string(), "true".to_string())];
    res.extend(params);
    res
}

pub fn tcp_listen(addr: &str) -> Result<i64, String> {
    let listener =
        TcpListener::bind(addr).map_err(|e| format!("Failed to bind TCP socket on '{addr}': {e}"))?;
    let id = alloc_id();
    with_registry(|reg| {
        reg.tcp_listeners.insert(id, Arc::new(listener));
    });
    Ok(id)
}

pub fn tcp_port(handle_id: i64) -> Result<i64, String> {
    let addr = with_registry(|reg| {
        if let Some(l) = reg.tcp_listeners.get(&handle_id) {
            return Some(l.local_addr());
        }
        if let Some(u) = reg.udp_sockets.get(&handle_id) {
            return Some(u.local_addr());
        }
        if let Some(h) = reg.http_servers.get(&handle_id) {
            return Some(h.local_addr());
        }
        None
    })
    .ok_or_else(|| format!("Invalid network listener/socket handle {handle_id}."))?;
    let port = addr
        .map_err(|e| format!("Failed to query local port: {e}"))?
        .port();
    Ok(port as i64)
}

pub fn tcp_accept(listener_id: i64) -> Result<i64, String> {
    let listener = with_registry(|reg| reg.tcp_listeners.get(&listener_id).cloned())
        .ok_or_else(|| format!("Invalid TCP listener handle {listener_id}."))?;
    let (stream, _) = listener
        .accept()
        .map_err(|e| format!("TCP accept failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let id = alloc_id();
    with_registry(|reg| {
        reg.tcp_conns.insert(id, Arc::new(Mutex::new(stream)));
    });
    Ok(id)
}

pub fn tcp_connect(addr: &str) -> Result<i64, String> {
    let mut addrs = addr
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{addr}': {e}"))?;
    let sock_addr = addrs
        .next()
        .ok_or_else(|| format!("No resolved socket address for '{addr}'"))?;
    let stream = TcpStream::connect_timeout(&sock_addr, Duration::from_secs(10))
        .map_err(|e| format!("TCP connect to '{addr}' failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let id = alloc_id();
    with_registry(|reg| {
        reg.tcp_conns.insert(id, Arc::new(Mutex::new(stream)));
    });
    Ok(id)
}

pub fn tcp_send(conn_id: i64, data: &str) -> Result<(), String> {
    let conn = with_registry(|reg| reg.tcp_conns.get(&conn_id).cloned())
        .ok_or_else(|| format!("Invalid TCP connection handle {conn_id}."))?;
    let mut stream = conn.lock().unwrap_or_else(|e| e.into_inner());
    stream
        .write_all(data.as_bytes())
        .map_err(|e| format!("TCP send failed: {e}"))?;
    let _ = stream.flush();
    Ok(())
}

pub fn tcp_recv(conn_id: i64, max_bytes: i64) -> Result<String, String> {
    let conn = with_registry(|reg| reg.tcp_conns.get(&conn_id).cloned())
        .ok_or_else(|| format!("Invalid TCP connection handle {conn_id}."))?;
    let cap = max_bytes.clamp(1, 1_048_576) as usize;
    let mut buf = vec![0u8; cap];
    let mut stream = conn.lock().unwrap_or_else(|e| e.into_inner());
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("TCP recv failed: {e}"))?;
    Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
}

pub fn tcp_set_timeout(conn_id: i64, timeout_ms: i64) -> Result<(), String> {
    let dur = if timeout_ms <= 0 {
        None
    } else {
        Some(Duration::from_millis(timeout_ms as u64))
    };
    if let Some(conn) = with_registry(|reg| reg.tcp_conns.get(&conn_id).cloned()) {
        let stream = conn.lock().unwrap_or_else(|e| e.into_inner());
        stream
            .set_read_timeout(dur)
            .map_err(|e| format!("Failed setting read timeout: {e}"))?;
        stream
            .set_write_timeout(dur)
            .map_err(|e| format!("Failed setting write timeout: {e}"))?;
        return Ok(());
    }
    if let Some(ws) = with_registry(|reg| reg.ws_conns.get(&conn_id).cloned()) {
        let guard = ws.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .stream
            .set_read_timeout(dur)
            .map_err(|e| format!("Failed setting WebSocket timeout: {e}"))?;
        guard
            .stream
            .set_write_timeout(dur)
            .map_err(|e| format!("Failed setting WebSocket timeout: {e}"))?;
        return Ok(());
    }
    Err(format!("Invalid connection handle {conn_id}."))
}

pub fn tcp_remote_addr(conn_id: i64) -> Result<String, String> {
    let conn = with_registry(|reg| reg.tcp_conns.get(&conn_id).cloned())
        .ok_or_else(|| format!("Invalid TCP connection handle {conn_id}."))?;
    let stream = conn.lock().unwrap_or_else(|e| e.into_inner());
    let addr = stream
        .peer_addr()
        .map_err(|e| format!("Failed reading remote address: {e}"))?;
    Ok(addr.to_string())
}

pub fn tcp_close(handle_id: i64) -> Result<(), String> {
    let removed = with_registry(|reg| {
        reg.tcp_conns.remove(&handle_id).is_some()
            || reg.tcp_listeners.remove(&handle_id).is_some()
            || reg.http_servers.remove(&handle_id).is_some()
            || reg.ws_conns.remove(&handle_id).is_some()
            || reg.udp_sockets.remove(&handle_id).is_some()
    });
    if removed {
        Ok(())
    } else {
        Err(format!("Invalid network handle {handle_id}."))
    }
}

pub fn dns_lookup(host: &str) -> Result<Vec<String>, String> {
    let lookup_target = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:0")
    } else if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:0")
    };
    let addrs = lookup_target
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?;
    let mut out = Vec::new();
    for addr in addrs {
        let ip = addr.ip().to_string();
        if !out.contains(&ip) {
            out.push(ip);
        }
    }
    Ok(out)
}

pub fn port_is_open(addr: &str, timeout_ms: i64) -> bool {
    let Ok(mut addrs) = addr.to_socket_addrs() else {
        return false;
    };
    let Some(sock) = addrs.next() else {
        return false;
    };
    let timeout = Duration::from_millis(timeout_ms.clamp(10, 30_000) as u64);
    TcpStream::connect_timeout(&sock, timeout).is_ok()
}

pub fn ws_upgrade(req_id: &str) -> Result<i64, String> {
    let mut pending = with_registry(|reg| reg.pending_http_reqs.remove(req_id))
        .ok_or_else(|| format!("Invalid or already-handled HTTP request id '{req_id}'."))?;
    let key = pending
        .headers
        .get("sec-websocket-key")
        .cloned()
        .ok_or_else(|| "HTTP request is missing Sec-WebSocket-Key upgrade header.".to_string())?;
    let mut accept_src = key.trim().as_bytes().to_vec();
    accept_src.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let accept = base64_encode(&sha1_bytes(&accept_src));

    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    pending
        .stream
        .write_all(resp.as_bytes())
        .map_err(|e| format!("Failed writing WebSocket handshake: {e}"))?;
    let _ = pending.stream.flush();

    let id = alloc_id();
    with_registry(|reg| {
        reg.ws_conns.insert(
            id,
            Arc::new(Mutex::new(WsConn {
                stream: pending.stream,
                is_client: false,
                leftover: Vec::new(),
            })),
        );
    });
    Ok(id)
}

pub fn ws_connect(ws_url: &str) -> Result<i64, String> {
    if ws_url.starts_with("wss://") {
        return Err("WSS (TLS WebSocket) is not supported in this zero-crate build; use `ws://` for local WebSocket services.".to_string());
    }
    let rest = ws_url
        .strip_prefix("ws://")
        .ok_or_else(|| "WebSocket URL must start with 'ws://'.".to_string())?;
    let (authority, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = split_host_port(authority, "80")?;
    let connect_addr = format!("{host}:{port}");
    let mut addrs = connect_addr
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{connect_addr}': {e}"))?;
    let sock = addrs
        .next()
        .ok_or_else(|| format!("No resolved socket address for '{connect_addr}'."))?;
    let mut stream = TcpStream::connect_timeout(&sock, Duration::from_secs(10))
        .map_err(|e| format!("WebSocket TCP connect failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let key_bytes = pseudo_random_bytes(16);
    let sec_key = base64_encode(&key_bytes);
    let req = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {authority}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {sec_key}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("Failed writing WebSocket handshake: {e}"))?;
    let _ = stream.flush();

    let mut raw = Vec::with_capacity(1024);
    let mut buf = [0u8; 1024];
    let header_end = loop {
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("Failed reading WebSocket handshake response: {e}"))?;
        if n == 0 {
            return Err("Server closed connection during WebSocket handshake.".to_string());
        }
        raw.extend_from_slice(&buf[..n]);
        if let Some(pos) = find_double_crlf(&raw) {
            break pos;
        }
        if raw.len() > 16_384 {
            return Err("WebSocket handshake headers too large.".to_string());
        }
    };

    let head = String::from_utf8_lossy(&raw[..header_end]);
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if status != 101 {
        return Err(format!(
            "WebSocket upgrade rejected with HTTP status {status}."
        ));
    }

    let leftover = raw[header_end + 4..].to_vec();
    let id = alloc_id();
    with_registry(|reg| {
        reg.ws_conns.insert(
            id,
            Arc::new(Mutex::new(WsConn {
                stream,
                is_client: true,
                leftover,
            })),
        );
    });
    Ok(id)
}

pub fn ws_send(ws_id: i64, msg: &str) -> Result<(), String> {
    let conn = with_registry(|reg| reg.ws_conns.get(&ws_id).cloned())
        .ok_or_else(|| format!("Invalid WebSocket handle {ws_id}."))?;
    let mut guard = conn.lock().unwrap_or_else(|e| e.into_inner());
    let is_client = guard.is_client;
    write_ws_frame(&mut guard.stream, 0x1, msg.as_bytes(), is_client)
}

pub fn ws_recv(ws_id: i64) -> Result<String, String> {
    let conn = with_registry(|reg| reg.ws_conns.get(&ws_id).cloned())
        .ok_or_else(|| format!("Invalid WebSocket handle {ws_id}."))?;
    let mut guard = conn.lock().unwrap_or_else(|e| e.into_inner());
    let mut msg_buf = Vec::new();
    loop {
        let (fin, opcode, payload) = read_ws_frame(&mut guard)?;
        match opcode {
            0x8 => {
                // Close frame: echo close frame cleanly
                let is_client = guard.is_client;
                let _ = write_ws_frame(&mut guard.stream, 0x8, &[], is_client);
                return Ok(String::new());
            }
            0x9 => {
                // Ping frame: auto-reply with Pong
                let is_client = guard.is_client;
                let _ = write_ws_frame(&mut guard.stream, 0xA, &payload, is_client);
                continue;
            }
            0xA => {
                // Pong frame: ignore and continue reading
                continue;
            }
            0x1 | 0x2 | 0x0 => {
                msg_buf.extend_from_slice(&payload);
                if fin {
                    return Ok(String::from_utf8_lossy(&msg_buf).into_owned());
                }
            }
            other => {
                return Err(format!("Unsupported WebSocket opcode 0x{other:x}."));
            }
        }
    }
}

pub fn ws_ping(ws_id: i64, msg: &str) -> Result<(), String> {
    let conn = with_registry(|reg| reg.ws_conns.get(&ws_id).cloned())
        .ok_or_else(|| format!("Invalid WebSocket handle {ws_id}."))?;
    let mut guard = conn.lock().unwrap_or_else(|e| e.into_inner());
    let is_client = guard.is_client;
    write_ws_frame(&mut guard.stream, 0x9, msg.as_bytes(), is_client)
}

pub fn ws_close(ws_id: i64) -> Result<(), String> {
    let conn = with_registry(|reg| reg.ws_conns.remove(&ws_id))
        .ok_or_else(|| format!("Invalid WebSocket handle {ws_id}."))?;
    let mut guard = conn.lock().unwrap_or_else(|e| e.into_inner());
    let is_client = guard.is_client;
    let _ = write_ws_frame(&mut guard.stream, 0x8, &[], is_client);
    Ok(())
}

pub fn local_ip() -> String {
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

fn write_ws_frame(
    stream: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
    masked: bool,
) -> Result<(), String> {
    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.push(0x80 | (opcode & 0x0F));
    let mask_bit = if masked { 0x80u8 } else { 0u8 };
    let len = payload.len();
    if len <= 125 {
        frame.push(mask_bit | (len as u8));
    } else if len <= 65_535 {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    if masked {
        let mask_arr = pseudo_random_bytes(4);
        let mask = [mask_arr[0], mask_arr[1], mask_arr[2], mask_arr[3]];
        frame.extend_from_slice(&mask);
        for (idx, b) in payload.iter().enumerate() {
            frame.push(b ^ mask[idx % 4]);
        }
    } else {
        frame.extend_from_slice(payload);
    }

    stream
        .write_all(&frame)
        .map_err(|e| format!("WebSocket write failed: {e}"))?;
    let _ = stream.flush();
    Ok(())
}

fn read_exact_ws(conn: &mut WsConn, count: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; count];
    let mut filled = 0usize;
    if !conn.leftover.is_empty() {
        let take = count.min(conn.leftover.len());
        out[..take].copy_from_slice(&conn.leftover[..take]);
        conn.leftover.drain(..take);
        filled = take;
    }
    if filled < count {
        conn.stream
            .read_exact(&mut out[filled..])
            .map_err(|e| format!("WebSocket read failed: {e}"))?;
    }
    Ok(out)
}

fn read_ws_frame(conn: &mut WsConn) -> Result<(bool, u8, Vec<u8>), String> {
    let header = read_exact_ws(conn, 2)?;
    let fin = (header[0] & 0x80) != 0;
    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let len7 = header[1] & 0x7F;

    let payload_len: usize = match len7 {
        126 => {
            let ext = read_exact_ws(conn, 2)?;
            u16::from_be_bytes([ext[0], ext[1]]) as usize
        }
        127 => {
            let ext = read_exact_ws(conn, 8)?;
            let raw_len = u64::from_be_bytes([
                ext[0], ext[1], ext[2], ext[3], ext[4], ext[5], ext[6], ext[7],
            ]);
            if raw_len > 16 * 1024 * 1024 {
                return Err("WebSocket frame exceeds 16 MiB limit.".to_string());
            }
            raw_len as usize
        }
        n => n as usize,
    };

    if payload_len > 16 * 1024 * 1024 {
        return Err("WebSocket frame exceeds 16 MiB limit.".to_string());
    }

    let mask_key = if masked {
        let m = read_exact_ws(conn, 4)?;
        Some([m[0], m[1], m[2], m[3]])
    } else {
        None
    };

    let mut payload = if payload_len == 0 {
        Vec::new()
    } else {
        read_exact_ws(conn, payload_len)?
    };

    if let Some(mask) = mask_key {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
    }

    Ok((fin, opcode, payload))
}

fn pseudo_random_bytes(count: usize) -> Vec<u8> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut state = nanos ^ (alloc_id() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let word = state.wrapping_mul(0x2545_f491_4f6c_dd1d).to_le_bytes();
        out.extend_from_slice(&word);
    }
    out.truncate(count);
    out
}

pub fn udp_bind(addr: &str) -> Result<i64, String> {
    let sock =
        UdpSocket::bind(addr).map_err(|e| format!("Failed to bind UDP socket on '{addr}': {e}"))?;
    let _ = sock.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = sock.set_write_timeout(Some(Duration::from_secs(30)));
    let id = alloc_id();
    with_registry(|reg| {
        reg.udp_sockets.insert(id, Arc::new(sock));
    });
    Ok(id)
}

pub fn udp_send_to(sock_id: i64, target_addr: &str, data: &str) -> Result<i64, String> {
    let sock = with_registry(|reg| reg.udp_sockets.get(&sock_id).cloned())
        .ok_or_else(|| format!("Invalid UDP socket handle {sock_id}."))?;
    let sent = sock
        .send_to(data.as_bytes(), target_addr)
        .map_err(|e| format!("UDP send_to '{target_addr}' failed: {e}"))?;
    Ok(sent as i64)
}

pub fn udp_recv_from(sock_id: i64, max_bytes: i64) -> Result<Vec<(String, String)>, String> {
    let sock = with_registry(|reg| reg.udp_sockets.get(&sock_id).cloned())
        .ok_or_else(|| format!("Invalid UDP socket handle {sock_id}."))?;
    let cap = max_bytes.clamp(1, 65_535) as usize;
    let mut buf = vec![0u8; cap];
    let (n, remote) = sock
        .recv_from(&mut buf)
        .map_err(|e| format!("UDP recv_from failed: {e}"))?;
    Ok(vec![
        (
            "data".to_string(),
            String::from_utf8_lossy(&buf[..n]).into_owned(),
        ),
        ("remote".to_string(), remote.to_string()),
    ])
}

pub fn udp_close(sock_id: i64) -> Result<(), String> {
    let existed = with_registry(|reg| reg.udp_sockets.remove(&sock_id).is_some());
    if existed {
        Ok(())
    } else {
        Err(format!("Invalid UDP socket handle {sock_id}."))
    }
}

pub struct HttpClientResponse {
    pub status: u16,
    pub body: String,
    pub headers_json: String,
    pub final_url: String,
}

pub fn http_client_request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
) -> Result<HttpClientResponse, String> {
    http_client_request_with_timeout(method, url, headers, body, 15_000)
}

pub fn http_client_request_with_timeout(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    timeout_ms: i64,
) -> Result<HttpClientResponse, String> {
    let timeout = Duration::from_millis(timeout_ms.clamp(100, 120_000) as u64);
    let mut cur_method = method.trim().to_ascii_uppercase();
    let mut cur_url = url.to_string();
    let mut cur_body = body.to_string();

    for _hop in 0..=5 {
        if cur_url.starts_with("https://") {
            return https_via_curl(&cur_method, &cur_url, headers, &cur_body, timeout);
        }
        let resp = single_http_request(&cur_method, &cur_url, headers, &cur_body, timeout)?;
        if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
            if let Some(location) = extract_header_from_json(&resp.headers_json, "location") {
                cur_url = resolve_redirect_url(&cur_url, &location);
                if matches!(resp.status, 301 | 302 | 303)
                    && cur_method != "GET"
                    && cur_method != "HEAD"
                {
                    cur_method = "GET".to_string();
                    cur_body.clear();
                }
                continue;
            }
        }
        return Ok(resp);
    }

    Err(format!("HTTP redirect limit (5 hops) exceeded from '{url}'."))
}

fn single_http_request(
    upper_method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    timeout: Duration,
) -> Result<HttpClientResponse, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "URL must start with 'http://' or 'https://'".to_string())?;
    let (authority, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = split_host_port(authority, "80")?;
    let addr_str = format!("{host}:{port}");

    let mut addrs = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {e}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| "No resolved socket address".to_string())?;

    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("Connection failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let mut req = format!(
        "{upper_method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nUser-Agent: dog-lang/{}\r\n",
        env!("CARGO_PKG_VERSION")
    );
    let mut has_len = false;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("content-length") {
            has_len = true;
        }
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() && !has_len {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");

    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("Write failed: {e}"))?;
    if !body.is_empty() {
        stream
            .write_all(body.as_bytes())
            .map_err(|e| format!("Write body failed: {e}"))?;
    }

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("Read failed: {e}"))?;

    parse_raw_http_response(&raw, url)
}

fn resolve_redirect_url(current_url: &str, location: &str) -> String {
    let loc = location.trim();
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return loc.to_string();
    }
    let Some((scheme, rest)) = current_url.split_once("://") else {
        return loc.to_string();
    };
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    if loc.starts_with('/') {
        format!("{scheme}://{authority}{loc}")
    } else {
        format!("{scheme}://{authority}/{loc}")
    }
}

fn extract_header_from_json(headers_json: &str, name: &str) -> Option<String> {
    let Ok(JsonValue::Object(map)) = parse_json(headers_json) else {
        return None;
    };
    if let Some(JsonValue::Str(s)) = map.get(&name.to_ascii_lowercase()) {
        return Some(s.clone());
    }
    None
}

fn parse_headers_json(head: &str) -> (u16, String, bool) {
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(200);
    let mut map = BTreeMap::new();
    let mut is_chunked = false;
    for line in head.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            if key == "transfer-encoding" && val.to_ascii_lowercase().contains("chunked") {
                is_chunked = true;
            }
            map.insert(key, JsonValue::Str(val));
        }
    }
    (status, JsonValue::Object(map).stringify(), is_chunked)
}

fn parse_raw_http_response(raw: &[u8], url: &str) -> Result<HttpClientResponse, String> {
    if let Some(split_pos) = find_double_crlf(raw) {
        let head = String::from_utf8_lossy(&raw[..split_pos]);
        let (status, headers_json, is_chunked) = parse_headers_json(&head);
        let raw_body = &raw[split_pos + 4..];
        let body = if is_chunked {
            decode_chunked_body(raw_body)
        } else {
            String::from_utf8_lossy(raw_body).into_owned()
        };
        Ok(HttpClientResponse {
            status,
            body,
            headers_json,
            final_url: url.to_string(),
        })
    } else {
        Ok(HttpClientResponse {
            status: 200,
            body: String::from_utf8_lossy(raw).into_owned(),
            headers_json: "{}".to_string(),
            final_url: url.to_string(),
        })
    }
}

fn decode_chunked_body(data: &[u8]) -> String {
    let mut reader = BufReader::new(data);
    let mut out = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let hex_len = line.trim().split(';').next().unwrap_or("0");
        let Ok(chunk_size) = usize::from_str_radix(hex_len, 16) else {
            break;
        };
        if chunk_size == 0 {
            break;
        }
        let mut chunk = vec![0u8; chunk_size];
        if reader.read_exact(&mut chunk).is_err() {
            break;
        }
        out.extend_from_slice(&chunk);
        let mut crlf = [0u8; 2];
        let _ = reader.read_exact(&mut crlf);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn https_via_curl(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    timeout: Duration,
) -> Result<HttpClientResponse, String> {
    let upper_method = method.trim().to_ascii_uppercase();
    let secs = timeout.as_secs().max(1).to_string();
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sS",
        "-i",
        "-L",
        "--max-redirs",
        "5",
        "-X",
        upper_method.as_str(),
        "--connect-timeout",
        secs.as_str(),
        "--max-time",
        secs.as_str(),
        "-w",
        "\n__DOG_FINAL_URL__:%{url_effective}",
    ]);
    for (k, v) in headers {
        cmd.arg("-H").arg(format!("{k}: {v}"));
    }
    if !body.is_empty() {
        cmd.args(["--data-binary", "@-"]);
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.arg(url);

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "HTTPS requires `curl` on PATH in this zero-crate build (or use plain `http://` locally): {e}"
        )
    })?;

    if !body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(body.as_bytes());
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed waiting for curl HTTPS request: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if err.is_empty() {
            "HTTPS request failed via curl.".to_string()
        } else {
            err
        });
    }

    let marker = b"\n__DOG_FINAL_URL__:";
    let (raw_http, final_url) = if let Some(idx) = output
        .stdout
        .windows(marker.len())
        .rposition(|w| w == marker)
    {
        let u = String::from_utf8_lossy(&output.stdout[idx + marker.len()..])
            .trim()
            .to_string();
        (&output.stdout[..idx], if u.is_empty() { url.to_string() } else { u })
    } else {
        (output.stdout.as_slice(), url.to_string())
    };

    // `curl -i -L` may output multiple HTTP header blocks on redirects; take the last response block.
    let mut slice = raw_http;
    loop {
        if let Some(pos) = find_double_crlf(slice) {
            let rest = &slice[pos + 4..];
            if rest.starts_with(b"HTTP/") {
                slice = rest;
                continue;
            }
            let head = String::from_utf8_lossy(&slice[..pos]);
            let (status, headers_json, _) = parse_headers_json(&head);
            let body_str = String::from_utf8_lossy(rest).into_owned();
            return Ok(HttpClientResponse {
                status,
                body: body_str,
                headers_json,
                final_url,
            });
        }
        return Ok(HttpClientResponse {
            status: 200,
            body: String::from_utf8_lossy(slice).into_owned(),
            headers_json: "{}".to_string(),
            final_url,
        });
    }
}

pub fn http_download_bytes<F>(url: &str, mut on_progress: F) -> Result<Vec<u8>, String>
where
    F: FnMut(u64, Option<u64>),
{
    if url.starts_with("https://") {
        let mut cmd = Command::new("curl");
        cmd.args(["-sSLf", "--connect-timeout", "15", url])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "HTTPS download requires `curl` on PATH in this zero-crate build (or use `http://` / local paths): {e}"
            )
        })?;
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        if let Some(mut stdout) = child.stdout.take() {
            loop {
                let n = stdout
                    .read(&mut buf)
                    .map_err(|e| format!("Failed reading HTTPS response stream: {e}"))?;
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n]);
                on_progress(out.len() as u64, None);
            }
        }
        let status = child
            .wait()
            .map_err(|e| format!("Failed waiting for curl process: {e}"))?;
        if !status.success() {
            let mut err_buf = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut err_buf);
            }
            let trimmed = err_buf.trim();
            return Err(if trimmed.is_empty() {
                format!("HTTP/HTTPS download failed for '{url}'.")
            } else {
                trimmed.to_string()
            });
        }
        return Ok(out);
    }

    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "URL must start with 'http://' or 'https://'".to_string())?;
    let (authority, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = split_host_port(authority, "80")?;
    let addr_str = format!("{host}:{port}");

    let mut addrs = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| format!("No resolved socket address for '{host}'"))?;

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(15))
        .map_err(|e| format!("Connection to '{addr_str}' failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nUser-Agent: dog-lang/{}\r\n\r\n",
        env!("CARGO_PKG_VERSION")
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("Failed sending HTTP request: {e}"))?;

    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    let header_end;
    loop {
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("Failed reading HTTP headers: {e}"))?;
        if n == 0 {
            return Err("Server closed connection before sending HTTP headers.".to_string());
        }
        raw.extend_from_slice(&buf[..n]);
        if let Some(pos) = find_double_crlf(&raw) {
            header_end = pos;
            break;
        }
        if raw.len() > 65_536 {
            return Err("HTTP response headers exceeded 64 KiB.".to_string());
        }
    }

    let head = String::from_utf8_lossy(&raw[..header_end]);
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if !(200..=299).contains(&status) {
        return Err(format!("Remote server returned HTTP {status} for '{url}'."));
    }

    let mut content_len: Option<u64> = None;
    let mut is_chunked = false;
    for line in head.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse::<u64>().ok();
            } else if k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
            {
                is_chunked = true;
            }
        }
    }

    let mut body = raw[header_end + 4..].to_vec();
    if !body.is_empty() {
        on_progress(body.len() as u64, content_len);
    }
    loop {
        if let Some(expected) = content_len {
            if body.len() as u64 >= expected {
                break;
            }
        }
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("Failed reading HTTP body: {e}"))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        on_progress(body.len() as u64, content_len);
    }

    if is_chunked {
        Ok(decode_chunked_body(&body).into_bytes())
    } else {
        Ok(body)
    }
}

pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn url_decode(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("Incomplete percent-encoding in url_decode().".to_string());
                }
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .map_err(|_| "Invalid UTF-8 escape in url_decode().".to_string())?;
                let val = u8::from_str_radix(hex, 16)
                    .map_err(|_| format!("Invalid percent escape '%{hex}' in url_decode()."))?;
                out.push(val);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| "Decoded URL bytes are not valid UTF-8.".to_string())
}

pub fn csv_parse(raw: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let clean = line.trim_end_matches('\r');
        if clean.is_empty() && rows.is_empty() {
            continue;
        }
        let mut row = Vec::new();
        let mut cell = String::new();
        let chars: Vec<char> = clean.chars().collect();
        let mut i = 0usize;
        let mut in_quotes = false;
        while i < chars.len() {
            let ch = chars[i];
            if in_quotes {
                if ch == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        cell.push('"');
                        i += 2;
                        continue;
                    }
                    in_quotes = false;
                } else {
                    cell.push(ch);
                }
            } else if ch == '"' {
                in_quotes = true;
            } else if ch == ',' {
                row.push(std::mem::take(&mut cell));
            } else {
                cell.push(ch);
            }
            i += 1;
        }
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn load_kv_map(path: &str) -> Result<BTreeMap<String, JsonValue>, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(p).map_err(|e| format!("Failed to read KV store '{path}': {e}"))?;
    if raw.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    match parse_json(&raw)? {
        JsonValue::Object(map) => Ok(map),
        _ => Err(format!("KV file '{path}' must contain a JSON object.")),
    }
}

fn save_kv_map(path: &str, map: &BTreeMap<String, JsonValue>) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create KV directory: {e}"))?;
        }
    }
    let rendered = JsonValue::Object(map.clone()).stringify();
    fs::write(p, format!("{rendered}\n"))
        .map_err(|e| format!("Failed to write KV store '{path}': {e}"))
}

pub fn kv_set(path: &str, key: &str, val: &str) -> Result<(), String> {
    let mut map = load_kv_map(path)?;
    map.insert(key.to_string(), JsonValue::Str(val.to_string()));
    save_kv_map(path, &map)
}

pub fn kv_get(path: &str, key: &str) -> Result<Option<String>, String> {
    let map = load_kv_map(path)?;
    Ok(map.get(key).map(|v| match v {
        JsonValue::Str(s) => s.clone(),
        other => other.stringify(),
    }))
}

pub fn kv_has(path: &str, key: &str) -> Result<bool, String> {
    let map = load_kv_map(path)?;
    Ok(map.contains_key(key))
}

pub fn kv_delete(path: &str, key: &str) -> Result<bool, String> {
    let mut map = load_kv_map(path)?;
    let removed = map.remove(key).is_some();
    if removed {
        save_kv_map(path, &map)?;
    }
    Ok(removed)
}

pub fn kv_keys(path: &str) -> Result<Vec<String>, String> {
    let map = load_kv_map(path)?;
    Ok(map.keys().cloned().collect())
}

pub fn exec_shell_cmd(cmd_str: &str) -> Result<(i32, String, String), String> {
    let out = if cfg!(windows) {
        Command::new("cmd").args(["/C", cmd_str]).output()
    } else {
        Command::new("sh").args(["-c", cmd_str]).output()
    }
    .map_err(|e| format!("Failed to execute command '{cmd_str}': {e}"))?;

    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((code, stdout, stderr))
}

pub fn format_uuid_v4(mut seed_a: u64, mut seed_b: u64) -> String {
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        seed_a ^= seed_a >> 30;
        seed_a = seed_a.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        seed_a ^= seed_a >> 27;
        seed_a = seed_a.wrapping_mul(0x94d0_49bb_1331_11eb);
        seed_a ^= seed_a >> 31;
        let combined = seed_a ^ seed_b;
        chunk.copy_from_slice(&combined.to_le_bytes());
        seed_b = seed_b.wrapping_add(0x9e37_79b9_7f4a_7c15);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h = hex_encode(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}
