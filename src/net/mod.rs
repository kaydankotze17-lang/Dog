use crate::crypto::hex_encode;
use crate::json::{parse_json, JsonValue};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

fn alloc_id() -> i64 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

#[derive(Default)]
struct NetRegistry {
    http_servers: HashMap<i64, Arc<TcpListener>>,
    pending_http_reqs: HashMap<String, TcpStream>,
    tcp_listeners: HashMap<i64, Arc<TcpListener>>,
    tcp_conns: HashMap<i64, Arc<Mutex<TcpStream>>>,
}

static NET_STATE: Mutex<Option<NetRegistry>> = Mutex::new(None);

fn with_registry<T>(f: impl FnOnce(&mut NetRegistry) -> T) -> T {
    let mut guard = NET_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let reg = guard.get_or_insert_with(NetRegistry::default);
    f(reg)
}

pub fn http_listen(addr: &str) -> Result<i64, String> {
    let listener =
        TcpListener::bind(addr).map_err(|e| format!("Failed to bind HTTP server on '{addr}': {e}"))?;
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

    let (mut stream, peer) = listener
        .accept()
        .map_err(|e| format!("HTTP accept failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let (method, path, query, headers, body) = read_http_request(&mut stream)?;

    let req_id = format!("req_{}", alloc_id());
    let content_type = headers.get("content-type").cloned().unwrap_or_default();
    let authorization = headers.get("authorization").cloned().unwrap_or_default();
    let mut header_json_map = BTreeMap::new();
    for (k, v) in headers {
        header_json_map.insert(k, JsonValue::Str(v));
    }
    let headers_json = JsonValue::Object(header_json_map).stringify();

    with_registry(|reg| {
        reg.pending_http_reqs.insert(req_id.clone(), stream);
    });

    Ok(vec![
        ("id".to_string(), req_id),
        ("method".to_string(), method),
        ("path".to_string(), path),
        ("query".to_string(), query),
        ("body".to_string(), body),
        ("remote".to_string(), peer.to_string()),
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

    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(8 * 1024 * 1024);

    let body_start = header_end + 4;
    let mut body_bytes = raw[body_start..].to_vec();
    while body_bytes.len() < content_length {
        let remaining = content_length - body_bytes.len();
        let take = remaining.min(buf.len());
        let n = stream
            .read(&mut buf[..take])
            .map_err(|e| format!("Failed reading HTTP request body: {e}"))?;
        if n == 0 {
            break;
        }
        body_bytes.extend_from_slice(&buf[..n]);
    }
    if body_bytes.len() > content_length {
        body_bytes.truncate(content_length);
    }

    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    Ok((method, path, query, headers, body))
}

fn find_double_crlf(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn http_reply(req_id: &str, status: i64, content_type: &str, body: &str) -> Result<(), String> {
    http_reply_bytes(req_id, status, content_type, body.as_bytes())
}

pub fn http_serve_file(req_id: &str, file_path: &str) -> Result<(), String> {
    let p = Path::new(file_path);
    if !p.is_file() {
        return http_reply_bytes(req_id, 404, "text/plain; charset=utf-8", b"404 Not Found\n");
    }
    let data = fs::read(p).map_err(|e| format!("Failed to read '{file_path}': {e}"))?;
    let ctype = guess_mime_type(p);
    http_reply_bytes(req_id, 200, ctype, &data)
}

fn http_reply_bytes(
    req_id: &str,
    status: i64,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let mut stream = with_registry(|reg| reg.pending_http_reqs.remove(req_id))
        .ok_or_else(|| format!("Invalid or already-replied HTTP request id '{req_id}'."))?;
    let reason = status_reason(status);
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|e| format!("Failed writing HTTP response header: {e}"))?;
    stream
        .write_all(body)
        .map_err(|e| format!("Failed writing HTTP response body: {e}"))?;
    let _ = stream.flush();
    Ok(())
}

fn status_reason(code: i64) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
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
        _ => "application/octet-stream",
    }
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

pub fn tcp_port(listener_id: i64) -> Result<i64, String> {
    let listener = with_registry(|reg| reg.tcp_listeners.get(&listener_id).cloned())
        .ok_or_else(|| format!("Invalid TCP listener handle {listener_id}."))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to query local TCP port: {e}"))?
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

pub fn tcp_close(handle_id: i64) -> Result<(), String> {
    let removed = with_registry(|reg| {
        reg.tcp_conns.remove(&handle_id).is_some()
            || reg.tcp_listeners.remove(&handle_id).is_some()
            || reg.http_servers.remove(&handle_id).is_some()
    });
    if removed {
        Ok(())
    } else {
        Err(format!("Invalid network handle {handle_id}."))
    }
}

pub struct HttpClientResponse {
    pub status: u16,
    pub body: String,
}

pub fn http_client_request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
) -> Result<HttpClientResponse, String> {
    if url.starts_with("https://") {
        return https_via_curl(method, url, headers, body);
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "URL must start with 'http://' or 'https://'".to_string())?;
    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, addr_str) = if host_port.contains(':') {
        (
            host_port.split(':').next().unwrap_or(host_port),
            host_port.to_string(),
        )
    } else {
        (host_port, format!("{host_port}:80"))
    };

    let mut addrs = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {e}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| "No resolved socket address".to_string())?;

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(15))
        .map_err(|e| format!("Connection failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    let upper_method = method.trim().to_ascii_uppercase();
    let mut req = format!("{upper_method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: dog-lang/0.1\r\n");
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

    parse_raw_http_response(&raw)
}

fn parse_raw_http_response(raw: &[u8]) -> Result<HttpClientResponse, String> {
    if let Some(split_pos) = find_double_crlf(raw) {
        let head = String::from_utf8_lossy(&raw[..split_pos]);
        let status = head
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(200);
        let is_chunked = head
            .lines()
            .any(|l| l.to_ascii_lowercase().contains("transfer-encoding: chunked"));
        let raw_body = &raw[split_pos + 4..];
        let body = if is_chunked {
            decode_chunked_body(raw_body)
        } else {
            String::from_utf8_lossy(raw_body).into_owned()
        };
        Ok(HttpClientResponse { status, body })
    } else {
        Ok(HttpClientResponse {
            status: 200,
            body: String::from_utf8_lossy(raw).into_owned(),
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
) -> Result<HttpClientResponse, String> {
    let upper_method = method.trim().to_ascii_uppercase();
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sS",
        "-i",
        "-L",
        "-X",
        upper_method.as_str(),
        "--connect-timeout",
        "15",
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

    // `curl -i -L` may output multiple HTTP header blocks on redirects; take the last response block.
    let mut slice = output.stdout.as_slice();
    loop {
        if let Some(pos) = find_double_crlf(slice) {
            let rest = &slice[pos + 4..];
            if rest.starts_with(b"HTTP/") {
                slice = rest;
                continue;
            }
            let head = String::from_utf8_lossy(&slice[..pos]);
            let status = head
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(200);
            let body_str = String::from_utf8_lossy(rest).into_owned();
            return Ok(HttpClientResponse {
                status,
                body: body_str,
            });
        }
        return Ok(HttpClientResponse {
            status: 200,
            body: String::from_utf8_lossy(slice).into_owned(),
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
    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, addr_str) = if host_port.contains(':') {
        (
            host_port.split(':').next().unwrap_or(host_port),
            host_port.to_string(),
        )
    } else {
        (host_port, format!("{host_port}:80"))
    };

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
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: dog-lang/{}\r\n\r\n",
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
    fs::write(p, format!("{rendered}\n")).map_err(|e| format!("Failed to write KV store '{path}': {e}"))
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
