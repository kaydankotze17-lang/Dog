# DOG Standard Library (v0.1.0)

## Core I/O & Assertions
- `say(value) -> void`: Prints value to stdout followed by newline.
- `answer(prompt?: text) -> text`: Prints optional prompt and reads one line from stdin.
- `assert(cond: bool, msg?: text) -> void`: Fails with `E033` if `cond` is `false`.

## Explicit Type Conversions
- `text(value) -> text`: Explicit string conversion for any non-void value.
- `number(value: text | number | decimal) -> number`: Strict integer conversion (`i64`).
- `decimal(value: text | number | decimal) -> decimal`: Strict floating-point conversion (`f64`).

## Text Helpers
- `lower(t: text) -> text`
- `upper(t: text) -> text`
- `trim(t: text) -> text`
- `contains(haystack: text, needle: text) -> bool`
- `split(t: text, sep: text) -> list<text>`
- `join(items: list<text>, sep: text) -> text`
- `len(v: text | list<T>) -> number`

## List Operations (Requires `var` binding for mutation)
- `push(list_var: var list<T>, item: T) -> void`
- `pop(list_var: var list<T>) -> T`

## Math Helpers
- `abs(n: number | decimal) -> number | decimal`
- `min(a: T, b: T) -> T` (for `number` or `decimal`)
- `max(a: T, b: T) -> T` (for `number` or `decimal`)
