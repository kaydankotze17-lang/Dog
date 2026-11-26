# Termux Package & `dog update` Guide

This directory distributes `dog` as a Termux `.deb` package and isolated APT repository for `https://github.com/kaydankotze17-lang/Dog`.

## 1. Easiest Install & Bulletproof Upgrade (Recommended)

Run the standalone DOG updater script:

```bash
curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash
```

What the updater guarantees:
- **Zero broken Termux mirrors**: syncs only `sources.list.d/dog.list` with `No-Cache=true`.
- **No stale APT cache traps**: repairs interrupted `dpkg` state (`dpkg --configure -a`) and purges cached DOG APT indexes/archives before upgrading.
- **No `<!DOCTYPE html>` crashes**: rejects HTML 404 responses cleanly instead of passing them to `bash` or `dpkg`.
- **Integrity & rollback**: validates `!<arch>` headers, `dpkg-deb -I`, and SHA-256 checksums (`version.json` / `SHA256SUMS`), and backs up + rolls back your existing `dog` binary if installation fails.
- **Multi-channel fallback**: Isolated APT -> `/latest/dog_latest_<arch>.deb` -> `/pool/main/d/dog/...` -> GitHub Releases API -> `--from-source`.

Useful flags:

```bash
curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash -s -- --check
curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash -s -- --force
curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash -s -- --from-source
```

Once installed, upgrade anytime straight from DOG itself:

```bash
dog update                # check GitHub/APT and install newest version
dog update --check        # compare installed vs GitHub main & Termux APT
dog update --force        # reinstall latest package even if versions match
dog update --from-source  # pull & build from Cargo source
```

## 2. Manual APT Repo Setup

```bash
echo "deb [trusted=yes] https://kaydankotze17-lang.github.io/Dog termux main" > $PREFIX/etc/apt/sources.list.d/dog.list
rm -f $PREFIX/var/lib/apt/lists/*[Dd]og* $PREFIX/var/cache/apt/archives/dog_*.deb
apt-get update -o Dir::Etc::sourcelist="sources.list.d/dog.list" -o Dir::Etc::sourceparts="-" -o APT::Get::List-Cleanup="0" -o Acquire::http::No-Cache=true
apt-get install -y --reinstall --allow-downgrades dog
```

## 3. Direct `.deb` Install (No APT Repo Needed)

```bash
curl -fsSL https://kaydankotze17-lang.github.io/Dog/latest/dog_latest_aarch64.deb -o dog.deb
dpkg -i ./dog.deb
```
*(Replace `aarch64` with `arm` for 32-bit ARMv7 or `x86_64` for emulators/Chromebooks.)*

## 4. Built-In Official `pkg install dog` (Upstream Termux)

Once a version tag (`v1.0.0`) is pushed on GitHub:
1. Fork [`termux/termux-packages`](https://github.com/termux/termux-packages).
2. Copy `packaging/termux/build.sh` into `packages/dog/build.sh` and fill in `TERMUX_PKG_SHA256` for the `v1.0.0.tar.gz` release archive.
3. Open a pull request to `termux/termux-packages`.
