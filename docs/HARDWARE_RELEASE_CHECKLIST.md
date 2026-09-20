# Hardware compatibility release checklist

Run this matrix from a clean release commit before tagging the app.

## Automated gates

- Protocol fixture tests pass.
- Desktop tests pass.
- Windows MSVC release build passes.
- Windows zip contains the EXE, three runtime DLLs, assets, `BUILD_INFO.txt`, and `WINDOWS_USB_SUPPORT.md`.
- Zip integrity and SHA-256 verification pass.

## Windows USB matrix

- Legacy CP210x device: detected, connected, reconnects, and signs.
- Legacy CH340/FTDI device if shipped: detected, connected, reconnects, and signs.
- ESP32-S3 native USB device: detected as `303A:1001`, connected, reconnects, and signs.
- Unknown-but-valid wallet COM metadata: manual **Try COMx** connects.
- Port already open in another app: actionable open/busy error is displayed.
- No driver/no COM port: diagnostic report and matching official-driver instructions are displayed.

## Protocol and device matrix

- `LegacyV0`: connect, public key, signing, disconnect, reconnect.
- legacy PIN device (`NewV1`): connect, unlock, signing, incorrect PIN, reconnect.
- legacy OTP device (`NewV1`): connect, unlock, signing, incorrect OTP, reconnect.
- Ledger: connect, derivation-path scan, signing, disconnect, reconnect.
- blank dual-element device (`CurrentV2`): set PIN, unlock, generate, verify returned public key.
- configured dual-element device: unlock, show/hide receive QR, sign, disconnect, reconnect.
- interrupted dual-element setup (PIN set/key uninitialized): enter existing PIN, generate, verify.
- locked dual-element device: `AUTH_LOCKED` is shown without falling back to legacy behavior.
- secure-element fault: `KEYSTORE_CORRUPT` is shown and signing is blocked.
- oversized signing request: rejected before transport at 2048 bytes maximum.
- confirmation timeout: `CONFIRM_TIMEOUT` is shown and the app remains recoverable.
- wipe, if exposed in the release UI: device returns to PIN-unset/key-uninitialized state.

Record the Windows version, PC model, cable, Device Manager Hardware Id, COM number, app version, firmware version, and result for every physical run.
