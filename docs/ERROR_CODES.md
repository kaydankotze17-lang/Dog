# DOG Error Codes

Every DOG diagnostic follows a 4-part structure:
1. **What happened** (Error code + summary + `file:line:col`)
2. **WHY** (Technical root cause)
3. **FIX** (Concrete action to resolve)
4. **TIP / DOG** (Helpful advice and optional personality remark; suppressed in `silent` mode)

## Catalog
- `E001`: Unexpected character in source
- `E002`: Unterminated string literal
- `E003`: Invalid numeric literal
- `E004`: Unexpected token / syntax error
- `E005`: Missing `end` keyword to close block
- `E010`: Undefined variable, struct/enum field, or enum variant
- `E011`: Duplicate declaration in scope, type collision, or built-in name conflict
- `E012`: Cannot reassign or mutate immutable `let` or `const` binding
- `E013`: Unknown function call
- `E014`: Incorrect argument count in function call
- `E021`: Type mismatch detected
- `E022`: Condition in `if` or `while` must evaluate to `bool`
- `E023`: Logical operator `and` / `or` requires `bool` operands
- `E024`: Cannot infer element/entry type of empty `[]` or `{}` without annotation
- `E025`: `return` outside function, or `break` / `continue` outside loop
- `E026`: Non-void function does not return a value on all control paths
- `E030`: Runtime conversion failure, negative `repeat` count, or invalid JSON/hex/Base64 payload
- `E031`: Division or modulo by zero
- `E032`: Maximum call depth (256) exceeded
- `E033`: Runtime assertion failed (`assert()`)
- `E034`: Index out of bounds, missing map key (`map_get()`), or `pop()` on empty list
- `E035`: Explicit runtime error raised via `fail()`
- `E040`: I/O, module read, or HTTP network error
- `E041`: Circular module import detected (`use "..."`)
