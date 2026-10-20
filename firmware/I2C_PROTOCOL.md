# I2C Locker Protocol

The Mega is the I2C controller, and the Uno is a peripheral at **7-bit address 0x12**. The bus runs at **50 kHz**. Locker states are stored in software on the Uno.

## Transactions

The Mega sends a two-byte request followed by STOP. The Uno handles the request in its main loop, so the two-byte response is read in a separate transaction. A combined write/read transaction with repeated START is not used.

| Operation | Request bytes (hex) |
| --- | --- |
| OPEN 1 | 01 01 |
| OPEN 2 | 01 02 |
| CLOSE 1 | 02 01 |
| CLOSE 2 | 02 02 |
| STATUS | 03 00 |

Response: `[result, state_mask]` — result code and state bitmask.

| Result | Meaning |
| --- | --- |
| 00 | Success |
| 01 | Invalid operation code |
| 02 | Invalid locker ID |
| 03 | Invalid request length |
| 04 | Busy |

Bit 0 means locker 1 is open; bit 1 means locker 2 is open. Other bits must be zero. Errors leave locker states unchanged. The state snapshot in a BUSY response is not a completed operation result.

## Implementation details

The Uno's Wire interrupt callbacks copy the incoming request or send a prepared status response. They do not change locker states. The main loop handles the request, updates D13 and prepares the response atomically. Only one request is handled at a time. Avoid changing states over USB during an I2C test.

The Mega sends the request once, polls about every 10 loop ticks, and stops waiting after 250 ticks of BUSY. These are loop counts, not exact time limits. Transfer errors are not retried automatically. A failure can leave the operation result unknown; resolve the fault before querying status.

The pinned HAL waits for the hardware transfer without a stuck-bus timeout. An electrical fault can therefore stop Mega LED/USB servicing; the BUSY limit cannot interrupt that wait. Automatic bus recovery is not implemented.

## Wiring

| Mega | Uno |
| --- | --- |
| D20 / SDA | A4 / SDA |
| D21 / SCL | A5 / SCL |
| GND | GND |

Both boards use 5 V logic. Keep wires short and do not join independent power rails. Power down before rewiring. Internal pull-ups are enabled, but extra pull-ups on the clone boards and signal rise times have not been measured.

Normal I2C operation for both lockers was verified on real hardware. Fault recovery and long-duration tests have not yet been performed.
