# DOG Standard Library (v0.2.0)

## Language Ergonomics
- **String Interpolation & Triple Strings**: `$"Hello, {name}! Next = {count + 1}"` (escape literal braces via `{{` and `}}`), and multi-line triple-quoted `"""..."""` strings with verbatim formatting preservation. Plain `"..."` remains strictly literal for safe JSON/templates.
- **Numeric Literals**: Supports underscores as digit separators (`1_000_000`, `3.14_15`), hexadecimal (`0xFF_00`), and binary (`0b1010_0101`).
- **Chained Conditionals**: `if cond ... otherwise if cond2 ... otherwise ... end`
- **Compound Assignment**: `x += 1`, `x -= 2`, `x *= 3`, `x /= 2`, `x %= 5`, `s += "!"` on mutable `var` bindings (with zero-division `E031` protection).
- **Typo Hints**: Compile diagnostics (`E004`, `E010`, `E013`) and CLI subcommands emit Levenshtein `"Did you mean ...?"` suggestions.

## Core I/O, CLI Messaging, Logging & Assertions
- `say(value) -> void`: Prints value to stdout followed by newline.
- `answer(prompt?: text) -> text`: Prints optional prompt and reads one line from stdin.
- `log_info(msg: text) -> void`, `log_warn(msg: text) -> void`, `log_error(msg: text) -> void`
- `cli_info(msg: text) -> void`, `cli_success(msg: text) -> void`, `cli_warn(msg: text) -> void`, `cli_error(msg: text) -> void`, `cli_debug(msg: text) -> void`
- `cli_progress(label: text, current: number, total: number) -> void`, `cli_spinner(step: number, label: text) -> void`
- `cli_theme(name: text) -> void` (`default`, `kali`, `dracula`, `solar`, `mono`), `cli_prefix(prefix: text) -> void`, `cli_timestamps(enabled: bool) -> void`
- `cli_supports_color() -> bool`, `cli_is_tty() -> bool`
- `assert(cond: bool, msg?: text) -> void`: Fails with `E033` if `cond` is `false`.
- `fail(reason: text) -> void`: Raises a catchable `E035` runtime error.

## Explicit Type Conversions
- `text(value) -> text`: Explicit string conversion for any non-void value.
- `number(value: text | number | decimal) -> number`: Strict integer conversion (`i64`).
- `decimal(value: text | number | decimal) -> decimal`: Strict floating-point conversion (`f64`).

## Text & Formatting Helpers
- `lower(t: text) -> text`, `upper(t: text) -> text`, `trim(t: text) -> text`, `title_case(s: text) -> text`, `reverse_text(s: text) -> text`
- `contains(haystack: text, needle: text) -> bool`, `starts_with(t: text, prefix: text) -> bool`, `ends_with(t: text, suffix: text) -> bool`
- `replace(t: text, from: text, to: text) -> text`, `slice(t: text, start: number, end: number) -> text`, `char_at(s: text, idx: number) -> text`
- `remove_prefix(s: text, prefix: text) -> text`, `remove_suffix(s: text, suffix: text) -> text`
- `split(t: text, sep: text) -> list<text>`, `join(items: list<text>, sep: text) -> text`, `lines(s: text) -> list<text>`
- `index_of(haystack: text, needle: text) -> number`, `last_index_of(haystack: text, needle: text) -> number`, `count_matches(haystack: text, needle: text) -> number`
- `glob_match(pattern: text, text: text) -> bool` (`*` and `?` wildcards)
- `str_repeat(s: text, count: number) -> text`, `pad_left(s: text, width: number, fill: text) -> text`, `pad_right(s: text, width: number, fill: text) -> text`
- `html_escape(s: text) -> text` (escapes `&`, `<`, `>`, `"`, `'`)
- `len(v: text | list<T> | map<K, V>) -> number`

