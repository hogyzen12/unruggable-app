# Changelog

## Unreleased — v2.0.0

Status as of 2026-10-06: implementation is in progress on `dev/v2.0.0`. The package version remains `1.1.1` until release qualification is complete.

### Added

- `CurrentV2` detection when `GET_INFO` includes `BOARD`, `KEY_STATE`, and `PQC_STATE`.
- Support for the finalized MLH dual-element firmware, including protocol v2 state, session-bound authentication, a negotiated 4096-byte signing limit, on-device PIN entry, `GENERATE`, receive QR show/hide, `LOCK`, and `WIPE_KEYS` compatibility handling.
- Typed firmware errors for PIN/setup failures, permanent lockout, secure-element faults, rejection, timeout, busy state, oversized payloads, and unavailable reset/wipe operations.
- First-time CurrentV2 setup: set PIN, unlock, generate key, validate the returned 32-byte Solana public key, read it back, require an exact match, and require `KEY_STATE=READY`.
- Recovery of interrupted setup where the PIN is finalized but the wallet key is still uninitialized.
- Computer PIN prompts in connect, SOL send, token send, swap, and stake flows, with on-wallet PIN entry retained as a compact optional action for CurrentV2.
- On-device receive-address QR display, with automatic return to the wallet home screen when the receive modal closes.
- Windows diagnostics containing supported USB IDs, detected COM ports, port classifications, actionable open failures, and privacy-safe copyable output.
- Windows PnP inspection for supported hardware that Windows detects without exposing a COM port.
- Manual **Try COMx** fallback for unmatched Windows serial-port metadata.
- A release compatibility guide and physical-device test matrix under `docs/`.
- A configurable managed API client for desktop swap provider and RPC traffic.
- Capability-gated Solana v1 swap transactions for routes that exceed the v0 packet limit supported by the finalized MLH firmware.

### Changed

- Updated `serialport` from the old 4.3 requirement to 4.9.
- Disabled DTR and RTS, retained 115200 baud, bounded startup noise, and added command-specific response deadlines.
- Serial exchanges now perform complete writes and bounded line reads. A timeout, disconnect, cancellation, or uncertain exchange invalidates the connection and requires a clean reconnect.
- CurrentV2 session state is refreshed from the wallet instead of trusting a process-global public-key cache.
- Signing-size checks use the device's negotiated CurrentV2 limit while retaining legacy behavior.
- Sensitive PIN, OTP, message, Base64, and serial command buffers are cleared where owned by the app.
- CurrentV2 setup exposes PIN only; legacy NONE/PIN/OTP and Ledger behavior remain available.
- Permanent lockout messaging no longer recommends a factory reset that the finalized MLH firmware does not support.
- Desktop CSS and essential local imagery are served from the executable instead of depending on a mutable GitHub branch or network access.
- OpenSSL is scoped to Android. Windows desktop builds no longer require an OpenSSL SDK or the historical OpenSSL runtime DLLs.
- The Windows archive script now includes build provenance and the Windows USB support guide.
- Hardware PIN entry now defaults to the computer; CurrentV2 users can expand **Use PIN on Wallet** when they prefer local entry.
- App locking now occurs after five minutes of actual inactivity. Merely switching windows no longer locks the app or tears down the hardware session.
- Jupiter, DFlow, and Titan swap requests now use the managed API backend. Provider credentials and the obsolete direct Titan WebSocket client are no longer shipped in the desktop binary.
- Swap transaction RPC calls use the managed RPC by default while continuing to honor a user-selected custom RPC.
- Native staking uses the managed RPC by default, reads Solana's live minimum delegation, and enforces the greater of that value and the app's 1 SOL floor in both the UI and transaction layer.
- Jito-enabled transactions retain the finalized 4,200-lamport Jito tip and 100,000-lamport Jules tip, using the official eight-account Jito allow-list.
- Jupiter swaps are built from managed gateway instructions and submitted through the configured RPC, avoiding the unreliable Ultra execute path while preserving local hardware signing and review.
- CurrentV2 hardware transactions with the tip bundle include the exact firmware marker guard as instruction one, using a 512-slot review window. Software and legacy transactions do not add this marker-only instruction or slot lookup.
- Swap routes that fit within 1,232 bytes remain v0. Oversized Jupiter, DFlow, and Titan routes are rebuilt as v1 up to the 4,096-byte limit, with compute-budget instructions converted into the v1 transaction configuration.
- V1 swaps use base64 RPC submission with preflight. V0 transactions retain the existing RPC/TPU behavior.

### Fixed

