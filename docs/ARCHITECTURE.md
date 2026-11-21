# DOG Engine Architecture

## Pipeline

```text
Source (.dg)
  └─> Lexer (src/lexer)            -> Vec<Token> with Span (file, line, col)
       └─> Parser (src/parser)      -> Program AST (src/ast)
            └─> Module Resolver (src/interpreter::resolve_program)
                 └─> TypeChecker (src/checker + src/types) -> Verified Program
                      ├─> Interpreter (src/interpreter + src/runtime + src/stdlib)
                      └─> Bundle/Native Compiler (src/bytecode) -> .dgb or rustc binary
```

## Module Responsibilities
- `ast`: AST nodes (`Program`, `Stmt`, `Expr`, `TypeRef`, `Param`, `StructField`, `MatchCase`, `Span`).
- `lexer`: Character-by-character UTF-8 scanner with file, line, and column spans.
- `parser`: Recursive-descent parser enforcing `end`-delimited blocks independent of indentation.
- `types`: Static `DogType` (`Text`, `Number`, `Decimal`, `Bool`, `Void`, `List`, `Map`, `Struct`, `Enum`).
- `checker`: Two-pass type/struct/enum/function registration, mutability checks, return-path verification, and symbol listing.
- `runtime`: `Value` representation and lexical `Environment` stack.
- `stdlib`: Native Rust standard library functions with `InterpreterIo` abstraction.
- `crypto`: Pure-Rust FIPS 180-4 SHA-256, Hex, and Base64 codecs (zero external crates).
- `json`: Pure-Rust recursive-descent JSON parser and serializer.
- `interpreter`: Tree-walking evaluator with `ExecFlow` unwinding, `try/catch`, module cycle detection, and recursion depth limit.
- `bytecode`: `.dgb` bundle builder/verifier (`DOG0` header + SHA-256 checksum) and `rustc`-backed `--native` transpiler.
- `formatter`: Canonical 4-space formatter (`dog format`).
- `tester`: `*_test.dg` test runner (`dog test`).
- `doctor`: Environment, `rustc`, Termux, and config inspector (`dog doctor`).
- `editor`: Zero-`unsafe`, zero-crate Nano-inspired terminal editor (`dog edit`).
- `studio`: Split-pane DOG Studio terminal IDE (`dog studio`) with Project Explorer, Editor, Symbol Inspector, and Command Palette.
- `hub`: Offline DOG Hub package manifest (`dog.pack`), lockfile (`dog.lock`), and local cache manager (`~/.dog/cache`).
- `sdu`: Super Dog User subsystem with `/proc/self/status` root verification, scoped timed sessions, and audit logs.
- `sys`: Developer & system tools (`inspect`, `profile`, `docs`, `forge`, `server`, `system`, `status`, `env`, `process`, `permissions`, `update`, `clean`, `logs`).
- `diagnostics`: Structured 4-part error formatting + personality tips/facts/quotes/roasts.
- `config`: Persistent configuration (`~/.dog/config`).
- `cli`: Command router and stateful multi-line REPL with Kali-inspired `┌──(user⩫dog)-[path]` prompt.
