#!/bin/sh
set -eu

repository_url="${BREWY_REPOSITORY_URL:-https://github.com/Hexadecimall/homebrew-brewy.git}"
release_tag="${BREWY_VERSION:-v0.1.0}"
cleanup_dir=""

cleanup() {
    if [ -n "$cleanup_dir" ] && [ -d "$cleanup_dir" ]; then
        rm -rf "$cleanup_dir"
    fi
}
trap cleanup EXIT HUP INT TERM

fail() {
    printf 'brewy: %s\n' "$1" >&2
    exit 1
}

[ "$(uname -s)" = "Darwin" ] || fail "macOS is required"
command -v brew >/dev/null 2>&1 || fail "Homebrew is required"
command -v cargo >/dev/null 2>&1 || fail "Rust and Cargo are required"
command -v install >/dev/null 2>&1 || fail "the install utility is required"

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || true)
source_dir=""
if [ -n "$script_dir" ] && [ -f "$script_dir/Cargo.toml" ] &&
    grep -q '^name = "brewy"$' "$script_dir/Cargo.toml"; then
    source_dir=$script_dir
fi

if [ -n "${BREWY_INSTALL_DIR:-}" ]; then
    install_dir=$BREWY_INSTALL_DIR
else
    install_dir="$(brew --prefix)/bin"
fi

if [ -n "$source_dir" ]; then
    printf 'Building Brewy from %s\n' "$source_dir"
    cargo build --locked --release --manifest-path "$source_dir/Cargo.toml"
    built_binary="$source_dir/target/release/brewy"
else
    cleanup_dir=$(mktemp -d "${TMPDIR:-/tmp}/brewy-install.XXXXXX")
    printf 'Building Brewy %s\n' "$release_tag"
    cargo install \
        --locked \
        --git "$repository_url" \
        --tag "$release_tag" \
        --root "$cleanup_dir"
    built_binary="$cleanup_dir/bin/brewy"
fi

[ -x "$built_binary" ] || fail "build did not produce the brewy executable"
install -d "$install_dir" || fail "cannot create $install_dir"
install -m 0755 "$built_binary" "$install_dir/brewy" ||
    fail "cannot install to $install_dir; set BREWY_INSTALL_DIR to a writable directory"

printf 'Installed Brewy to %s/brewy\n' "$install_dir"
