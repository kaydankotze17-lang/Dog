# DOG Programming Language (`dog-lang`)

DOG is a strictly typed, readable, general-purpose programming language implemented in Rust with zero external crate dependencies and zero `unsafe` blocks, built to run natively on Linux, macOS, Windows, and Android Termux (ARMv7 & AArch64).

## Implemented Features (v0.1.0)

### Language Engine (`src/`)
- **Lexer & Recursive-Descent Parser**: Exact `file:line:col` spans and `end`-delimited blocks independent of indentation.
- **Strict Static Type Checker**: Primitives (`text`, `number`, `decimal`, `bool`, `void`), generics (`list<T>`, `map<K, V>`), user-defined `struct` and `enum`, return-path verification (`E026`), mutability enforcement (`E012`), and loop bounds checks (`E025`).
- **Bindings & Modules**: `let` (immutable), `var` (mutable), `const`, optional `: type` annotations, and `use "path.dg"` local module imports with cycle detection (`E041`).
- **Control Flow & Error Handling**: `if` / `otherwise` / `end`, `match` / `case` / `otherwise` / `end`, `try` / `catch err` / `end`, `while`, `repeat`, `for ... in ...`, `break`, `continue`, `fn ... -> ... end`, and recursion (depth limit 256).
- **Operators**: `==`, `!=`, `>`, `<`, `>=`, `<=`, `and`, `or`, `not`, `+`, `-`, `*`, `/`, `%`, `.` field/enum access, `[]` indexing.
- **Standard Library Built-ins**:
  - I/O & Logging: `say()`, `answer()`, `log_info()`, `log_warn()`, `log_error()`
  - Text & Casts: `lower()`, `upper()`, `trim()`, `contains()`, `starts_with()`, `ends_with()`, `replace()`, `slice()`, `split()`, `join()`, `len()`, `text()`, `number()`, `decimal()`
  - Math & RNG: `abs()`, `min()`, `max()`, `round()`, `floor()`, `ceil()`, `sqrt()`, `pow()`, `rand_int()`
  - Collections: `push()`, `pop()`, `sort()`, `reverse()`, `list_contains()`, `range()`, `map_get()`, `map_set()`, `map_has()`, `map_keys()`, `map_remove()`
  - Files, CLI & System: `file_read()`, `file_write()`, `file_append()`, `file_exists()`, `file_delete()`, `dir_list()`, `mkdir()`, `cwd()`, `args()`, `env_get()`, `env_set()`, `time_now_ms()`, `sleep_ms()`
  - JSON, Crypto & HTTP: `json_parse()`, `json_stringify()`, `hex_encode()`, `hex_decode()`, `base64_encode()`, `base64_decode()`, `sha256()`, `hmac_sha256()`, `http_get()` (plain `http://`)
  - Errors: `assert()`, `fail()`
- **Diagnostics**: Structured 4-part error messages (`What / WHY / FIX / TIP + DOG`) with configurable personality (`normal`, `sarcastic`, `chaotic`, `professional`, `silent`).

### CLI, Compiler, IDEs & Tooling
- `dog run <file.dg | file.dgb> [-- args...]` — Execute `.dg` scripts or `.dgb` bytecode bundles
- `dog build <file.dg> [--out <path>] [--native]` — Compile SHA-256 verified `.dgb` bundle (or native binary when `rustc` is on `PATH`)
- `dog check <file.dg>` / `dog format [--check] <file.dg>` (alias: `fmt`) / `dog test [path]`
- `dog inspect <file.dg>` / `dog profile <file.dg>` / `dog docs [topic]` / `dog forge new <name>` (alias: `new`)
- `dog edit [file.dg]` / `dog studio [dir]` — Polished 256-color dark-slate editor and split-pane IDE with auto-pairing, `Tab` autocomplete, live signature tooltips, and command palette
- `dog prompt <preview|install|uninstall|status|init>` — Add the git-aware Kali-style `┌──(user⩫dog)-[path]` prompt and tab completions directly to your Termux/Linux shell
- `dog completions [bash|zsh|fish]` — Print shell completion script
- `dog repl` — Stateful multi-line terminal with git-aware Kali-inspired `┌──(user⩫dog)-[path]` prompt
- `dog add` / `dog remove` / `dog search` / `dog list` / `dog info` — Offline DOG Hub (`dog.pack`, `dog.lock`, `~/.dog/cache`)
- `dog server [dir] [port]` — Local static HTTP file server
- `dog sdu <status|auth|permissions|revoke|logs|doctor>` — Super Dog User scoped privilege & audit subsystem
- `dog doctor` / `dog system` / `dog status` / `dog env` / `dog process` / `dog permissions` / `dog logs` / `dog clean` / `dog update` (`upgrade`)
- `dog config personality <mode>`

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
curl -sSL https://raw.githubusercontent.com/kaydankotze17-lang/Dog/main/packaging/termux/setup-apt.sh | bash
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
