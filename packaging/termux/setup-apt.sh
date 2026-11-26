#!/usr/bin/env bash
set -euo pipefail

# Bulletproof Termux installer & cache-free updater for DOG (`dog`).
# Features:
#  - Rejects HTML 404/error pages (never feeds <!DOCTYPE html> to bash or dpkg)
#  - Repairs interrupted dpkg state (`dpkg --configure -a`) before upgrading
#  - Purges ONLY stale DOG APT list/archive caches (leaves other Termux repos untouched)
#  - Syncs ONLY dog.list with caching disabled (avoids slow/broken upstream mirrors)
#  - Validates `.deb` ar magic (`!<arch>`) + `dpkg-deb -I` + SHA-256 checksums
#  - Cascades: Isolated APT -> /latest/dog_latest_<arch>.deb -> pool/main/d/dog -> GitHub Releases API
#  - Backs up existing `dog` binary and rolls back automatically if install/verification fails
#
# Usage:
#   curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash
#   curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash -s -- --check
#   curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash -s -- --force

GH_USER="kaydankotze17-lang"
GH_REPO="Dog"
CHECK_ONLY=0
FORCE_INSTALL=0
FROM_SOURCE=0

print_usage() {
    cat <<'EOF'
DOG Termux Installer & Updater
Usage:
  update.sh [options] [gh_user] [gh_repo]

Options:
  -c, --check         Compare installed version vs Termux APT & GitHub main without modifying system
  -f, --force         Reinstall latest package even when installed version already matches
      --from-source   Build and install directly from source using Cargo & Git
      --repo U/R      Override GitHub repository (default: kaydankotze17-lang/Dog)
  -h, --help          Show this help message
EOF
}

POS_ARGS=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        -c|--check)
            CHECK_ONLY=1
            shift
            ;;
        -f|--force)
            FORCE_INSTALL=1
            shift
            ;;
        --from-source|--source)
            FROM_SOURCE=1
            shift
            ;;
        --repo)
            shift
            [ "$#" -gt 0 ] || { echo "Error: --repo requires <user/repo>." >&2; exit 2; }
            GH_USER="${1%%/*}"
            GH_REPO="${1#*/}"
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        --)
            shift
            while [ "$#" -gt 0 ]; do
                POS_ARGS+=("$1")
                shift
            done
            ;;
        -*)
            echo "Error: Unknown option '$1'. Run with --help for usage." >&2
            exit 2
            ;;
        *)
            POS_ARGS+=("$1")
            shift
            ;;
    esac
done

if [ "${#POS_ARGS[@]}" -ge 1 ] && [ -n "${POS_ARGS[0]}" ]; then
    GH_USER="${POS_ARGS[0]}"
fi
if [ "${#POS_ARGS[@]}" -ge 2 ] && [ -n "${POS_ARGS[1]}" ]; then
    GH_REPO="${POS_ARGS[1]}"
fi

PREFIX_DIR="${PREFIX:-/data/data/com.termux/files/usr}"
LIST_DIR="$PREFIX_DIR/etc/apt/sources.list.d"
REPO_URL="${DOG_REPO_URL:-https://${GH_USER}.github.io/${GH_REPO}}"
RAW_CARGO_URL="${DOG_RAW_CARGO_URL:-https://raw.githubusercontent.com/${GH_USER}/${GH_REPO}/main/Cargo.toml}"
GH_RELEASES_API="${DOG_GH_RELEASES_API:-https://api.github.com/repos/${GH_USER}/${GH_REPO}/releases/latest}"
WORK_TMP="${DOG_TMPDIR:-${TMPDIR:-$PREFIX_DIR/tmp}/dog-update-$$}"

BACKUP_BIN=""
BIN_TARGET="$PREFIX_DIR/bin/dog"

cleanup() {
    rm -rf "$WORK_TMP" 2>/dev/null || true
}
trap cleanup EXIT

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

semver_ge() {
    local a="${1#v}"
    local b="${2#v}"
    a="${a%%[-+]*}"
    b="${b%%[-+]*}"
    local a1 a2 a3 b1 b2 b3
    IFS='.' read -r a1 a2 a3 <<< "$a"
    IFS='.' read -r b1 b2 b3 <<< "$b"
    a1="${a1:-0}"; a2="${a2:-0}"; a3="${a3:-0}"
    b1="${b1:-0}"; b2="${b2:-0}"; b3="${b3:-0}"
    if [ "$a1" -ne "$b1" ] 2>/dev/null; then [ "$a1" -gt "$b1" ]; return $?; fi
    if [ "$a2" -ne "$b2" ] 2>/dev/null; then [ "$a2" -gt "$b2" ]; return $?; fi
    if [ "$a3" -ne "$b3" ] 2>/dev/null; then [ "$a3" -gt "$b3" ]; return $?; fi
    return 0
}

