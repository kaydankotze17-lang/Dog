#!/usr/bin/env bash
set -euo pipefail

# Build a Termux-compatible .deb package for DOG.
# Usage: ./packaging/termux/build-deb.sh <arch: aarch64|arm|x86_64> <version> <binary-path> <out-deb>

if [ "$#" -ne 4 ]; then
    echo "Usage: $0 <arch: aarch64|arm|x86_64> <version> <binary-path> <out-deb>" >&2
    exit 2
fi

ARCH="$1"
VERSION="$2"
BIN_PATH="$3"
OUT_DEB="$4"

if [ ! -f "$BIN_PATH" ]; then
    echo "Binary not found: $BIN_PATH" >&2
    exit 1
fi

STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGE_DIR"' EXIT

TERMUX_USR_DIR="$STAGE_DIR/data/data/com.termux/files/usr"
TERMUX_BIN_DIR="$TERMUX_USR_DIR/bin"
TERMUX_EX_DIR="$TERMUX_USR_DIR/share/dog/examples"
mkdir -p "$STAGE_DIR/DEBIAN" "$TERMUX_BIN_DIR" "$TERMUX_EX_DIR"

cp "$BIN_PATH" "$TERMUX_BIN_DIR/dog"
chmod 0755 "$TERMUX_BIN_DIR/dog"
if [ -d "examples" ]; then
    cp examples/*.dg examples/*.dgss "$TERMUX_EX_DIR/" 2>/dev/null || true
    chmod 0644 "$TERMUX_EX_DIR"/*.dg "$TERMUX_EX_DIR"/*.dgss 2>/dev/null || true
fi

INSTALLED_KB="$(du -sk "$STAGE_DIR/data" | awk '{print $1}')"

cat > "$STAGE_DIR/DEBIAN/control" <<EOF
Package: dog
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: kaydankotze17-lang <https://github.com/kaydankotze17-lang/Dog>
Installed-Size: ${INSTALLED_KB}
Section: devel
Priority: optional
Homepage: https://github.com/kaydankotze17-lang/Dog
Description: Strictly typed general-purpose programming language, Studio IDE, and CLI
 DOG is a zero-dependency Rust programming language featuring strict static typing,
 4-part structured diagnostics, a terminal Studio IDE, .dgb bytecode bundling,
 and Termux-native tooling.
EOF

mkdir -p "$(dirname "$OUT_DEB")"
dpkg-deb --build --root-owner-group "$STAGE_DIR" "$OUT_DEB"
echo "Built Termux package: $OUT_DEB"
