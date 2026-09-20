# Hardware compatibility overview

App release `0.2.0` uses one 115200-baud USB serial transport for Unruggable devices and selects behavior from the firmware protocol response.

| Capability | Detection | Setup and auth | Supported behavior |
| --- | --- | --- | --- |
| `LegacyV0` | `GET_INFO` is unsupported | Existing legacy flow | Existing public-key and signing commands |
| `NewV1` | `GET_INFO` is present without the CurrentV2 field set | Existing NONE, PIN, or OTP behavior | Existing setup, unlock, public-key, and signing commands |
| `CurrentV2` | `GET_INFO` contains `BOARD`, `KEY_STATE`, and `PQC_STATE` | PIN only | PIN setup, unlock, `GENERATE`, verified public key, receive QR, signing, and key wipe |
| Ledger | Ledger HID discovery | Existing Ledger flow | Existing derivation-path discovery and signing |

## CurrentV2 setup sequence

The dual-element firmware intentionally separates authentication from key creation. The app now performs:

1. `SET_MODE:PIN:<pin>` and verifies that PIN mode was finalized.
2. `UNLOCK:PIN:<pin>` so the secure-element session is open.
3. `GENERATE` and waits through the intermediate `KEY_GENERATED` status line.
4. Parses the returned `PUBKEY`, validates it as an actual 32-byte Solana public key, reads it back with `GET_PUBKEY`, and requires both values to match.
5. Refreshes `GET_INFO` and requires `KEY_STATE=READY`.

An interrupted setup where the PIN is finalized but the key remains uninitialized is recoverable by entering the existing PIN and completing steps 2–5.

## Windows changes

- Serial discovery recognizes CP210x, CH340, FTDI, and ESP32-S3 native USB Serial/JTAG IDs, plus an explicitly branded Unruggable identity.
- `serialport` is updated to 4.9, which uses the maintained `windows-sys` Windows backend introduced in 4.8.
- Enumeration and port-open failures are preserved and shown in a copyable diagnostic report without USB serial numbers.
- A user can explicitly try an unmatched COM port after checking it in Device Manager.
- The release archive includes matching official-driver guidance for machines where Windows creates no COM port.

The app cannot repair a missing kernel COM device. A no-COM case still requires the driver matching the Device Manager Hardware Id, or correction of the cable, hub, or machine USB policy.

## Release boundary

Automated protocol tests and a Windows MSVC build establish source/build compatibility. Production release still requires the physical-device matrix in `HARDWARE_RELEASE_CHECKLIST.md`; firmware and secure-element behavior cannot be proven by a cross-build alone.
