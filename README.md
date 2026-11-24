# DOG Programming Language (`dog-lang`)

DOG is a strictly typed, readable, general-purpose programming language implemented in Rust with zero external crate dependencies and zero `unsafe` blocks, built to run natively on Linux, macOS, Windows, and Android Termux (ARMv7 & AArch64).

## Implemented Features (v0.3.0)

### Language Engine (`src/`)
- **Lexer & Recursive-Descent Parser**: Exact `file:line:col` spans, `"""..."""` multi-line triple-quoted strings, `0x`/`0b` integer literals, `_` numeric separators (`1_000_000`), and `end`-delimited blocks independent of indentation.
- **Strict Static Type Checker**: Primitives (`text`, `number`, `decimal`, `bool`, `void`), generics (`list<T>`, `map<K, V>`), user-defined `struct` and `enum`, default function parameters (`fn greet(name: text, prefix: text = "Hi") -> text`), return-path verification (`E026`), mutability enforcement (`E012`), and loop bounds checks (`E025`).
- **Bindings & Modules**: `let` (immutable), `var` (mutable), `const`, optional `: type` annotations, `$"..."` string interpolation, local module imports (`use "path.dg"`) with cycle detection (`E041`), and 10 modular standard library namespaces (`use "std.<mod>"` + direct `std.<mod>.<fn>(...)` calls across `std.io`, `std.text`, `std.math`, `std.files`, `std.net`, `std.system`, `std.data`, `std.crypto`, `std.cli`, `std.collections`).
- **Control Flow & Error Handling**: `if ... [then]` / `otherwise if` (`else if` / `elif`) / `otherwise` (`else`) / `end`, `unless ... [then] ... end`, `match` / `case` / `otherwise` (`else`) / `end`, `try` / `catch err` / `end`, `while`, `until`, `repeat N [times]`, `for [each] ... in ...`, `for [each] i from A to B`, `break`, `continue`, `fn` (`define` / `function`) with `->` or `returns`, and recursion (depth limit 256) with runtime `STACK TRACE:` capture.
- **Operators & English Comparisons**: `==`, `!=`, `>`, `<`, `>=`, `<=`, natural English comparisons (`is`, `is not`, `equals`, `is equal to`, `is not equal to`, `is greater than`, `is less than`, `is at least`, `is at most`, `is over`, `is under`), `and`, `or`, `not`, `+`, `-`, `*`, `/`, `%`, `+=`, `-=`, `*=`, `/=`, `%=`, English updates (`set x to v`, `increase x by n`, `decrease x by n`), `.` field/enum/module access, `[]` indexing.
- **Standard Library Built-ins & `std.*` Modules**:
  - Interactive I/O & CLI UI (`std.io`, `std.cli`): `say()`, `print()`, `ask()`, `answer()`, `confirm()`, `choose()`, `password()`, `read_key()`, `clear()` (0-arg terminal clear or 1-arg mutable list clear), `progress()`, `table()`, `log_info()`, `log_warn()`, `log_error()`, `cli_info()`, `cli_success()`, `cli_warn()`, `cli_error()`, `cli_debug()`, `cli_progress()`, `cli_spinner()`, `cli_theme()`, `cli_prefix()`, `cli_timestamps()`, `cli_supports_color()`, `cli_is_tty()`
  - Text & Casts (`std.text`): `lower()`, `upper()`, `trim()`, `title_case()`, `reverse_text()`, `contains()`, `starts_with()`, `ends_with()`, `replace()`, `slice()`, `char_at()`, `remove_prefix()`, `remove_suffix()`, `split()`, `join()`, `lines()`, `index_of()`, `last_index_of()`, `count_matches()`, `glob_match()`, `str_repeat()`, `pad_left()`, `pad_right()`, `html_escape()`, `len()`, `text()`, `number()`, `decimal()`
  - Math & RNG (`std.math`): `abs()`, `sign()`, `min()`, `max()`, `clamp()`, `round()`, `floor()`, `ceil()`, `sqrt()`, `pow()`, `pow_int()`, `sin()`, `cos()`, `tan()`, `atan2()`, `exp()`, `log()`, `log10()`, `gcd()`, `lcm()`, `math_pi()`, `math_e()`, `rand_int()`
  - Collections (`std.collections`): `push()`, `pop()`, `insert_at()`, `remove_at()`, `clear()`, `sort()`, `reverse()`, `list_contains()`, `list_index_of()`, `list_slice()`, `list_unique()`, `list_take()`, `list_skip()`, `list_flatten()`, `list_sum()`, `list_avg()`, `list_concat()`, `range()`, `map_get()`, `map_get_or()`, `map_set()`, `map_has()`, `map_keys()`, `map_values()`, `map_from_lists()`, `map_remove()`, `map_clear()`, `map_merge()`
  - Files, KV DB & System (`std.files`, `std.system`): `file_read()`, `file_write()`, `file_append()`, `file_exists()`, `file_delete()`, `file_copy()`, `file_move()`, `file_size()`, `is_file()`, `is_dir()`, `path_join()`, `path_dirname()`, `path_basename()`, `path_ext()`, `dir_list()`, `mkdir()`, `rmdir()`, `temp_dir()`, `cwd()`, `kv_set()`, `kv_get()`, `kv_has()`, `kv_delete()`, `kv_keys()`, `args()`, `env_get()`, `env_set()`, `env_load()`, `exec_cmd()`, `os_name()`, `os_arch()`, `uuid_v4()`, `time_now_ms()`, `time_utc()`, `time_format()`, `sleep_ms()`
  - JSON, TOML, CSV, URL, Routing & Crypto (`std.data`, `std.crypto`): `json_parse()`, `json_stringify()`, `json_pretty()`, `json_has_key()`, `json_read()`, `json_write()`, `toml_parse()`, `csv_parse()`, `csv_stringify()`, `url_encode()`, `url_decode()`, `url_parse()`, `query_parse()`, `query_stringify()`, `cookie_parse()`, `http_route_match()`, `hex_encode()`, `hex_decode()`, `base64_encode()`, `base64_decode()`, `base64_url_encode()`, `base64_url_decode()`, `sha1()`, `sha256()`, `sha512()`, `hmac_sha256()`, `hmac_sha256_verify()`, `rand_bytes_hex()`
  - Networking (`std.net` — Web, REST, WebSockets, Raw TCP & UDP): `http_listen()`, `http_port()`, `http_accept()`, `http_try_accept()`, `http_reply()`, `http_reply_headers()`, `http_redirect()`, `http_serve_file()`, `http_status_text()`, `http_close()`, `http_get()`, `http_post()`, `http_put()`, `http_delete()`, `http_request()`, `http_request_timeout()` (redirects, chunked bodies, IPv6, `http://` & `https://`), `ws_upgrade()`, `ws_connect()`, `ws_send()`, `ws_recv()`, `ws_ping()`, `ws_close()`, `tcp_listen()`, `tcp_port()`, `tcp_accept()`, `tcp_connect()`, `tcp_send()`, `tcp_recv()`, `tcp_set_timeout()`, `tcp_remote_addr()`, `tcp_close()`, `udp_bind()`, `udp_send_to()`, `udp_recv_from()`, `udp_close()`, `dns_lookup()`, `local_ip()`, `port_is_open()`
  - Errors: `assert()`, `fail()`
