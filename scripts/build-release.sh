#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cargo_home_dir=${CARGO_HOME:-${HOME}/.cargo}
rustup_home_dir=${RUSTUP_HOME:-${HOME}/.rustup}

export RUSTFLAGS="--remap-path-prefix=${project_dir}=. --remap-path-prefix=${cargo_home_dir}=/cargo --remap-path-prefix=${rustup_home_dir}=/rustup"

cargo build \
    --locked \
    --release \
    --manifest-path "$project_dir/Cargo.toml"

binary="$project_dir/target/release/brewy"
if strings "$binary" | grep -F "$project_dir" >/dev/null 2>&1; then
    printf 'brewy: release binary contains the project path\n' >&2
    exit 1
fi
if strings "$binary" | grep -F "$cargo_home_dir" >/dev/null 2>&1; then
    printf 'brewy: release binary contains the Cargo home path\n' >&2
    exit 1
fi
if strings "$binary" | grep -F "$rustup_home_dir" >/dev/null 2>&1; then
    printf 'brewy: release binary contains the Rustup home path\n' >&2
    exit 1
fi

printf '%s\n' "$binary"
