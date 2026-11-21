#!/usr/bin/env bash
set -euo pipefail

# One-time Termux setup to enable `pkg install dog` from your GitHub Pages APT repo.
# Usage: bash packaging/termux/setup-apt.sh [github-user] [repo-name]

GH_USER="${1:-kaydankotze17-lang}"
GH_REPO="${2:-dog}"
PREFIX_DIR="${PREFIX:-/data/data/com.termux/files/usr}"
LIST_DIR="$PREFIX_DIR/etc/apt/sources.list.d"
REPO_URL="https://${GH_USER}.github.io/${GH_REPO}"

if [ ! -d "$PREFIX_DIR" ]; then
    echo "Warning: Termux PREFIX '$PREFIX_DIR' not found. Run this inside Android Termux." >&2
    exit 1
fi

mkdir -p "$LIST_DIR"
echo "deb [trusted=yes] ${REPO_URL} termux main" > "$LIST_DIR/dog.list"

echo "Added DOG Termux APT source: $REPO_URL"
pkg update -y
pkg install -y dog
dog --version
