# DOG Programming Language (`dog-lang`)

DOG is a strictly typed, readable, general-purpose programming language implemented in Rust with zero external crate dependencies.

## Implemented Features (v0.1.0)

Language engine (`dog-lang/src/`):
- Lexer & recursive-descent Parser with exact `file:line:col` spans
- Strict static Type Checker (`text`, `number`, `decimal`, `bool`, `void`, `list<T>`)
- Bindings: `let` (immutable), `var` (mutable), `const` (constant), optional `: type` annotations
- Control flow: `if` / `otherwise` / `end`, `while`, `repeat`, `for ... in ...`, `break`, `continue`
- Functions: `fn name(param: type) -> return_type ... end`, `return`, recursion
- Operators: `==`, `!=`, `>`, `<`, `>=`, `<=`, `and`, `or`, `not`, `+`, `-`, `*`, `/`, `%`, `[]` indexing
- Built-ins: `say()`, `answer()`, `lower()`, `upper()`, `trim()`, `contains()`, `split()`, `join()`, `len()`, `text()`, `number()`, `decimal()`, `push()`, `pop()`, `abs()`, `min()`, `max()`, `assert()`
- Diagnostics: Structured 4-part error messages (`What / WHY / FIX / TIP + DOG`) with configurable personality (`normal`, `sarcastic`, `chaotic`, `professional`, `silent`)
- Tooling & CLI:
  - `dog run <file.dg>`
  - `dog check <file.dg>`
  - `dog format [--check] <file.dg>`
  - `dog test [path]`
  - `dog doctor`
  - `dog edit [file.dg]` / `dog editor [file.dg]` (Nano-inspired terminal editor)
  - `dog repl` (stateful multi-line terminal with Kali-inspired `┌──(user⩫dog)-[path]` prompt)
  - `dog config personality [mode]`
  - `dog --version`, `dog --help`

Not yet implemented (tracked honestly in `docs/ROADMAP.md`):
- Native compiler (`dog build`)
- Package manager (`dog add`, DOG Hub)
- DOG Studio IDE
- SDU privilege subsystem (`dog sdu`)

## Building & Running (Requires Rust >= 1.70)

```bash
cargo test
cargo build --release
cargo run -- run examples/hello.dg
```

## Android / Termux (ARMv7 32-bit & AArch64)

```bash
pkg update && pkg install rust git
cargo test
cargo install --path .
dog run examples/hello.dg
dog edit examples/hello.dg
```
