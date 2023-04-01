#!/bin/bash

VERSION="$1"

rustup target add thumbv6m-none-eabi || exit 1
sudo apt install -y libudev-dev || exit 1
cargo install elf2uf2-rs || exit 1

sed -i '/\[package\]/,/^version = "[^"]*"$/ s/^version = "[^"]*"$/version = "'"$VERSION"'"/' Cargo.toml
cargo build --release || exit 1
elf2uf2-rs target/thumbv6m-none-eabi/release/bloop-nfc-scanner || exit 1
