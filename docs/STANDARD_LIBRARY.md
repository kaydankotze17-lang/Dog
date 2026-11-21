# DOG Standard Library (v0.1.0)

## Core I/O, Logging & Assertions
- `say(value) -> void`: Prints value to stdout followed by newline.
- `answer(prompt?: text) -> text`: Prints optional prompt and reads one line from stdin.
- `log_info(msg: text) -> void`, `log_warn(msg: text) -> void`, `log_error(msg: text) -> void`
- `assert(cond: bool, msg?: text) -> void`: Fails with `E033` if `cond` is `false`.
- `fail(reason: text) -> void`: Raises a catchable `E035` runtime error.

## Explicit Type Conversions
- `text(value) -> text`: Explicit string conversion for any non-void value.
- `number(value: text | number | decimal) -> number`: Strict integer conversion (`i64`).
- `decimal(value: text | number | decimal) -> decimal`: Strict floating-point conversion (`f64`).

## Text Helpers
- `lower(t: text) -> text`, `upper(t: text) -> text`, `trim(t: text) -> text`
- `contains(haystack: text, needle: text) -> bool`, `starts_with(t: text, prefix: text) -> bool`, `ends_with(t: text, suffix: text) -> bool`
- `replace(t: text, from: text, to: text) -> text`, `slice(t: text, start: number, end: number) -> text`
- `split(t: text, sep: text) -> list<text>`, `join(items: list<text>, sep: text) -> text`
- `len(v: text | list<T> | map<K, V>) -> number`

## List & Map Operations (`var` binding required for in-place mutation)
- `push(list_var: var list<T>, item: T) -> void`, `pop(list_var: var list<T>) -> T`
- `sort(list_var: var list<T>) -> void`, `reverse(list_var: var list<T>) -> void`
- `list_contains(items: list<T>, item: T) -> bool`, `range(start: number, end: number) -> list<number>`
- `map_get(m: map<K, V>, key: K) -> V`
- `map_set(map_var: var map<K, V>, key: K, val: V) -> void`
- `map_has(m: map<K, V>, key: K) -> bool`
- `map_keys(m: map<K, V>) -> list<K>`
- `map_remove(map_var: var map<K, V>, key: K) -> bool`

## Math & Random Helpers
- `abs(n: number | decimal) -> number | decimal`
- `min(a: T, b: T) -> T`, `max(a: T, b: T) -> T`
- `round(x: decimal) -> number`, `floor(x: decimal) -> number`, `ceil(x: decimal) -> number`
- `sqrt(x: decimal) -> decimal`, `pow(base: decimal, exp: decimal) -> decimal`
- `rand_int(min: number, max: number) -> number`

## Filesystem, Environment, CLI & Time
- `file_read(path: text) -> text`, `file_write(path: text, content: text) -> void`, `file_append(path: text, content: text) -> void`
- `file_exists(path: text) -> bool`, `file_delete(path: text) -> bool`
- `dir_list(path: text) -> list<text>`, `mkdir(path: text) -> void`, `cwd() -> text`
- `args() -> list<text>`, `env_get(key: text) -> text`, `env_set(key: text, value: text) -> void`
- `time_now_ms() -> number`, `sleep_ms(ms: number) -> void`

## JSON, Cryptography & HTTP
- `json_parse(raw: text) -> map<text, text>`, `json_stringify(val) -> text`
- `hex_encode(s: text) -> text`, `hex_decode(hex: text) -> text`
- `base64_encode(s: text) -> text`, `base64_decode(b64: text) -> text`
- `sha256(s: text) -> text`, `hmac_sha256(key: text, msg: text) -> text`
- `http_get(url: text) -> text` (plain `http://` over `std::net::TcpStream`; `https://` explicitly explains TLS requirement)
