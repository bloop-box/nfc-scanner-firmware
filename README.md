# Bloop NFC Scanner Firmware

[![CI](https://github.com/bloop-box/nfc-scanner-firmware/actions/workflows/ci.yml/badge.svg)](https://github.com/bloop-box/nfc-scanner-firmware/actions/workflows/ci.yml)

Firmware for the Bloop NFC Scanner running on RP2040.

## Usage

Download the latest release from the releases page and flash it onto your NFC scanner.

## Wiring

The firmware requires the following wiring in order to work:

| GPIO | Description            |
|------|------------------------|
| 2    | MFRC522 - Reset        |
| 3    | MFRC522 - IRQ          |
| 4    | MFRC522 - MISO         |
| 5    | MFRC522 - CS           |
| 6    | MFRC522 - SCK          |
| 7    | MFRC522 - MOSI         |
| 16   | Status LED             |
| 22   | Mode Switch (optional) |
| 25   | Power LED (internal)   |

## How it works

When the scanner detects an NFC card, it will emit a start hotkey, followed by the hex characters of the UID and ending
with an end hotkey.

- Start hotkey: CTRL + ALT + SHIFT + "U"
- End hotkey: CTRL + ALT + SHIFT + "D"

For integration into browser environments we provide an easy to use
[SDK](https://github.com/bloop-box/nfc-scanner-client-browser) which takes care of all event handling.

## Mode Switch

When a mode switch is installed, you can deactivate the sending of the start and end hotkeys. In this mode, the NFC
scanner will only transmit the UID. The standard mode enabled is when the switch connects to 3.3v, while the UID-only
mode is enabled when the switch connects to GND.

## Development

- Install probe-run: `cargo install probe-run --version=0.3.6 --locked`
- Add target: `rustup target add thumbv6m-none-eabi`
- Run code against a probe: `cargo run`