- **Diagnostics & Terminal Capabilities**: Structured 4-part error messages (`What / WHY / FIX / TIP + DOG`) with runtime call-stack traces (`STACK TRACE:`), Levenshtein `"Did you mean ...?"` hints, configurable personality, terminal themes (`default`, `kali`, `dracula`, `solar`, `mono`), timestamps/prefixes, and automatic capability fallback (`NO_COLOR`, `TERM=dumb`, `CLICOLOR_FORCE`, UTF-8/ASCII) — see [`docs/CLI_EXPERIENCE.md`](docs/CLI_EXPERIENCE.md).

### CLI, Compiler, Debugger, IDEs & Tooling
- `dog learn [lesson] [--run]` — Built-in interactive beginner lessons (`basics`, `english`, `input`, `functions`, `data`, `errors`) with live execution
- `dog run <file.dg | file.dgb> [-- args...]` / `dog eval "<expr | stmt>"` — Execute `.dg` scripts, `.dgb` bytecode bundles, or inline snippets (see `examples/english_basics.dg`, `examples/web_server.dg`, `examples/discord_bot.dg`, and `examples/cli_experience.dg`)
- `dog build <file.dg> [--out <path>] [--native]` / `dog package [file.dg] [out.dgb]` — Compile SHA-256 verified `.dgb` bundle (or native binary when `rustc` is on `PATH`)
- `dog debug <file.dg>` — Interactive step/breakpoint/variable-inspector debugger (`n`, `c`, `b <line>`, `p [var]`, `l`, `q`)
- `dog watch <file.dg> [--test | --check]` — Live file watcher monitoring the entrypoint and transitive `use` dependencies
- `dog coverage [path]` — Line-level execution coverage report across `*_test.dg` suites
- `dog graph [target]` — Dependency & `std.*` module import graph visualizer
- `dog explain <E-code | file.dg>` — Explain any `E001`–`E041` error code or summarize a `.dg` file's AST, imports, and capabilities
- `dog check <file.dg>` / `dog lint [--strict] <file.dg>` / `dog format [--check] <file.dg>` (alias: `fmt`) / `dog test [path]`
- `dog inspect <file.dg>` / `dog profile <file.dg>` / `dog bench <file.dg> [runs]` (alias: `benchmark`) / `dog docs [std | std.<mod> | topic | builtin]` / `dog forge new <name>` (alias: `new` / `init` / `innit`)
- `dog edit [file.dg]` / `dog studio [dir]` — Mobile-friendly 256-color editor and split-pane IDE with auto soft word-wrap on narrow screens (`Alt+W` / `:wrap`), live IntelliSense popup (`Ctrl+Space` / `Alt+/`), `std.*` namespace completion, dot field/enum completion, fuzzy matching, inline ghost completions, auto-pairing/auto-dedent, snippets, and command palette
- `dog prompt <preview|install|uninstall|status|init>` — Add the git-aware Kali-style `┌──(user⩫dog)-[path]` prompt and tab completions directly to your Termux/Linux shell
- `dog completions [bash|zsh|fish]` — Print shell completion script
- `dog repl` (alias: `playground`) — Stateful multi-line terminal with git-aware Kali-inspired `┌──(user⩫dog)-[path]` prompt
- `dog add <path|pkg|url>` / `dog remove <pkg> [--purge]` / `dog search` / `dog list` / `dog info` — DOG Hub (`dog.pack` with custom `[messages]`, real SHA-256 `dog.lock`, real progress bars)
- `dog server [dir] [port] [--host 0.0.0.0]` — Multi-threaded local/LAN static HTTP file server
- `dog sdu <status|auth|permissions|revoke|logs|doctor>` — Super Dog User scoped privilege & audit subsystem
- `dog doctor` / `dog system` / `dog status` / `dog env` / `dog process` / `dog permissions` / `dog logs` / `dog clean` / `dog update` (`upgrade`)
- `dog config <show|theme|timestamps|prefix|debug|install-style|personality>`

