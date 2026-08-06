#!/usr/bin/env sh

set -e

# shellcheck disable=SC1090
[ -e ~/.cargo/env ] && . ~/.cargo/env

uname -a
echo "$SHELL"
df -h
pwd
ls -lah
whoami
env | sort
freebsd-version

valgrind --version

rustup --version
rustup show
rustup component list --installed
rustc --version --verbose
