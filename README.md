# Bloop NFC Scanner Firmware

[![CI](https://github.com/bloop-box/nfc-scanner-firmware/actions/workflows/ci.yml/badge.svg)](https://github.com/bloop-box/nfc-scanner-firmware/actions/workflows/ci.yml)

Firmware for the Bloop NFC Scanner running on RP2040.

## Usage

Download the latest release from the releases page and flash it onto your NFC scanner.

## How it works

When the scanner detects an NFC card, it will emit a start hotkey, followed by the hex characters of the UID and ending
with an end hotkey.

- Start hotkey: CTRL + ALT + SHIFT + "U"
- End hotkey: CTRL + ALT + SHIFT + "D"

For integration into browser environments we provide an easy to use
[SDK](https://github.com/bloop-box/nfc-scanner-client-browser) which takes care of all event handling.

## Development

- Install probe-run: `cargo install probe-run --version=0.3.6 --locked`
- Add target: `rustup target add thumbv6m-none-eabi`
- Run code against a probe: `cargo run`