## Building & Running (Requires Rust >= 1.70)

```bash
cargo test
cargo build --release
cargo run -- run examples/hello.dg
```

## Android / Termux (`pkg install dog` & From Source)

### Instant Package Install (via GitHub Pages APT Repo)
Once you push this repo to `https://github.com/kaydankotze17-lang/Dog` and enable **Settings → Pages → Branch: `gh-pages`**, `.github/workflows/termux-apt.yml` automatically cross-compiles `dog` for `aarch64`, `arm` (ARMv7), and `x86_64` into `.deb` packages:

```bash
curl -fsSL https://raw.githubusercontent.com/kaydankotze17-lang/Dog/main/packaging/termux/setup-apt.sh | bash
dog run examples/hello.dg
```

Or add the APT line manually:
```bash
echo "deb [trusted=yes] https://kaydankotze17-lang.github.io/Dog termux main" > $PREFIX/etc/apt/sources.list.d/dog.list
pkg update && pkg install -y dog
```

See [`packaging/termux/README.md`](packaging/termux/README.md) for direct `.deb` releases and upstream `termux-packages` submission (`packaging/termux/build.sh`).

### Build From Source on Termux
```bash
pkg update && pkg install rust git
cargo test
cargo install --path .
dog run examples/hello.dg
dog studio .
```
