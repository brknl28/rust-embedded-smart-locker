# Technical Notes

## Implementation

The Rust application on the Mega runs with `no_std`. A state machine manages locker selection, PIN entry and the result screen. Data is stored in fixed-size buffers. The Mega checks the PIN and sends only masked text to the display. An OPEN request is created for the selected locker only when the four-digit PIN is correct.

The screen connection is checked without blocking the main loop. When the screen responds, the main page is prepared and button actions are enabled after page readiness is confirmed. A display restart, missing replies or an unexpected page triggers resynchronization and clears the entered PIN. This can also be started manually with `HMI START`. Establishing the connection does not create a locker-open request.

The Uno software uses C++ and Arduino `Wire`. I2C interrupt callbacks only exchange request and status data. The main loop updates locker states and D13; interrupt callbacks do not print serial messages or wait.

## Toolchain

| Component | Configuration |
| --- | --- |
| Mega MCU | ATmega2560, 16 MHz, `avr-none` |
| Rust | `nightly-2025-04-27`, with `rust-src` |
| avr-hal | `e5c8f37fe48419956e722490a82b9ca9b9fc61a2` |
| Uno target | `arduino:avr:uno`, ATmega328P |
| Arduino core | Arduino AVR Boards 1.8.8 |
| Nextion | NX8048P070-011R, 800x480; Editor 1.68.2 |

## Build and checks

Run Mega commands from `firmware/mega-rust/` so the pinned toolchain and AVR configuration apply:

```text
cargo build --release --bin mega-rust --locked --offline
```

On the Windows development computer, the Rust logic tests use a host target without firmware dependencies:

```text
rustc +stable --edition=2021 --test src/host_tests.rs --target x86_64-pc-windows-msvc -o target/host-tests.exe
./target/host-tests.exe
```

Build the Uno sketch from the repository root:

```text
arduino-cli compile --fqbn arduino:avr:uno firmware/uno-cpp/locker_uno
```

The C++ host test command is in the [Uno software notes](firmware/uno-cpp/README.md). The Nextion project and required fonts are in `nextion/`.

## Validation summary

| Area | Verified scope |
| --- | --- |
| Firmware | Mega AVR and Uno C++ builds, upload and read-back verification |
| USB serial | PING/PONG and locker command responses |
| Nextion | Physical TFT upload, button events, navigation, masking, editing, incorrect-PIN result and HOME |
| Integrated application | Both correct-PIN flows, selected Uno I2C state and physical LED changes; 10/10 command checks |
| Automatic-start update | 26 Rust tests; AVR build; 15,968 flash bytes verified; USB PING/PONG 3/3; physically confirmed power-on navigation |

The correct-PIN flows and I2C status responses for both lockers were tested together before the automatic-start update. That complete test has not yet been repeated with the current firmware. Screen changes and LED behavior were physically observed, while serial replies were compared with expected values.

All device power-on orders and an isolated display restart have not been fully tested. Host tests do not verify electrical reliability.

## Operating limits

- The demo PIN `1234` is public and is not suitable for real access control.
- ACCESS GRANTED means PIN acceptance, not door feedback. Check the Uno state for the operation result.
- The HAL's blocking I2C transfer can hang on a stuck bus. The higher-level response timeout cannot interrupt that wait.
- Keep all connected devices powered during operation. Power down before rewiring and avoid feeding an unpowered board through signal pins.
- There is no backup of the display's original factory demo. A TFT upload replaces the current content; a failed serial upload may require a recovery procedure that has not been tested here.
- The cause of the known 27-byte startup prefix is unknown. Incomplete-PIN physical testing, endurance and fault recovery remain unverified.
