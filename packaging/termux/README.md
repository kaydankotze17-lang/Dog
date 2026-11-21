# Termux Package & `pkg install dog` Guide

This directory contains everything needed to distribute `dog` as a real Termux `.deb` package and APT repository for `https://github.com/kaydankotze17-lang/Dog`.

## 1. Enable `pkg install dog` via GitHub Pages APT Repo

1. Push this repository to `https://github.com/kaydankotze17-lang/Dog` (`main` branch).
2. Wait for the **Build Termux APT Repository & Release** GitHub Action (`.github/workflows/termux-apt.yml`) to finish — it cross-compiles `dog` for `aarch64`, `arm` (ARMv7), and `x86_64`, generates `.deb` archives, and pushes an APT repository to the `gh-pages` branch.
3. In GitHub repo **Settings → Pages → Source: `Deploy from a branch` → Branch: `gh-pages` (`/root`)**.
4. In Termux on Android, register your APT repo once and install `dog`:
   ```bash
   echo "deb [trusted=yes] https://kaydankotze17-lang.github.io/Dog termux main" > $PREFIX/etc/apt/sources.list.d/dog.list
   pkg update && pkg install -y dog
   ```
   Or run the one-line helper script:
   ```bash
   curl -sSL https://raw.githubusercontent.com/kaydankotze17-lang/Dog/main/packaging/termux/setup-apt.sh | bash
   ```
   From then on, `pkg upgrade` automatically updates `dog` whenever you push to `main`!

## 2. Direct `.deb` Install (No APT Repo Needed)

Push a version tag (`git tag v0.1.0 && git push origin v0.1.0`). GitHub Actions attaches `dog_0.1.0_aarch64.deb`, `dog_0.1.0_arm.deb`, and `dog_0.1.0_x86_64.deb` to the Release:
```bash
curl -LO https://github.com/kaydankotze17-lang/Dog/releases/latest/download/dog_0.1.0_aarch64.deb
pkg install ./dog_0.1.0_aarch64.deb
```

## 3. Built-In Official `pkg install dog` (Upstream Termux)

Once you have a tagged release on GitHub:
1. Fork [`termux/termux-packages`](https://github.com/termux/termux-packages).
2. Copy `packaging/termux/build.sh` into `packages/dog/build.sh` and fill in `TERMUX_PKG_SHA256` for the `v0.1.0.tar.gz` release archive.
3. Open a pull request to `termux/termux-packages`. Once merged, every Termux user can run `pkg install dog` directly with zero extra setup.
