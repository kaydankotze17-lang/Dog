# Official termux-packages recipe for upstream Termux (`packages/dog/build.sh`).
# Submit this to https://github.com/termux/termux-packages once you tag a release.

TERMUX_PKG_HOMEPAGE=https://github.com/kaydankotze17-lang/Dog
TERMUX_PKG_DESCRIPTION="Strictly typed general-purpose programming language, Studio IDE, and CLI"
TERMUX_PKG_LICENSE="Apache-2.0"
TERMUX_PKG_MAINTAINER="@kaydankotze17-lang"
TERMUX_PKG_VERSION="0.1.0"
TERMUX_PKG_SRCURL="https://github.com/kaydankotze17-lang/Dog/archive/refs/tags/v${TERMUX_PKG_VERSION}.tar.gz"
TERMUX_PKG_SHA256="REPLACE_WITH_TARBALL_SHA256"
TERMUX_PKG_BUILD_IN_SRC=true

termux_step_make() {
	termux_setup_rust
	cargo build --jobs "$TERMUX_PKG_MAKE_PROCESSES" --target "$CARGO_TARGET_NAME" --release
}

termux_step_make_install() {
	install -Dm755 -t "$TERMUX_PREFIX/bin" "target/${CARGO_TARGET_NAME}/release/dog"
}
