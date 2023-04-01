#!/bin/bash

VERSION="$1"

apt install libudev-dev
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs

sed -i '/\[package\]/,/^version = "[^"]*"$/ s/^version = "[^"]*"$/version = "'"$VERSION"'"/' Cargo.toml
cargo build --release || exit 1
elf2uf2-rs target/thumbv6m-none-eabi/release/bloop-nfc-scanner || exit 1
