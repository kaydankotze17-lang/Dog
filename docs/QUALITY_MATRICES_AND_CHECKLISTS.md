# DOG v1.0.0 Engineering Quality Matrices, Acceptance Gates & Checklists

Implements Part E & Part G systematic validation, failure-mode matrices, security review checklists, maintenance checklists, and performance baselines across DOG's subsystems (`09564`–`10150`).

## 1. Subsystem Failure-Mode Matrix
| Subsystem | Failure Mode | Deterministic Diagnostic / Mitigation | Verification Method |
| :--- | :--- | :--- | :--- |
| Lexer | Unterminated string / `$"` / stray `! & \| ;` | Emit `E001`/`E002` with exact line/column & English fix hint | Golden malformed-token tests |
| Parser | Missing `end`, malformed `for`/`if`/`style` | Emit `E004`/`E005` cleanly without panics; bounded recursion | Malformed block boundary tests |
| Type Checker | Implicit `text + number`, immutable write | Reject statically with `E021` (type) / `E012` (mutability) | Strict type-mismatch corpus |
| Interpreter / Sandbox | Division by zero, runaway loop, OOB index | Catch cleanly (`E031`, `E035`, `E032`); recoverable via `try/catch` | Step-limit & edge-case tests |
| Stdlib Crypto & Data | Non-constant-time compares, invalid JSON/CSV | Enforce `constant_time_eq`/`secure_eq`; validated JSON schema checks | SHA-256 / HMAC / PBKDF2 vectors |
| DOG Web & `.dgss` | XSS injection in HTML, unclosed CSS block | Escape via `__dogEscape` in builders; catch `E005` on unclosed rules | Headless DOM & CSS tests |
| Networking (`std.net`) | Path traversal `..`, malformed HTTP request | Normalize paths, block `..` escape outside root, bound body read | HTTP route & traversal tests |
| Updater & Packaging | Stale APT cache, partial download, bad arch | `No-Cache=true`, direct `.deb` fallback, `dpkg --print-architecture` check | Clean-room Termux install check |

## 2. Release-Blocking Acceptance Gates
1. **Language & Dual-Syntax Parity**: `examples/parity_english.dg` and `examples/parity_classic.dg` must evaluate to byte-identical output and declaration summaries.
2. **Type Safety**: `text + number`, `let` reassignment, and division by zero must deterministically return `E021`, `E012`, and `E031` across both native Rust and browser sandbox.
3. **Stdlib Showcase**: `examples/v1_showcase.dg` must pass end-to-end assertions across `std.text`, `std.math`, `std.collections`, `std.crypto`, `std.data`, `std.net`, and `std.files`.
4. **Web Compilation**: `.dg` + `.dgss` must bundle cleanly with variable resolution, vendor prefixing, responsive `@media`, and zero inline XSS escape leaks.
5. **Documentation Sync**: `/api/readiness` verifies `docs/STANDARD_LIBRARY.md`, `src/stdlib/modules.rs`, and `website/data.js` cover all 12 `std.*` modules cleanly.

## 3. Security Review Checklist (Per Release)
- [ ] Verify `unsafe` block count across `src/**/*.rs` remains strictly `0`.
- [ ] Verify zero `/bin/sh -c` invocation on untrusted `.dg` paths or filenames.
- [ ] Verify `dog env` masks `KEY`, `TOKEN`, `SECRET`, `PASS`, `AUTH`, `CREDENTIAL`.
- [ ] Verify HTML builders escape attributes and text content cleanly (`<`, `>`, `&`, `"`, `'`).
- [ ] Verify cryptographic primitives use constant-time equality (`hmac_sha256_verify`, `secure_eq`).
- [ ] Verify Termux `.deb` & `.dgb` artifacts include valid SHA-256 checksums.

## 4. Maintenance & Operations Checklist (Per Release)
- [ ] Synchronize version string across `Cargo.toml`, `server.mjs`, `website/stats_engine.mjs`, `packaging/termux/`.
- [ ] Run full Rust suite (`cargo test --all-targets`) on host Linux/macOS/Termux target.
- [ ] Run full Web & Readiness suite (`npm test && npm run build && npm run lint`).
- [ ] Inspect live `/api/stats`, `/api/readiness`, and `/api/roadmap` JSON responses.
- [ ] Verify all 5 official project templates under `templates/*` pass syntax checks and tests.

## 5. Performance Baseline Targets
| Workload | Target Budget | Measurement Tool |
| :--- | :--- | :--- |
| Lex + Parse + Typecheck (1,000-line `.dg`) | `< 15 ms` native / `< 40 ms` JS sandbox | `dog bench` / Node `perf_hooks` |
| `.dgss` + `.dg` Web Bundle Compile | `< 25 ms` per preset | `compileDogWebsite()` |
| Query across 10,150-item Roadmap Dataset | `< 10 ms` per filtered query | `/api/roadmap` |
| Cold CLI startup (`dog --version`) | `< 5 ms` | Native release binary |
