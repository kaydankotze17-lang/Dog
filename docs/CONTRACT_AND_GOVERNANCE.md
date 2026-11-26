# DOG v1.0.0 Compatibility Contract, Policies & Governance

Implements Part F deliverables (`09514`, `09516`, `09518`–`09525`) for DOG (`dog-lang`).

## 1. v1.0.0 Source & Standard Library Compatibility Contract (`09514`)
1. **Source Compatibility**: Valid DOG programs written against the stable grammar snapshot (`docs/SPECIFICATION.md`) compile and execute without breaking behavioral changes across all `1.x` minor releases.
2. **Dual-Syntax Equivalence**: English-style (`define ... returns`, `is at least`, `set x to y`, `increase x by n`, `unless`) and classic (`fn ... ->`, `>=`, `x = y`, `x += n`) forms lower to identical AST nodes and preserve output parity (`examples/parity_english.dg`, `examples/parity_classic.dg`).
3. **Strict Typing & Memory Safety Guarantee**: No implicit coercions (`text + number` always emits `E021`), immutable-by-default `let`/`const` (`E012`), and zero `unsafe` blocks in core Rust.
4. **Standard Library API Stability**: Signatures across all 12 `std.*` modules (`std.io`, `std.text`, `std.math`, `std.files`, `std.net`, `std.system`, `std.data`, `std.crypto`, `std.cli`, `std.collections`, `std.css`, `std.web`) remain backwards-compatible throughout `1.x`.

## 2. Support Policy (`09518`)
- **Current Feature Line (`v1.0.0` Stable Release)**: Active bug fixes, Termux APT packaging reliability, security updates, and regression tests.
- **v1.0.0 LTS Support Window**: Once tagged, `1.0.x` receives security and critical reliability fixes for at least 18 months or until `1.2.0` + 6 months overlap.
- **Tier-1 Targets**: Linux (`x86_64`, `aarch64`), Android Termux (`aarch64`, `arm` ARMv7), macOS (`x86_64`, `aarch64`), Windows (`x86_64`), Web Sandbox / DOM (`dog build --web`).

## 3. Security Response Policy (`09519`)
- **Reporting Channel**: Report vulnerabilities privately via GitHub Security Advisories (`kaydankotze17-lang/Dog/security/advisories`) or encrypted contact in `docs/SECURITY.md`.
- **SLA**: Acknowledge within 72 hours; triage severity (CVSS) within 7 days; coordinated patch + advisory release across source tarball, `.deb` Termux APT, and release manifest checksums.
- **Built-in Protections**: Constant-time digest/HMAC checks (`constant_time_eq`, `secure_eq`), zero-shell path handling, `dog env` secret redaction, SDU `/proc/self/status` kernel UID verification, SHA-256 `.dgb` bundle verification.

## 4. Package Naming & Namespace Policy (`09520`)
- Standard library namespaces reserve `std.*` (`std.io`, `std.net`, etc.) exclusively.
- Registry package names in `dog.pack` must match `^[a-z][a-z0-9-]{2,48}$`, avoid reserved typosquatting distances (`levenshtein_distance <= 1` from `std.*` or top core packages), and carry an explicit SPDX `license` and entry module.

## 5. Governance, RFC Workflow & Proposal Template (`09521`–`09523`)
1. **Proposal**: Author fills out the RFC template below and opens a PR under `docs/rfcs/`.
2. **Review**: Compiler, Stdlib, and Security reviewers verify grammar impact, error-code clarity, zero-external-crate impact, and Termux ARMv7 portability.
3. **Final Comment Period (FCP)**: Minimum 7 calendar days with passing `cargo test` + `npm test` + compatibility parity corpus.
4. **Acceptance & Tracking**: Approved RFC enters experimental flag before graduating to stable compat contract.

### RFC Proposal Template (`09522`)
```markdown
- RFC Title:
- Feature Area (Language / Compiler / Stdlib / CLI / Web / Updater):
- Motivation & Use Cases:
- Exact Syntax & Semantics (English + Classic):
- Error Codes & Diagnostics Impact:
- Backwards Compatibility & Migration Impact:
- Alternatives Considered:
- Test Plan (Golden, Malformed, Cross-Platform):
```

## 6. Stable-vs-Experimental Policy & Deprecation Window (`09524`–`09525`)
- **Stable**: Shipped by default, documented in `docs/SPECIFICATION.md` & `docs/STANDARD_LIBRARY.md`, protected by compatibility contract.
- **Experimental**: Documented as experimental / planning candidates in `DOG_v1.0.0_10150_Upgrade_Ideas.txt`; never added to the stability contract without passing gate tests.
- **Deprecation Window (`09525`)**: Any public stdlib/CLI deprecation must emit clear `[Dxxx]` / `[Lxxx]` warning diagnostics for at least **two minor releases** or **6 months** with an automated `dog upgrade` / migration path before removal.

## 7. Public Known-Issues Register (`09516`)
| ID | Severity | Component | Description & Workaround | Target |
| :--- | :--- | :--- | :--- | :--- |
| KI-001 | Medium | Termux APT Cache | Older Termux installs on v0.2.0 had cached `Release` metadata; run `curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh \| bash` or `dog update --force` (v0.3.0+ and v1.0.0 bypass cache cleanly). | Resolved in v1.0.0 |
| KI-002 | Low | Browser Sandbox | Browser playground emulates core + crypto + web + in-memory KV; real TCP/HTTP listeners & OS files require native `dog run`. | By Design (Sandbox Isolation) |
| KI-003 | Low | Concurrency | Multi-threaded async/await channels are still in v1.0 planning (`Part A 00036`); use `dog server` threaded worker loop or OS processes today. | v1.0.0 Milestone |
