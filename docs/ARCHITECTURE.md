# DOG Engine Architecture

## Pipeline

```text
Source (.dg)
  └─> Lexer (src/lexer)           -> Vec<Token> with Span (file, line, col)
       └─> Parser (src/parser)     -> Program AST (src/ast)
            └─> TypeChecker (src/checker + src/types) -> Verified Program
                 └─> Interpreter (src/interpreter + src/runtime + src/stdlib)
                      └─> Diagnostics (src/diagnostics + src/config)
```

## Module Responsibilities
- `ast`: AST nodes (`Program`, `Stmt`, `Expr`, `TypeRef`, `Param`, `Span`).
- `lexer`: Character-by-character UTF-8 scanner with file, line, and column spans.
- `parser`: Recursive-descent parser enforcing `end`-delimited blocks independent of indentation.
- `types`: Static `DogType` (`Text`, `Number`, `Decimal`, `Bool`, `Void`, `List`).
- `checker`: Two-pass function registration, scope & mutability checks, return-path verification, and strict type checking.
- `runtime`: `Value` representation and lexical `Environment` stack.
- `stdlib`: Native Rust standard library functions with `InterpreterIo` abstraction.
- `interpreter`: Tree-walking evaluator with `ExecFlow` unwinding and recursion depth limit.
- `formatter`: Canonical 4-space formatter (`dog format`).
- `tester`: `*_test.dg` test runner (`dog test`).
- `doctor`: Environment & Termux health inspector (`dog doctor`).
- `editor`: Zero-`unsafe`, zero-crate Nano-inspired terminal IDE (`dog edit`).
- `diagnostics`: Structured 4-part error formatting + personality modes.
- `config`: Persistent configuration (`~/.dog/config`).
- `cli`: Command router and stateful multi-line REPL with Kali-inspired `┌──(user⩫dog)-[path]` prompt.