## List & Map Operations (`var` binding required for in-place mutation)
- `push(list_var: var list<T>, item: T) -> void`, `pop(list_var: var list<T>) -> T`
- `insert_at(list_var: var list<T>, idx: number, item: T) -> void`, `remove_at(list_var: var list<T>, idx: number) -> T`, `clear(list_var: var list<T>) -> void`
- `sort(list_var: var list<T>) -> void`, `reverse(list_var: var list<T>) -> void`
- `list_contains(items: list<T>, item: T) -> bool`, `list_index_of(items: list<T>, item: T) -> number`, `list_slice(items: list<T>, start: number, end: number) -> list<T>`
- `list_unique(items: list<T>) -> list<T>`, `list_take(items: list<T>, n: number) -> list<T>`, `list_skip(items: list<T>, n: number) -> list<T>`
- `list_flatten(lists: list<list<T>>) -> list<T>`, `list_concat(a: list<T>, b: list<T>) -> list<T>`
- `list_sum(items: list<T>) -> T`, `list_avg(items: list<T>) -> decimal`, `range(start: number, end: number) -> list<number>`
- `map_get(m: map<K, V>, key: K) -> V`, `map_get_or(m: map<K, V>, key: K, fallback: V) -> V`
- `map_set(map_var: var map<K, V>, key: K, val: V) -> void`, `map_has(m: map<K, V>, key: K) -> bool`
- `map_keys(m: map<K, V>) -> list<K>`, `map_values(m: map<K, V>) -> list<V>`, `map_from_lists(keys: list<K>, vals: list<V>) -> map<K, V>`
- `map_remove(map_var: var map<K, V>, key: K) -> bool`, `map_clear(map_var: var map<K, V>) -> void`, `map_merge(dst_var: var map<K, V>, src: map<K, V>) -> void`

## Math & Random Helpers
- `abs(n: number | decimal) -> number | decimal`, `sign(n: number | decimal) -> number`
- `min(a: T, b: T) -> T`, `max(a: T, b: T) -> T`, `clamp(x: T, lo: T, hi: T) -> T`
- `round(x: decimal) -> number`, `floor(x: decimal) -> number`, `ceil(x: decimal) -> number`
- `sqrt(x: decimal) -> decimal`, `pow(base: decimal, exp: decimal) -> decimal`, `pow_int(base: number, exp: number) -> number`
- `sin(rad: decimal) -> decimal`, `cos(rad: decimal) -> decimal`, `tan(rad: decimal) -> decimal`, `atan2(y: decimal, x: decimal) -> decimal`
- `exp(x: decimal) -> decimal`, `log(x: decimal) -> decimal`, `log10(x: decimal) -> decimal`
- `gcd(a: number, b: number) -> number`, `lcm(a: number, b: number) -> number`, `math_pi() -> decimal`, `math_e() -> decimal`
- `rand_int(min: number, max: number) -> number`

## Filesystem, Embedded KV Database, Environment, CLI & System
- `file_read(path: text) -> text`, `file_write(path: text, content: text) -> void`, `file_append(path: text, content: text) -> void`
- `file_exists(path: text) -> bool`, `file_delete(path: text) -> bool`, `file_copy(src: text, dst: text) -> void`, `file_move(src: text, dst: text) -> void`, `file_size(path: text) -> number`
- `is_file(path: text) -> bool`, `is_dir(path: text) -> bool`
- `path_join(a: text, b: text) -> text`, `path_dirname(path: text) -> text`, `path_basename(path: text) -> text`, `path_ext(path: text) -> text`
- `dir_list(path: text) -> list<text>`, `mkdir(path: text) -> void`, `rmdir(path: text) -> void`, `temp_dir() -> text`, `cwd() -> text`
- `kv_set(db: text, key: text, val: text) -> void`, `kv_get(db: text, key: text) -> text`
- `kv_has(db: text, key: text) -> bool`, `kv_delete(db: text, key: text) -> bool`, `kv_keys(db: text) -> list<text>`
- `args() -> list<text>`, `env_get(key: text) -> text`, `env_set(key: text, value: text) -> void`
- `exec_cmd(cmd: text) -> map<text, text>` (`code`, `stdout`, `stderr`)
- `os_name() -> text`, `os_arch() -> text`, `uuid_v4() -> text`
- `time_now_ms() -> number`, `time_utc() -> text` (ISO-8601), `sleep_ms(ms: number) -> void`

