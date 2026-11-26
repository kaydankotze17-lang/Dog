# DOG Language Specification (v1.0.0 Grammar Snapshot)

## 1. Lexical Structure
- File extensions: `.dg` (source), `.dgss` (DOG Style Sheet), `.dgb` (SHA-256 verified bundle), `dog.pack` (package manifest)
- Encoding: UTF-8
- Comments: `# ...` or `// ...` until end of line
- Semicolons: Disallowed (`E001` with friendly hint — statements end on newlines)
- Indentation: 4 spaces per level by convention (`dog format` enforces 4 spaces). Indentation is visual only; `end` determines block boundaries.

## 2. Bindings, Type Aliases & Mutability
- `let <name>[: <type>] = <expr>`: Immutable binding (`E012` on reassignment).
- `var <name>[: <type>] = <expr>`: Mutable binding (reassignable with `=` or `set <name> to <expr>`, `increase`/`decrease`).
- `const <name>[: <type>] = <expr>`: Constant immutable binding (exported across `use` imports).
- `type <AliasName> = <TypeExpr>`: Named type alias (`type UserId = text`).

## 3. Data Types (Strict Typing)
- `text`: UTF-8 string `"..."` or interpolated `$"..."`
- `number`: 64-bit signed integer (`i64`)
- `decimal`: 64-bit IEEE 754 floating-point (`f64`)
- `bool`: `true` or `false`
- `list<T>`: Homogeneous list literal `[a, b, c]` (empty `[]` requires type annotation `: list<T>`)
- `map<K, V>`: Strictly typed map literal `{ "k": v }` (empty `{}` requires `: map<K, V>`)
- `struct Name ... end`: Named product type with typed fields (`Name { field: value }`)
- `enum Name ... end`: Named variant type (`Name.Variant`)
- `void`: Unit return type

DOG never performs implicit coercions between incompatible types. Use `text()`, `number()`, or `decimal()` explicitly (`E021` otherwise).

## 4. Operators & Natural English Aliases
- Equality: `==` (`is`, `equals`, `is equal to`), `!=` (`is not`, `is not equal to`)
- Comparison: `>` (`is greater than`, `is over`), `<` (`is less than`, `is under`), `>=` (`is at least`), `<=` (`is at most`)
- Logical: `and`, `or`, `not` (`!` / `&&` / `||` rejected with helpful hints)
- Arithmetic: `+`, `-`, `*`, `/`, `%` (`%` on `number`; `+` concatenates `text + text`)
- Access & Indexing: `obj.field`, `Enum.Variant`, `collection[index]`
- Mutation: `=`, `set x to v`, `increase x by n`, `decrease x by n`, `+=`, `-=`, `*=`, `/=`, `%=`

## 5. Formal Grammar Snapshot (EBNF Summary)
```ebnf
Program      := Statement*
Statement    := UseDecl | TypeAlias | EnumDecl | StructDecl | FnDecl | StyleBlock
              | Binding | Assign | IfStmt | UnlessStmt | MatchStmt | TryCatch
              | WhileStmt | UntilStmt | RepeatStmt | ForStmt | Return | Break | Continue | Expr
FnDecl       := ("fn" | "define" | "function") Ident "(" Params? ")" ("->" | "returns") Type Block "end"
StyleBlock   := "style" ("theme" | String) (Decl | ("on" StateOrBreakpoint Block "end"))* "end"
IfStmt       := "if" Expr "then"? Block (("elif" | "else if" | "otherwise if") Expr "then"? Block)* (("else" | "otherwise") Block)? "end"
ForStmt      := "for" "each"? Ident ("in" Expr | "from" Expr "to" Expr) Block "end"
```

## 6. Control Flow, Error Handling, Functions & Web Styles
```dog
use "std.web"

type UserId = text

enum Role
    Admin
    Member
end

struct User
    id: UserId
    role: Role
end

define greet(u: User) returns text
    match u.role
        case Role.Admin
            return $"Admin {u.id}"
        otherwise
            return $"Member {u.id}"
    end
end
```
`break` and `continue` are valid inside `while`, `until`, `repeat`, and `for`.