is_html_file() {
    local f="$1"
    [ -f "$f" ] || return 1
    local head_sample
    head_sample="$(head -c 512 "$f" 2>/dev/null | tr '[:upper:]' '[:lower:]' | tr -d '\000' || true)"
    case "$head_sample" in
        *"<!doctype html"*|*"<html"*|*"<title>404"*) return 0 ;;
        *) return 1 ;;
    esac
}

download_file() {
    local url="$1"
    local dest="$2"
    rm -f "$dest"
    if command -v curl >/dev/null 2>&1; then
        if curl --retry 3 --retry-delay 1 --connect-timeout 10 --max-time 60 \
            -fsSL -H "Cache-Control: no-cache" -H "Pragma: no-cache" \
            "$url" -o "$dest" 2>/dev/null; then
            if [ -s "$dest" ]; then
                return 0
            fi
        fi
    fi
    if command -v wget >/dev/null 2>&1; then
        if wget -q --timeout=30 --tries=3 --no-cache -O "$dest" "$url" 2>/dev/null; then
            if [ -s "$dest" ]; then
                return 0
            fi
        fi
    fi
    rm -f "$dest"
    return 1
}

fetch_text() {
    local url="$1"
    local out_file="$WORK_TMP/fetch_text.tmp"
    mkdir -p "$WORK_TMP"
    if download_file "$url" "$out_file"; then
        if is_html_file "$out_file"; then
            rm -f "$out_file"
            return 1
        fi
        cat "$out_file"
        rm -f "$out_file"
        return 0
    fi
    return 1
}

verify_deb_file() {
    local deb="$1"
    if [ ! -s "$deb" ]; then
        echo "  [!] Downloaded .deb is empty: $deb" >&2
        return 1
    fi
    if is_html_file "$deb"; then
        echo "  [!] Received HTML error page instead of a .deb package (404 / mirror error)." >&2
        return 1
    fi
    local magic=""
    magic="$(head -c 7 "$deb" 2>/dev/null | tr -d '\000' || true)"
    if [ "$magic" != "!<arch>" ]; then
        echo "  [!] Invalid .deb header (expected '!<arch>', got '$magic')." >&2
        return 1
    fi
    if command -v dpkg-deb >/dev/null 2>&1; then
        if ! dpkg-deb -I "$deb" >/dev/null 2>&1; then
            echo "  [!] Corrupted .deb archive rejected by dpkg-deb -I." >&2
            return 1
        fi
    fi
    return 0
}

compute_sha256() {
    local f="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$f" | awk '{print tolower($1)}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$f" | awk '{print tolower($1)}'
    else
        echo ""
    fi
}

verify_sha256_if_available() {
    local deb="$1"
    local expected="$2"
    [ -n "$expected" ] || return 0
    local actual=""
    actual="$(compute_sha256 "$deb")"
    [ -n "$actual" ] || return 0
    expected="$(echo "$expected" | tr '[:upper:]' '[:lower:]')"
    if [ "$actual" != "$expected" ]; then
        echo "  [!] SHA-256 verification failed for downloaded .deb!" >&2
        echo "      Expected: $expected" >&2
        echo "      Actual  : $actual" >&2
        return 1
    fi
    echo "  SHA-256 verified: ${actual:0:16}..."
    return 0
}

extract_arch_sha256() {
    local ver_json="$1"
    local sha_sums="$2"
    local arch="$3"
    local remote_deb_ver="$4"
    local found=""
    if [ -n "$ver_json" ]; then
        found="$(printf '%s' "$ver_json" | tr '\n' ' ' | sed -n "s/.*\"${arch}\"[[:space:]]*:[[:space:]]*\"\([0-9a-fA-F]\{64\}\)\".*/\1/p" | head -n1 || true)"
    fi
    if [ -z "$found" ] && [ -n "$sha_sums" ]; then
        found="$(printf '%s\n' "$sha_sums" | grep -E "dog_(latest|${remote_deb_ver})_${arch}\.deb" | awk '{print $1}' | head -n1 || true)"
    fi
    printf '%s' "$found"
}

