# Termux Package & `dog update` Guide

This directory contains everything needed to distribute `dog` as a Termux `.deb` package and APT repository for `https://github.com/kaydankotze17-lang/Dog`.

## 1. Easiest Install & Zero-Error Upgrade (Recommended)

Run the standalone DOG updater (it syncs ONLY DOG's APT index with caching disabled so broken Termux mirrors never cause errors, and falls back directly to `latest/dog_latest_<arch>.deb` if APT serves a stale version):

```bash
curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash
```

Once installed, upgrade anytime straight from DOG itself:

```bash
dog update          # check GitHub/APT and install newest version
dog update --check  # compare installed vs GitHub main & Termux APT
dog update --force  # reinstall latest package even if versions match
```

## 2. Manual APT Repo Setup

```bash
echo "deb [trusted=yes] https://kaydankotze17-lang.github.io/Dog termux main" > $PREFIX/etc/apt/sources.list.d/dog.list
apt-get update -o Dir::Etc::sourcelist="sources.list.d/dog.list" -o Dir::Etc::sourceparts="-" -o APT::Get::List-Cleanup="0" -o Acquire::http::No-Cache=true
apt-get install -y --reinstall dog
```

## 3. Direct `.deb` Install (No APT Repo Needed)

```bash
curl -fsSL https://kaydankotze17-lang.github.io/Dog/latest/dog_latest_aarch64.deb -o dog.deb
dpkg -i ./dog.deb
```
*(Replace `aarch64` with `arm` for 32-bit ARMv7 or `x86_64` for emulators/Chromebooks.)*

## 4. Built-In Official `pkg install dog` (Upstream Termux)

Once a version tag (`v0.3.0`) is pushed on GitHub:
1. Fork [`termux/termux-packages`](https://github.com/termux/termux-packages).
2. Copy `packaging/termux/build.sh` into `packages/dog/build.sh` and fill in `TERMUX_PKG_SHA256` for the `v0.3.0.tar.gz` release archive.
3. Open a pull request to `termux/termux-packages`.
