# DOG Security & Reliability Policy

## Core Engine Guarantees
1. **Zero `unsafe` Rust**: `dog-lang` v0.1.0 contains zero `unsafe` blocks.
2. **No Shell Injection**: `dog` never passes `.dg` file paths or arguments through `/bin/sh`.
3. **Strict Resource Safety**: Safe UTF-8 handling, bounded recursion/call depth checks, checked arithmetic / explicit division-by-zero diagnostics.
4. **Honest Privilege Reporting**: The DOG terminal prompt inspects actual OS identity (`EUID == 0` / `USER`) and displays `#` only when actual root privileges exist. It never spoofs root.

## SDU (Super Dog User) Architectural Principles (Planned)
- SDU will delegate exclusively to trusted OS authentication (`sudo`, `su`, or optional Android Shizuku IPC bridge).
- Passwords, tokens, and secrets are never written to audit logs or diagnostics.
- SDU is currently in `specified` status and is not exposed as a CLI command until audited.