- Fixed Windows wallet discovery gaps for CP210x (`10C4:EA60`), CH340 (`1A86:7523`), FTDI (`0403:6001`), and native ESP32-S3 USB Serial/JTAG (`303A:1001`).
- Fixed weak public-key validation that previously accepted any valid Base58 string.
- Fixed stale unlock state surviving uncertain USB exchanges or disconnects.
- Fixed inconsistent hardware unlock choices between connect and transaction flows.
- Fixed stale Jupiter order data remaining selectable after a later order error.
- Fixed staking accepting amounts below the network's current minimum delegation.
- Fixed a failed swap provider (including a 502 Bad Gateway response) overriding a valid route returned by another provider. Provider failures are now isolated and only surfaced when every provider fails.
- Fixed hardware transactions becoming invalid during user review by replacing the previous 24-slot marker window with the 512-slot CurrentV2 window used by the newer app.
- Fixed successful hardware sends refreshing the selected software-wallet address and overwriting the hardware balance with zero. Send, token-send, stake, and swap now refresh the active wallet through one shared path.
- Debounced USB presence monitoring so one transient serial-enumeration miss cannot silently clear a connected hardware session.
- Fixed the desktop UI losing its CSS/images after the old `dev-app-release` branch was archived.
- Fixed the existing synchronous SNS test so the full desktop suite runs inside a Tokio runtime.
- Fixed some larger swaps failing at RPC submission with `Invalid Request: decoded too large`.
- Fixed the connected hardware-wallet row overflowing and inheriting the transaction-approval icon layout in the wallet selector.
- Fixed native stake scans failing against current `jsonParsed` RPC responses that omit the legacy `warmupCooldownRate` field.
- Fixed stake discovery to inspect both stake and withdrawal authorities, then de-duplicate accounts controlled through both roles.
- Fixed displayed and aggregate stake balances using total account lamports minus rent instead of the RPC's delegated stake amount.
- Fixed activation/deactivation epoch boundaries, added a distinct deactivating state, and restricted deactivate, split, instant-unstake, and withdraw actions to the authority required by each operation.
- Fixed staking insufficient-balance errors reporting the wallet's SOL balance as though it were lamports.
- Corrected capability-aware button guidance: First Edition PIN setup says to keep the button held, First Edition transaction signing says to press once, and CurrentV2/MLH transaction review says to press and hold.
- Fixed hardware approval cancellation so it interrupts an unapproved serial exchange, closes the logical USB session, and requires a clean reconnect. Once the device signature has already been accepted, the app no longer claims the transaction was canceled and instead waits for submission to finish.

### Compatibility and operations

- Preserved `LegacyV0`, legacy PIN/OTP (`NewV1`), and Ledger code paths.
- Published the immutable `dev-app-release` compatibility tag at archived commit `dbb022b5bfd9a883ab005b54eb9de61004d6af2a` so already-installed builds retain their original jsDelivr asset URLs.
- Purged all affected jsDelivr paths and verified the main CSS, PIN CSS, lock image, and onboarding image return HTTP 200 with their expected content types and sizes.
- Historical branch work remains available through descriptive `archive/*` tags; active development remains limited to `main` and `dev/v2.0.0`.

### Verification completed

- Desktop `cargo check` passes.
- Windows MSVC `cargo xwin check` passes.
- The locked Windows MSVC release cross-build passes with Solana v1 support enabled.
- Six focused Solana v1 tests pass: v0 preservation, oversized fallback/round-trip, capability rejection, compute-budget conversion, and malformed/duplicate budget rejection.
- All focused protocol and bounded-serial tests pass.
- Deterministic desktop suite passes serially: 42 passed, 0 failed, 1 deliberately ignored physical-device test, with four live-network tests excluded from the offline run. The serial run avoids the existing process-global PIN-state race between parallel tests.
- Four focused staking regressions pass for the current RPC fixture, the 1 SOL floor, delegated-balance accounting, and activation/deactivation epoch boundaries.
- Native stake-account discovery and display were verified against a hot wallet with stake accounts.
- A physical First Edition wallet connected and completed PIN setup; full signing/reconnect regression remains in progress.
- The First Edition/CurrentV2 transaction-guidance capability regression passes on macOS and the Windows MSVC cross-check remains clean.
- The ignored physical-device test was run explicitly against the connected finalized MLH wallet and passed CurrentV2 classification, firmware-state checks, real Solana public-key validation, and clean disconnect.
- The macOS desktop app builds, launches, and loads CSS/images from the local asset handler.
- Live managed API checks pass for Jupiter quote/order/build, DFlow quote, latest blockhash, and epoch RPC requests.
- The configured staking RPC reports the current 1 SOL minimum, and the preselected Unruggable vote account is active and owned by the Solana vote program.

### Required before release

- Complete the interactive and physical test matrix in `docs/HARDWARE_RELEASE_CHECKLIST.md`.
- Test the Windows diagnostics and driver/no-COM cases on real Windows PCs.
- Confirm the production gateway's desktop access/authentication policy and rate limits before final release.
- Complete a controlled end-to-end swap and stake on macOS; live checks in this changelog did not sign or submit transactions.
- Complete controlled small-v0 and oversized-v1 swaps on the finalized CurrentV2 firmware; v1 transactions use the firmware's blind-review path rather than structured v0 marker review.
- Regress physical legacy hardware and Ledger.
- Build a signed Windows installer and Authenticode-sign the installer/executable.
- Produce and verify the macOS release package, signing, and notarization.
- Bump the package version to `2.0.0` only after qualification, commit from a clean tree, build final artifacts, and tag the release.
