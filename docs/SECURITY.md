# DOG Security & Reliability Policy

## Core Engine Guarantees
1. **Zero `unsafe` Rust**: `dog-lang` v0.1.0 contains zero `unsafe` blocks.
2. **No Shell Injection**: `dog` never passes `.dg` file paths, module paths, or expressions through `/bin/sh`.
3. **Strict Resource Safety**: Safe UTF-8 handling, bounded recursion/call depth checks (`MAX_CALL_DEPTH = 256`), explicit division/modulo-by-zero diagnostics (`E031`), and safe path-traversal checks (`..` blocked) in `dog server`.
4. **Honest Privilege Reporting**: Both the DOG REPL prompt and `dog sdu` inspect `/proc/self/status` `Uid:` to verify actual kernel `EUID == 0` and display `#` or root status only when actual root privileges exist.
5. **Sensitive Data Masking**: `dog env` masks environment variables matching `KEY`, `TOKEN`, `SECRET`, `PASS`, `AUTH`, or `CREDENTIAL`, and `dog sdu` never logs passwords or tokens to `~/.dog/logs/sdu_audit.log`.

## SDU (Super Dog User) Architectural Model
- SDU validates actual kernel `EUID == 0` and checks host binaries (`sudo`, `su`, Android Shizuku `rish`) on `PATH`.
- `dog sdu auth --confirm` creates a time-bounded scoped session at `~/.dog/sdu_session` (max 3600s) and appends an entry to `~/.dog/logs/sdu_audit.log`.
- `dog sdu revoke` invalidates the scoped session immediately.
- SDU never fabricates OS root access; real kernel privilege elevation always delegates to host OS authorization (`sudo`, `tsu`, or Shizuku).
