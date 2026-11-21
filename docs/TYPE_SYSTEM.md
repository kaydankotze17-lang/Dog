# DOG Type System

DOG enforces **strict static typing** prior to execution via `src/checker`.

## Primitive & Composite Types
| Type | Representation | Example |
| :--- | :--- | :--- |
| `text` | UTF-8 `String` | `"DOG"` |
| `number` | `i64` | `42` |
| `decimal` | `f64` | `3.14` |
| `bool` | `bool` | `true`, `18 >= 18` |
| `list<T>` | `Vec<Value>` | `[1, 2, 3]` |
| `map<K, V>` | `Vec<(Value, Value)>` | `{"host": "127.0.0.1"}` |
| `struct Name` | Named record | `User { name: "Ada" }` |
| `enum Name` | Tagged variant | `Role.Admin` |
| `void` | Unit | return of `say(...)` |

## Strict Rules
1. `text + number` is rejected at check-time with `E021` (Type mismatch).
2. `number + decimal` is rejected at check-time with `E021` unless explicitly converted via `decimal(n)` or `number(d)`.
3. `if <expr>` and `while <expr>` require `<expr>` to have type `bool` (`E022`).
4. Empty `[]` or `{}` literals require a contextual type annotation (`: list<T>` or `: map<K, V>`) (`E024`).
5. Reassigning a `let` or `const` binding (or mutating its fields/elements/list/map contents) is rejected with `E012`.
6. Logical `and` / `or` / `not` require `bool` operands (`E023`).
7. Non-`void` functions must return a matching value along every execution path (`E026`).
