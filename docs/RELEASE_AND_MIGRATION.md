# DOG v1.0.0 Release Engineering, Migration & Reproducible Builds

Implements Part B & Part F release-engineering deliverables (`09515`, `09517`, `09526`–`09527`, `09535`–`09537`, `09544`–`09558`, `09563`).

## 1. v0.3.0 to v1.0.0 Migration Guide (`09527`)
1. **Strict String Concatenation**: Replace `"count: " + n` with string interpolation `$"count: {n}"` or explicit `text(n)`.
2. **Modular Stdlib Imports**: Prefer explicit `use "std.text"`, `use "std.crypto"`, `use "std.web"`, `use "std.data"` in reusable libraries and templates.
3. **Inline Web Styles & `.dgss`**: Replace manual `<style>` HTML concatenation with `style theme ... end`, `style ".card" ... end`, or `.dgss` stylesheets (`dog build app.dg --web`).
4. **Lint Strictness**: Run `dog lint --strict <file.dg>` and `dog format --check <file.dg>` in CI; eliminate `[L005]` unused variables and `[L006]` empty `catch` blocks.
5. **Updater Channel Refresh**: If upgrading from Termux `0.2.0-5` with APT cache errors, run:
   ```bash
   curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash
   ```
   From `v0.3.0+`, use `dog update --check` and `dog update`.

## 2. Reproducible Build Guide Per OS & Architecture (`09517`)
Because `Cargo.toml` uses **0 external Rust crates**, builds depend only on `rustc` / `cargo` (`>= 1.75`) and deterministic source state:
- **Linux x86_64 / aarch64**:
  ```bash
  SOURCE_DATE_EPOCH=$(git log -1 --format=%ct) cargo build --release --locked
  sha256sum target/release/dog > dog-linux-$(uname -m).sha256
  ```
- **Android Termux (AArch64 & ARMv7 32-bit)**:
  ```bash
  pkg install -y rust binutils
  cargo build --release --locked
  ./packaging/termux/build-deb.sh aarch64 1.0.0 target/release/dog dist/dog_1.0.0_aarch64.deb
  ```
- **macOS (`x86_64-apple-darwin` / `aarch64-apple-darwin`)**: `cargo build --release --locked`
- **Windows (`x86_64-pc-windows-msvc`)**: `cargo build --release --locked`

## 3. Minimal-Install & Offline Termux / Air-Gapped Profiles (`09535`–`09537`)
- **Constrained / Minimal Profile (`09535`)**: Single stripped `dog` binary (`~2–4 MB`), zero runtime shared libraries beyond `libc`, zero `target/` build cache required at runtime.
- **Offline Termux Docs Bundle (`09536`)**: Copy `docs/` + `examples/` + `templates/` directly into `$PREFIX/share/dog/`; query offline with `dog docs` and `dog learn`.
- **Air-Gapped Package Workflow (`09537`)**: Export verified `.dgb` bytecode bundle (`dog build app.dg --out app.dgb`) plus SHA-256 digest and install `.deb` directly via `dpkg -i dog_<ver>_<arch>.deb`.

## 4. Platform Troubleshooting Guide (`09558`)
- **`dog is already the newest version (0.2.0-5)`**: Caused by stale upstream APT index cache or CDN mirror delay. Run `apt-get update -o Acquire::http::No-Cache=true ...` or `curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash`.
- **Upstreams repo `404` during `apt update`**: Isolate updates exclusively to `sources.list.d/dog.list` using `-o Dir::Etc::sourceparts="-"`.
- **`curl | bash` gets `<!DOCTYPE html>`**: Wait ~2 minutes for GitHub Pages deployment to publish `update.sh` or download the `.deb` directly from GitHub Releases.

## 5. Regression Bisect, Redacted Bug Reports & Verification Workflows (`09544`–`09555`)
- **Git Bisect Helper (`09544`)**: `git bisect run sh -c "cargo test --quiet && node --test website/*.test.mjs"`
- **Secret-Redacted Support Diagnostics (`09545`–`09546`)**: `dog env` automatically masks `KEY`, `TOKEN`, `SECRET`, `PASS`, `AUTH`, and `CREDENTIAL` values before capturing output.
- **Clean-Room & Downgrade Safety (`09547`–`09549`)**: Project files live in workspace directories while toolchain binaries live in `$PREFIX/bin/dog` or `~/.cargo/bin/dog`; upgrades/rollbacks never touch project `.dg`/`.dgss` files.

## 6. Shared v1.0.0 Release Checklist (`09515`, `09526`, `09563`)
- [ ] Zero compiler warnings (`cargo clippy` / `cargo test --all-targets`)
- [x] All website & engine tests green (`npm test`, `npm run build`, `npm run lint`)
- [x] `/api/readiness` Doc Consistency check = `PASS`
- [x] Cross-syntax English vs Classic vs Simple 3-way parity suite (`parity_english.dg` vs `parity_classic.dg` vs `parity_simple.dg`) = `PASS`
- [x] SBOM & Apache-2.0 license report verified (`0` external Rust crates, `0` unsafe blocks)
- [ ] Termux `.deb` packages (`aarch64`, `arm`) built, installed in clean-room test, and `version.json` SHA-256 verified
