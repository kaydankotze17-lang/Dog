# DOG Programming Language Cheat Sheet (v1.0.0)

Official quick-reference guide covering English, Classic, and Simple syntax dialects, strict types, structs, enums, pattern matching, lambdas, pipes, SQLite, cryptography, concurrency, DOG CSS (`.dgss`), `std.web`, diagnostics, and Termux CLI workflows.

Interactive web version: `/cheatsheet` (with live search, copy buttons, Markdown export, print layout, and one-click **Try in Playground** execution).

## English Style vs Classic Style vs Simple Style

| Concept | English Style | Classic Style | Simple Style |
| :--- | :--- | :--- | :--- |
| Immutable / mutable | `let x = 1` / `var y = 0` | `let x: number = 1` / `var y: number = 0` | `keep x = 1` / `make y = 0` |
| Reassignment | `set y to 5` / `increase y by 2` | `y = 5` / `y += 2` | `y = 5` / `bump y by 2` / `add 2 to y` |
| Functions | `define f(a: number) returns number` | `fn f(a: number) -> number` | `do f(a)` or `do f(a) = a * 2` |
| Conditionals | `if a is at least 10 then ... elif ... else ... end` | `if a >= 10 ... elif ... otherwise ... end` | `when a >= 10 ... or when ... else ... end` |
| Guarded / inverted checks | `unless locked then ... end` | `if not locked ... end` | `when not locked ... end` |
| Loops | `repeat 3 times` / `until done` / `for i from 1 to 5` | `while not done` / `for each x in list` | `loop 3` / `loop` / `each i in 1..6` |
| Comparisons | `is`, `is not`, `is over`, `is under`, `is at least` | `==`, `!=`, `>`, `<`, `>=` | `==`, `!=`, `>`, `<`, `>=`, `is` |
| Booleans | `and`, `or`, `not` (never `&&`, `\|\|`, `!`) | `and`, `or`, `not` | `and`, `or`, `not` |
| Output / Return | `print(v)` / `return v` | `say(v)` / `return v` | `show v` / `give v` |
| Error handling | `try ... catch err ... end` | `try ... catch err ... end` | `try ... oops err ... end` |
| Block termination | `end` | `end` | `end` (or one-liner `do f(x) = expr`) |

## Simple Style (Ultra-Short Syntax)

```dog
do double(n) = n * 2

make score = 10
bump score by 5
keep clean = "  simple dog  " |> trim |> upper

when score >= 15
    show $"{clean}: score={score}, x2={double(score)}"
else
    show "pending"
end

make sum = 0
each i in 1..5
    when i == 3
        skip
    end
    add i to sum
end
check sum == 7
```

## Operator Precedence

| Level | Operators | Meaning |
| :--- | :--- | :--- |
| 1 (Highest) | `.` `()` `[i]` `[a..b]` | Member access, calls, indexing, slices |
| 2 | `-` `not` | Unary negation & boolean NOT |
| 3 | `*` `/` `%` | Multiply, divide, modulo |
| 4 | `+` `-` | Add, concat, subtract |
| 5 | `==` `!=` `<` `<=` `>` `>=` `is` | Strict comparisons |
| 6 | `and` | Short-circuit boolean AND |
| 7 | `or` | Short-circuit boolean OR |
| 8 | `??` | Nullish fallback when left value is `null` |
| 9 (Lowest) | `\|>` | Pipeline left value into next call or lambda |

## Types, Structs, Enums & Pattern Matching

```dog
type Score = number

struct User
    name: text
    score: Score
end

enum Status
    Ready(number)
    Failed(text)
    Idle
end

var alice = User { name: "Alice", score: 95 }
set alice.score to 100

let state = Status.Ready(alice.score)
match state
    case Status.Ready(s) if s >= 90
        print($"Honors ready: {s}")
    case Status.Ready(s)
        print($"Ready: {s}")
    case Status.Failed(msg)
        print($"Error: {msg}")
    else
        print("Idle")
end
```

## Lambdas, Closures, Destructuring & Slices

```dog
fn make_adder(base: number) -> fn
    return fn(x) => x + base
end

let [head, second] = [10, 20, 30, 40]
let mid = [10, 20, 30, 40][1..3]
let add5 = make_adder(5)
let mapped = map(filter([3, 8, 12], fn(x) => x >= 8), add5)
print($"head={head}, second={second}, mid={join(mid, ",")}, mapped={join(mapped, ",")}")
```

## Standard Library Highlights (`std.data`, `std.crypto`, Concurrency)

- **SQLite**: `db_open(":memory:")`, `db_exec(db, sql, params)`, `db_query(db, sql, params)`, `db_insert`, `db_begin` / `db_commit` / `db_rollback`, `db_tables`, `db_schema`, `db_backup`, `query_builder`
- **Crypto**: `sha1`, `sha256`, `sha512`, `hmac_sha256`, `hmac_sha256_verify`, `secure_eq`, `pbkdf2_sha256`, `password_hash`, `password_verify`, `encrypt_aes256`, `decrypt_aes256`, `ed25519_keypair`, `ed25519_sign`, `ed25519_verify`, `uuid_v4`
- **Concurrency & HTTP**: `spawn_task`, `await_task`, `await_all`, `channel_create`, `channel_send`, `channel_recv`, `atomic_create`, `atomic_add`, `atomic_get`, `http_req`, `http_res`, `http_route_group`, `http_rate_limit_check`, `openapi_spec`

## DOG CSS (`.dgss`) & `std.web`

- **Shorthands**: `bg:`, `ink:`, `pad:`, `px:`, `py:`, `m:`, `mx:`, `my:`, `w:`, `h:`, `max-w:`, `radius:`, `size:`, `weight:`, `font:`, `leading:`, `cols:`, `flex: row|col`, `center: true`, `shadow: sm|md|lg`, `ease:`
- **Inheritance & Tree-Shaking**: `extend: .base-rule` inherits styles; `dog web build app.dg --minify --tree-shake` strips unused CSS selectors cleanly.
- **Reactive Web Helpers**: `web_component`, `web_render_component`, `web_state_set`, `web_state_get`, `web_on_state`, `dom_delegate`, `web_tabs`, `web_modal`, `web_form`, `web_field`, `web_validate_required`, `web_validate_email`

## Diagnostics & CLI

- **Errors**: `E001–E005` (Lex/Parse), `E010–E014` (Scope/Calls), `E021–E026` (Types/Return paths), `E030–E035` (Runtime), `E040–E041` (I/O & Cycles)
- **Warnings**: `W001` (Unreachable code after `return`/`break`/`continue`), `W002` (Constant-foldable / redundant check), `W003` (Unused variable; prefix `_` to silence)
- **Termux Update**: `curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash` or `dog update --check`