repair_dpkg_and_purge_dog_cache() {
    if command -v dpkg >/dev/null 2>&1; then
        if [ -n "$(dpkg --audit 2>/dev/null || true)" ]; then
            echo "Repairing interrupted dpkg state..."
            dpkg --configure -a >/dev/null 2>&1 || true
        fi
    fi
    # Purge ONLY cached DOG APT indexes and .deb files; leave other Termux repos intact.
    rm -f "$PREFIX_DIR/var/lib/apt/lists/"*[Dd]og* \
          "$PREFIX_DIR/var/lib/apt/lists/partial/"*[Dd]og* \
          "$PREFIX_DIR/var/cache/apt/archives/"dog_*.deb 2>/dev/null || true
}

backup_existing_dog_binary() {
    local current_bin=""
    current_bin="$(command -v dog 2>/dev/null || true)"
    if [ -z "$current_bin" ] && [ -x "$BIN_TARGET" ]; then
        current_bin="$BIN_TARGET"
    fi
    if [ -n "$current_bin" ] && [ -f "$current_bin" ]; then
        mkdir -p "$WORK_TMP"
        BACKUP_BIN="$WORK_TMP/dog.backup"
        cp -p "$current_bin" "$BACKUP_BIN" 2>/dev/null || BACKUP_BIN=""
    fi
}

restore_backup_on_failure() {
    if [ -n "$BACKUP_BIN" ] && [ -f "$BACKUP_BIN" ]; then
        mkdir -p "$(dirname "$BIN_TARGET")"
        cp -p "$BACKUP_BIN" "$BIN_TARGET" 2>/dev/null || true
        chmod +x "$BIN_TARGET" 2>/dev/null || true
        echo "  [!] Rolled back to previous working dog binary at $BIN_TARGET." >&2
    fi
}

install_verified_deb() {
    local deb="$1"
    local expected_sha="$2"
    verify_deb_file "$deb" || return 1
    verify_sha256_if_available "$deb" "$expected_sha" || return 1

    backup_existing_dog_binary
    if ! dpkg -i "$deb"; then
        if command -v apt-get >/dev/null 2>&1; then
            apt-get install -f -y >/dev/null 2>&1 || true
        fi
        if ! dpkg -i "$deb"; then
            restore_backup_on_failure
            return 1
        fi
    fi

    if ! command -v dog >/dev/null 2>&1 || ! dog --version >/dev/null 2>&1; then
        echo "  [!] Installed dog binary failed post-install sanity check." >&2
        restore_backup_on_failure
        return 1
    fi
    return 0
}

upgrade_from_source() {
    echo "Building & installing DOG directly from source..."
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Error: Cargo/Rust is not installed. In Termux run: pkg install -y rust git" >&2
        exit 1
    fi
    if [ -f "Cargo.toml" ] && [ -d ".git" ]; then
        git pull --ff-only
        cargo install --path . --force
    else
        cargo install --git "https://github.com/${GH_USER}/${GH_REPO}.git" --force
    fi
    echo "Active DOG version: $(dog --version)"
    exit 0
}

if [ "$FROM_SOURCE" -eq 1 ]; then
    upgrade_from_source
fi

if [ ! -d "$PREFIX_DIR" ]; then
    echo "Error: Termux PREFIX '$PREFIX_DIR' not found. Run this inside Android Termux (or set PREFIX)." >&2
    exit 1
fi

mkdir -p "$WORK_TMP"
ARCH="$(detect_arch)"
EPOCH="$(date +%s 2>/dev/null || echo 0)"
CUR_VER="$(installed_version)"

echo "Checking DOG versions (${ARCH})..."
VER_JSON="$(fetch_text "${REPO_URL}/version.json?t=${EPOCH}" || true)"
SHA_SUMS="$(fetch_text "${REPO_URL}/SHA256SUMS?t=${EPOCH}" || true)"
REMOTE_VER=""
REMOTE_DEB_VER=""
if [ -n "$VER_JSON" ]; then
    REMOTE_VER="$(printf '%s' "$VER_JSON" | tr '\n' ' ' | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1 || true)"
    REMOTE_DEB_VER="$(printf '%s' "$VER_JSON" | tr '\n' ' ' | sed -n 's/.*"deb_version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1 || true)"
fi
CARGO_RAW="$(fetch_text "${RAW_CARGO_URL}?t=${EPOCH}" || true)"
MAIN_VER=""
if [ -n "$CARGO_RAW" ]; then
    MAIN_VER="$(printf '%s\n' "$CARGO_RAW" | sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1 || true)"
fi

echo "  Installed  : $CUR_VER"
[ -n "$REMOTE_VER" ] && echo "  Termux APT : $REMOTE_VER${REMOTE_DEB_VER:+ (deb $REMOTE_DEB_VER)}"
[ -n "$MAIN_VER" ] && echo "  GitHub main: $MAIN_VER"

TARGET_VER="${REMOTE_VER:-$MAIN_VER}"