## JSON, CSV, URL, Cryptography & Networking
- `json_parse(raw: text) -> map<text, text>`, `json_stringify(val) -> text`, `json_pretty(raw_or_val) -> text`, `json_has_key(raw: text, key: text) -> bool`
- `csv_parse(raw: text) -> list<list<text>>`, `csv_stringify(rows: list<list<text>>) -> text`
- `url_encode(s: text) -> text`, `url_decode(s: text) -> text`
- `url_parse(url: text) -> map<text, text>` (`scheme`, `host`, `port`, `path`, `query`)
- `query_parse(query: text) -> map<text, text>`, `query_stringify(params: map<text, text>) -> text`
- `hex_encode(s: text) -> text`, `hex_decode(hex: text) -> text`
- `base64_encode(s: text) -> text`, `base64_decode(b64: text) -> text`, `base64_url_encode(s: text) -> text`, `base64_url_decode(b64u: text) -> text`
- `sha1(s: text) -> text`, `sha256(s: text) -> text`, `sha512(s: text) -> text`
- `hmac_sha256(key: text, msg: text) -> text`, `hmac_sha256_verify(key: text, msg: text, sig_hex: text) -> bool`, `rand_bytes_hex(count: number) -> text`
- **HTTP Client** (`http://` natively with 301/302/303/307/308 redirects, chunked decoding & IPv6; `https://` via system `curl`):
  - `http_get(url: text) -> text`, `http_post(url: text, body: text, content_type: text) -> text`
  - `http_put(url: text, body: text, content_type: text) -> text`, `http_delete(url: text) -> text`
  - `http_request(method: text, url: text, headers: map<text, text>, body: text) -> map<text, text>` (`status`, `body`, `headers`, `final_url`, `ok`)
  - `http_request_timeout(method: text, url: text, headers: map<text, text>, body: text, timeout_ms: number) -> map<text, text>`
  - `http_status_text(code: number) -> text`
- **Local HTTP / Website Server**:
  - `http_listen(addr: text) -> number`, `http_port(server_id: number) -> number`, `http_close(server_id: number) -> void`
  - `http_accept(server_id: number) -> map<text, text>` (supports `Content-Length` & `Transfer-Encoding: chunked`)
  - `http_try_accept(server_id: number, timeout_ms: number) -> map<text, text>` (`_ok` = `"true"` | `"false"`)
  - `http_reply(req: map<text, text>, status: number, content_type: text, body: text) -> void`
  - `http_reply_headers(req: map<text, text>, status: number, headers: map<text, text>, body: text) -> void`
  - `http_redirect(req: map<text, text>, status: number, location: text) -> void`
  - `http_serve_file(req: map<text, text>, path: text) -> void`
  - `http_route_match(pattern: text, path: text) -> map<text, text>` (`/users/:id/*rest`, `_matched`)
- **RFC 6455 WebSockets**:
  - `ws_upgrade(req: map<text, text>) -> number`, `ws_connect(url: text) -> number`
  - `ws_send(ws_id: number, msg: text) -> void`, `ws_recv(ws_id: number) -> text`, `ws_ping(ws_id: number, msg: text) -> void`, `ws_close(ws_id: number) -> void`
- **Raw TCP, UDP & Network Diagnostics**:
  - `tcp_listen(addr: text) -> number`, `tcp_port(id: number) -> number`, `tcp_accept(id: number) -> number`
  - `tcp_connect(addr: text) -> number`, `tcp_send(id: number, data: text) -> void`, `tcp_recv(id: number, max_bytes: number) -> text`
  - `tcp_set_timeout(id: number, timeout_ms: number) -> void`, `tcp_remote_addr(id: number) -> text`, `tcp_close(id: number) -> void`
  - `udp_bind(addr: text) -> number`, `udp_send_to(id: number, addr: text, data: text) -> void`, `udp_recv_from(id: number, max_bytes: number) -> map<text, text>`, `udp_close(id: number) -> void`
  - `dns_lookup(host: text) -> list<text>`, `local_ip() -> text`, `port_is_open(addr: text, timeout_ms: number) -> bool`

