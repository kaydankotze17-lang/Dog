# Contributing to DOG

## Development Principles
1. Preserve approved syntax (`let`, `var`, `const`, `say()`, `answer()`, `lower()`, `upper()`, `number()`, `decimal()`, `if`/`otherwise`/`end`, `or`, `==`).
2. Every feature requires unit tests, error-case tests, and documentation updates.
3. Never mark unimplemented features as `implemented`.
4. Keep external Rust crate dependencies minimal to preserve ARMv7 Android Termux support.

## Running Tests
```bash
cargo test
```