if [ "$CHECK_ONLY" -eq 1 ]; then
    if [ "$CUR_VER" = "none" ]; then
        echo "Status: dog is not installed yet. Run without --check to install."
    elif [ -n "$TARGET_VER" ] && [ "$CUR_VER" != "$TARGET_VER" ] && semver_ge "$TARGET_VER" "$CUR_VER"; then
        echo "Status: Update available ($CUR_VER -> $TARGET_VER). Run without --check to upgrade."
    else
        echo "Status: Up to date ($CUR_VER). Pass --force to reinstall."
    fi
    exit 0
fi

if [ "$FORCE_INSTALL" -eq 0 ] && [ "$CUR_VER" != "none" ] && [ -n "$TARGET_VER" ] && [ "$CUR_VER" = "$TARGET_VER" ]; then
    echo "DOG is already up to date ($CUR_VER). Run with --force to reinstall."
    exit 0
fi

mkdir -p "$LIST_DIR"
echo "deb [trusted=yes] ${REPO_URL} termux main" > "$LIST_DIR/dog.list"

repair_dpkg_and_purge_dog_cache

# Channel 1: Isolated DOG APT (only dog.list, zero upstream Termux mirror exposure, No-Cache)
APT_OK=0
if command -v apt-get >/dev/null 2>&1; then
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
fi

AFTER_APT_VER="$(installed_version)"
NEED_DIRECT=0
if [ "$APT_OK" -eq 0 ] || [ "$AFTER_APT_VER" = "none" ]; then
    NEED_DIRECT=1
elif [ -n "$REMOTE_VER" ] && [ "$AFTER_APT_VER" != "$REMOTE_VER" ]; then
    NEED_DIRECT=1
fi

if [ "$NEED_DIRECT" -eq 1 ]; then
    echo "APT did not deliver ${REMOTE_VER:-latest}; switching to verified direct .deb installation..."
    EXPECTED_SHA="$(extract_arch_sha256 "$VER_JSON" "$SHA_SUMS" "$ARCH" "$REMOTE_DEB_VER")"
    TMP_DEB="$WORK_TMP/dog_candidate_${ARCH}.deb"
    DIRECT_OK=0

    # Channel 2: GitHub Pages /latest/dog_latest_<arch>.deb
    if download_file "${REPO_URL}/latest/dog_latest_${ARCH}.deb?t=${EPOCH}" "$TMP_DEB"; then
        if install_verified_deb "$TMP_DEB" "$EXPECTED_SHA"; then
            DIRECT_OK=1
        fi
    fi

    # Channel 3: Versioned APT pool .deb
    if [ "$DIRECT_OK" -eq 0 ] && [ -n "$REMOTE_DEB_VER" ]; then
        POOL_URL="${REPO_URL}/pool/main/d/dog/dog_${REMOTE_DEB_VER}_${ARCH}.deb?t=${EPOCH}"
        echo "Retrying versioned pool package (${REMOTE_DEB_VER})..."
        if download_file "$POOL_URL" "$TMP_DEB"; then
            if install_verified_deb "$TMP_DEB" "$EXPECTED_SHA"; then
                DIRECT_OK=1
            fi
        fi
    fi

    # Channel 4: GitHub Releases latest asset matching _<arch>.deb
    if [ "$DIRECT_OK" -eq 0 ]; then
        REL_JSON="$(fetch_text "${GH_RELEASES_API}?t=${EPOCH}" || true)"
        REL_URL="$(printf '%s' "$REL_JSON" | grep -oE "https://[^\"]+_${ARCH}\.deb" | head -n1 || true)"
        if [ -n "$REL_URL" ]; then
            echo "Retrying GitHub Releases asset ($REL_URL)..."
            if download_file "$REL_URL" "$TMP_DEB"; then
                if install_verified_deb "$TMP_DEB" ""; then
                    DIRECT_OK=1
                fi
            fi
        fi
    fi

    if [ "$DIRECT_OK" -eq 0 ]; then
        echo "Error: All Termux binary download channels failed or returned invalid packages." >&2
        echo "Try building directly from source instead:" >&2
        echo "  curl -fsSL ${REPO_URL}/update.sh | bash -s -- --from-source" >&2
        exit 1
    fi
fi

FINAL_VER="$(installed_version)"
echo "Active DOG version: $(dog --version)"
if [ -n "$MAIN_VER" ] && [ "$FINAL_VER" != "$MAIN_VER" ]; then
    echo "Note: GitHub main is v${MAIN_VER}; if CI just started publishing, wait ~2 minutes and re-run (or use --from-source)."
fi
