# Nextion UART Uploader

A Windows tool written in Rust to upload the SmartLocker TFT file to Nextion over UART. During transfer, the Mega runs a separate UART bridge that forwards binary data unchanged. The locker application firmware is not used for this transfer.

## Transfer behavior

- Fixed COM3, 9600 baud, 8N1; checks the CH340 USB identity.
- Checks the target model, device identity, file length and SHA-256 before writing.
- Uses `whmi-wri`, an initial ready response, 4096-byte blocks and one ACK per block.
- Sends 16-byte chunks at controlled intervals; no baud changes, retries or resume.
- Requires the final screen-ready frame; stops on errors or timeouts.
- The default build has no serial backend; `hardware-upload` enables it.

The tool is configured for this project's screen and serial connection. Hardware identity checks must be updated when using a different display or port.

## Build and simulation

From this directory:

```text
cargo build --offline --locked
cargo test --offline --locked
cargo test --offline --locked --features hardware-upload
cargo build --offline --locked --features hardware-upload
```

Simulation uses fake transports and does not open a serial port:

```text
./target/debug/nextion-upload.exe simulate --file ../../nextion/SmartLocker.tft --size 824432 --sha256 B297F91CA16755AF8C86F235B5095DC8D9CEBB0AFFB21C48F5DA249FC4FB167B
```

Supported scenarios: success, missing-ready, missing-ack, bad-ack, disconnect, late-ack, missing-restart, wrong-model, wrong-serial and wrong-address. Select one with `--scenario NAME`. Simulation does not verify physical UART reliability or power-loss recovery.

## Hardware upload

The hardware-enabled executable accepts `upload` with `--file`, `--size`, `--sha256` and `--confirm-write REPLACE_NEXTION_CONTENT_NO_FACTORY_BACKUP`. The confirmation option prevents accidental writes; uploading replaces the display application. Do not use Nextion Editor Upload at the same time.

The SmartLocker TFT was successfully uploaded to real hardware. At the fixed baud rate, the transfer took about 24 minutes and received 202 block acknowledgments for 824,432 bytes.

Display power and the bridge connection must stay connected during upload. There is no automatic rollback. An interrupted transfer may require a separate recovery procedure, which has not yet been tested.

Protocol sources: [Nextion upload protocol](https://nextion.ca/documentation/nextion-upload-protocol-v1-1/) and [ITEAD upload implementation](https://github.com/itead/ITEADLIB_Arduino_Nextion/blob/master/NexUpload.cpp).
