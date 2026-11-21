# DOG Type System

DOG enforces **strict static typing** prior to execution via `src/checker`.

## Primitive Types
| Type | Representation | Example |
| :--- | :--- | :--- |
| `text` | UTF-8 `String` | `"DOG"` |
| `number` | `i64` | `42` |
| `decimal` | `f64` | `3.14` |
| `bool` | `bool` | `true`, `18 >= 18` |
| `void` | Unit | return of `say(...)` |

## Strict Rules
1. `text + number` is rejected at check-time with `E021` (Type mismatch).
2. `number + decimal` is rejected at check-time with `E021` unless explicitly converted via `decimal(n)` or `number(d)`.
3. `if <expr>` requires `<expr>` to have type `bool` (E022).
4. Reassigning a `let` or `const` binding is rejected with `E012` (Immutable reassignment).
5. Logical `or` requires `bool` on both sides (E023).
