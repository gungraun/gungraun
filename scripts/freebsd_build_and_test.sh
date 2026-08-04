#!/usr/bin/env sh
# spell-checker:ignore tlsv proto

set -e

# shellcheck disable=SC1090
. ~/.cargo/env

echo Prepare

# Generic approach
# host=$(rustc -vV | grep '^host:' | cut -d' ' -f2)
# curl --proto '=https' --tlsv1.2 -fsSL "https://github.com/taiki-e/cargo-hack/releases/latest/download/cargo-hack-$host.tar.gz" | tar xzf - -C "$HOME/.cargo/bin"

# Download binary and install to $HOME/.cargo/bin
curl --proto '=https' --tlsv1.2 -fsSL "https://github.com/taiki-e/cargo-hack/releases/latest/download/cargo-hack-x86_64-unknown-freebsd.tar.gz" | tar xzf - -C "$HOME/.cargo/bin"

cargo update

echo Build with the feature powerset
just build-hack

# The freebsd github action vm tends to run out of space, so we run `cargo
# clean` as often as possible
cargo clean

# This excludes the ui tests
echo Run normal tests
cargo test --workspace --exclude valgrind-requests-tests

cargo clean

echo Run valgrind requests tests
cargo +stable test -p valgrind-requests-tests --test tests --release -- --nocapture

cargo clean

echo Run benchmark tests
just system-test-all
