# DOG Language Specification (v0.1.0)

## 1. Lexical Structure
- File extensions: `.dg` (source), `.dgb` (SHA-256 verified bundle)
- Encoding: UTF-8
- Comments: `# ...` or `// ...` until end of line
- Semicolons: Not required
- Indentation: 4 spaces per level by convention (`dog format` enforces 4 spaces). Indentation is visual only; `end` determines block boundaries.

## 2. Bindings & Mutability
- `let <name>[: <type>] = <expr>`: Immutable binding.
- `var <name>[: <type>] = <expr>`: Mutable binding (reassignable with `=`, mutable list/map/struct updates).
- `const <name>[: <type>] = <expr>`: Constant immutable binding (also exported across `use` imports).

## 3. Data Types (Strict Typing)
- `text`: UTF-8 string `"..."`
- `number`: 64-bit signed integer (`i64`)
- `decimal`: 64-bit IEEE 754 floating-point (`f64`)
- `bool`: `true` or `false`
- `list<T>`: Homogeneous list literal `[a, b, c]` (empty `[]` requires type annotation `: list<T>`)
- `map<K, V>`: Strictly typed map literal `{ "k": v }` (empty `{}` requires `: map<K, V>`)
- `struct Name ... end`: Named product type with typed fields (`Name { field: value }`)
- `enum Name ... end`: Named variant type (`Name.Variant`)
- `void`: Unit return type

DOG never performs implicit coercions between incompatible types. Use `text()`, `number()`, or `decimal()` explicitly.

## 4. Operators
- Comparison: `==`, `!=`, `>`, `<`, `>=`, `<=`
- Logical: `and`, `or`, `not`
- Arithmetic: `+`, `-`, `*`, `/`, `%` (`%` on `number`; `+` also concatenates `text + text`)
- Access & Indexing: `obj.field`, `Enum.Variant`, `collection[index]`
- Assignment: `=`, `obj.field = val`, `items[i] = val`

## 5. Control Flow, Error Handling, Functions & Modules
```dog
use "helpers.dg"

enum Role
    Admin
    Member
end

struct User
    name: text
    role: Role
end

if <bool_expr>
    ...
otherwise
    ...
end

match user.role
    case Role.Admin
        ...
    otherwise
        ...
end

try
    ...
catch err
    say(err)
end

while <bool_expr>
    ...
end

repeat <number_expr>
    ...
end

for <item> in <list_expr>
    ...
end

fn <name>(<param>: <type>, ...) -> <return_type>
    return <expr>
end
```
`break` and `continue` are valid inside `while`, `repeat`, and `for`.
