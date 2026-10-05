# Hardware compatibility

The planned v2 desktop release uses one 115200-baud USB serial transport for Unruggable wallets and selects behavior from the firmware response.

| Capability | Detection | Setup and authentication | Supported behavior |
| --- | --- | --- | --- |
| `LegacyV0` | `GET_INFO` is unsupported | Existing legacy flow | Existing public-key and signing commands |
| `NewV1` | `GET_INFO` is present without the CurrentV2 field set | Existing NONE, PIN, or OTP behavior | Existing setup, unlock, public-key, and signing commands |
| `CurrentV2` | `GET_INFO` contains `BOARD`, `KEY_STATE`, and `PQC_STATE` | PIN only; computer entry by default, on-wallet entry optional | PIN setup, `GENERATE`, verified public key, receive QR, session-bound signing, and permanent lockout reporting |
| Ledger | Ledger HID discovery | Existing Ledger flow | Existing derivation-path discovery and signing |

## CurrentV2 setup

The dual-element firmware separates authentication from key creation. The app performs:

1. `SET_MODE:PIN:<pin>` and verifies that PIN mode was finalized.
2. `UNLOCK:PIN:<pin>` to open the secure-element session.
3. `GENERATE` and waits through intermediate status lines.
4. Validates the returned value as a real 32-byte Solana public key, reads it back with `GET_PUBKEY`, and requires both values to match.
5. Refreshes `GET_INFO` and requires `KEY_STATE=READY`.

An interrupted setup where the PIN is finalized but the key remains uninitialized can be resumed with the existing PIN. CurrentV2 does not support NONE/OTP setup, reset, key wipe, or PIN recovery.

## Windows behavior

- Discovery recognizes CP210x, CH340, FTDI, and ESP32-S3 native USB Serial/JTAG IDs, plus explicitly branded Unruggable devices.
- `serialport` 4.9 provides the maintained Windows backend.
- Serial enumeration, port-open failures, and Windows PnP devices that have no COM port are shown in a copyable report without USB serial numbers.
- Users can explicitly try an unmatched COM port after confirming it in Device Manager.
- OpenSSL is Android-only; the Windows desktop build no longer needs the two historical OpenSSL runtime DLLs.

The app cannot repair a missing kernel COM device. A no-COM case still requires the driver matching the Device Manager Hardware ID, or correction of the cable, hub, or machine USB policy.

## Swap transaction formats

Swap routes stay on legacy/v0 whenever the serialized transaction fits Solana's 1,232-byte packet limit. If a Jupiter, DFlow, or Titan route is larger, the app can rebuild it as a Solana v1 transaction up to 4,096 bytes. This fallback is enabled for software wallets and for CurrentV2 hardware only when `GET_INFO` advertises both `TX_V1=1` and a signing limit above the v0 ceiling. Legacy hardware and Ledger fail before signing with an actionable route-compatibility message.

V1 moves compute limits and priority fees from compute-budget instructions into the message configuration and does not use address lookup tables. The finalized MLH firmware accepts the v1 wire format, but v1 currently follows its blind-review signing path; structured **App markers verified** review remains available for fitting v0 transactions.
