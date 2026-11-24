#!/usr/bin/env bash
set -euo pipefail

# One-step Termux installer & cache-free updater for DOG (`dog`).
# Syncs ONLY the DOG APT list first (avoiding slow/broken upstream Termux mirrors)
# and falls back to downloading latest/dog_latest_<arch>.deb directly if APT is stale.
# Usage: curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash

GH_USER="${1:-kaydankotze17-lang}"
GH_REPO="${2:-Dog}"
PREFIX_DIR="${PREFIX:-/data/data/com.termux/files/usr}"
LIST_DIR="$PREFIX_DIR/etc/apt/sources.list.d"
REPO_URL="https://${GH_USER}.github.io/${GH_REPO}"
RAW_CARGO_URL="https://raw.githubusercontent.com/${GH_USER}/${GH_REPO}/main/Cargo.toml"

if [ ! -d "$PREFIX_DIR" ]; then
    echo "Warning: Termux PREFIX '$PREFIX_DIR' not found. Run this inside Android Termux." >&2
    exit 1
fi

detect_arch() {
    local arch=""
    if command -v dpkg >/dev/null 2>&1; then
        arch="$(dpkg --print-architecture 2>/dev/null || true)"
    fi
    if [ -z "$arch" ]; then
        arch="$(uname -m 2>/dev/null || echo aarch64)"
    fi
    case "$arch" in
        aarch64|arm64) echo "aarch64" ;;
        arm|armhf|armv7l|armv8l) echo "arm" ;;
        x86_64|amd64) echo "x86_64" ;;
        *) echo "aarch64" ;;
    esac
}

installed_version() {
    if command -v dog >/dev/null 2>&1; then
        dog --version 2>/dev/null | awk '{print $NF}'
    else
        echo "none"
    fi
}

ARCH="$(detect_arch)"
mkdir -p "$LIST_DIR"
echo "deb [trusted=yes] ${REPO_URL} termux main" > "$LIST_DIR/dog.list"

echo "Checking DOG versions (${ARCH})..."
REMOTE_VER="$(curl -fsSL "${REPO_URL}/version.json?t=$(date +%s)" 2>/dev/null | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1 || true)"
MAIN_VER="$(curl -fsSL "${RAW_CARGO_URL}?t=$(date +%s)" 2>/dev/null | sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1 || true)"
echo "  Installed  : $(installed_version)"
[ -n "$REMOTE_VER" ] && echo "  Termux APT : $REMOTE_VER"
[ -n "$MAIN_VER" ] && echo "  GitHub main: $MAIN_VER"

APT_OK=0
if apt-get update \
    -o Dir::Etc::sourcelist="sources.list.d/dog.list" \
    -o Dir::Etc::sourceparts="-" \
    -o APT::Get::List-Cleanup="0" \
    -o Acquire::http::No-Cache=true \
    -o Acquire::Https::No-Cache=true; then
    if apt-get install -y --reinstall --allow-downgrades \
        -o Acquire::http::No-Cache=true \
        -o Acquire::Https::No-Cache=true \
        dog; then
        APT_OK=1
    fi
fi

CUR_VER="$(installed_version)"
if [ "$APT_OK" -eq 0 ] || { [ -n "$REMOTE_VER" ] && [ "$CUR_VER" != "$REMOTE_VER" ]; }; then
    echo "Installing latest ${ARCH} .deb directly (bypassing APT mirror cache)..."
    TMP_DEB="${TMPDIR:-/data/data/com.termux/files/usr/tmp}/dog_latest_${ARCH}.deb"
    mkdir -p "$(dirname "$TMP_DEB")"
    if curl -fsSL "${REPO_URL}/latest/dog_latest_${ARCH}.deb?t=$(date +%s)" -o "$TMP_DEB"; then
        dpkg -i "$TMP_DEB" || apt-get install -f -y
        rm -f "$TMP_DEB"
    else
        echo "Direct .deb download failed. Verify GitHub Actions finished publishing to gh-pages." >&2
        exit 1
    fi
fi

FINAL_VER="$(installed_version)"
echo "Active DOG version: $(dog --version)"
if [ -n "$MAIN_VER" ] && [ "$FINAL_VER" != "$MAIN_VER" ]; then
    echo "Note: GitHub main is v${MAIN_VER}; if Actions just started building, wait ~2 minutes and re-run this script (or run: dog update --from-source)."
fi
