# Nextion Screen Design

The interface is designed for the **NX8048P070-011R** Intelligent Series display in **800x480 landscape** mode. The project file is `SmartLocker.HMI`, with fonts in `title32.zi` and `body24.zi`. The screen uses a simple dark theme and ASCII labels.

## Pages

| Page ID | Name | Purpose |
| --- | --- | --- |
| 0 | pMain | Select locker 1 or 2 |
| 1 | pPin | Selected locker, masked PIN, keypad and editing controls |
| 2 | pResult | Access result, selected locker and return button |

## Component mapping

IDs are local to each page and match the current Editor project and Rust application. Changing components in Editor can change their IDs; keep this mapping up to date.

| Page | Component ID | Name / action |
| --- | --- | --- |
| pMain | 1, 2 | tTitle, tHint |
| pMain | 3, 4 | bLocker1, bLocker2 |
| pPin | 1, 2, 3, 4 | tTitle, tLocker, tPin, tHint |
| pPin | 5–13 | b1–b9: digits 1–9 |
| pPin | 14 | bClear |
| pPin | 15 | b0: digit 0 |
| pPin | 16 | bDelete |
| pPin | 17 | bBack |
| pPin | 18 | bConfirm |
| pResult | 1, 2, 3, 4 | tTitle, tLocker, tResult, tDetail |
| pResult | 5 | bHome |

Action buttons have Send Component ID enabled for Touch Release and disabled for Touch Press. Button event scripts are empty. The Mega handles page changes, PIN validation and locker requests.

## UART rules

Use 9600 baud, 8N1. End each outgoing command with three actual `FF` bytes. Component events are seven bytes long:

```text
65 page_id component_id state FF FF FF
```

State 00 means RELEASE; 01 means PRESS. Rust acts only on matching RELEASE events. `sendme` returns `66 page_id FF FF FF`; page-specific text is updated after the page is ready. The application also handles the startup frame `88 FF FF FF` for resynchronization.

PIN text uses `----`, `*---`, `**--`, `***-`, `****`. The result starts neutral; the Mega writes ACCESS GRANTED or ACCESS DENIED after validation. The public four-digit development PIN is 1234 and is not stored in display text.

## Appearance and export

RGB565 colors: background 4259, button 6438, text 65535, secondary text 42360, accent 11452, success 11758 and denial 57931. Title and body fonts are 32 and 24 pixels. There are no animations or external image assets.

The project compiled in Nextion Editor without errors or warnings and was uploaded to the physical display. Build and export the HMI project to create a TFT file. The [Rust UART upload tool](../tools/nextion-upload/README.md) is described separately.
