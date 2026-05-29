# ESP32 Hardware Firmware Integration Spec (Final)

Date: 2026-02-15  
App repo: `unruggable-app`  
Firmware repo: `esp32-solana-signer`  
Firmware profile: Dev-device freeze (no OTA/update flow in this phase)

## 1. Objective

Integrate the finalized ESP32 firmware into the app with:
- Seamless first-connect onboarding for fresh devices (`UNSET` auth state).
- Support for exactly one selected device auth mode (`NONE`, `PIN`, or `OTP`).
- No breakage for legacy firmware devices that only support `GET_PUBKEY` and `SIGN`.
- Minimal complexity and clear ownership boundaries in app code.

## 2. Scope and Non-Goals

In scope:
- Firmware capability detection and backward-compatible runtime behavior.
- First-time setup UI/flow for `NONE | PIN | OTP`.
- Unlock orchestration for PIN/OTP devices before signing.
- Transport/parser hardening for ESP32 boot log noise and richer responses.

Out of scope for this phase:
- Firmware update/OTA UX.
- Device-side transaction display (device has no screen, blind-signing accepted).
- Multi-factor stacking (exactly one mode only, or none).
- Any protocol to switch mode after finalized (physical wipe only).

## 3. Final Firmware Contract (Source of Truth)

Source files:
- `esp32-solana-signer/src/main.rs`
- `esp32-solana-signer/src/twofa.rs`

Final constants:
- `MAX_CMD_LEN = 4028`
- `MAX_SIGN_PAYLOAD_BYTES = 2048` (decoded bytes)
- `MAX_AUTH_ATTEMPTS = 10`
- `UNLOCK_WINDOW_SECS = 120`
- `PIN = exactly 6 digits`
- `OTP_DIGITS = 6`
- `OTP_PERIOD = 30s`
- `OTP_WINDOW = +/-1 step`

Persisted auth/key state (NVS):
- `solana_key` (ed25519 seed)
- `auth_mode` (`UNSET=0`, `NONE=1`, `PIN=2`, `OTP=3`)
- `auth_finalized`
- `auth_fail_count`
- `auth_locked`
- `pin_salt`, `pin_hash`
- `otp_secret`, `otp_last`, `otp_enrolled`

Factory wipe behavior (kept as requested):
- Trigger: hold boot button at startup for ~10s.
- Wipes auth + OTP + PIN + `solana_key`.
- After wipe, next boot generates a new keypair.
- Device identity/pubkey changes after wipe.

## 4. Wire Protocol (Exact)

Transport framing:
- ASCII line protocol over serial/USB.
- Commands terminated with `\n`.
- Responses are single lines ending with `\n`.
- Success responses are plain payload strings (no `OK:` prefix).
- Error responses are `ERROR:<code_or_message>`.

Core commands and responses:

| Command | Success response | Notes |
|---|---|---|
| `PING` | `PONG` | Safe health check |
| `GET_INFO` | `INFO;VER=<v>;AUTH_MODE=<UNSET|NONE|PIN|OTP>;FINALIZED=<0|1>;LOCKED=<0|1>;RETRIES_LEFT=<n>` | Capability + auth state |
| `GET_PUBKEY` | `PUBKEY:<base58>` | Works in locked state |
| `SET_MODE:NONE` | `MODE_SET:NONE` | Requires button hold; only before finalized |
| `SET_MODE:PIN:<6digits>` | `MODE_SET:PIN` | Requires button hold; only before finalized |
| `SET_MODE:OTP_BEGIN` | `OTP_SECRET:<base32>;URI=<otpauth_uri>;ALGO=SHA1;DIGITS=6;PERIOD=30` | Requires button hold |
| `SET_MODE:OTP_CONFIRM:<6digits>` | `MODE_SET:OTP` | Completes OTP enrollment |
| `UNLOCK:PIN:<6digits>` | `UNLOCKED_UNTIL:<unix>` | PIN mode only |
| `UNLOCK:OTP:<6digits>` | `UNLOCKED_UNTIL:<unix>` | OTP mode only |
| `SIGN:<base64_payload>` | `SIGNATURE:<base64_signature>` | Requires active unlock window for PIN/OTP |
| `SHUTDOWN` | `SHUTDOWN_OK` | Requires button hold |

Legacy aliases supported by new firmware:
- `OTP_BEGIN` -> same as `SET_MODE:OTP_BEGIN`
- `OTP_CONFIRM:<code>[:unix]` -> same as `SET_MODE:OTP_CONFIRM:<code>` (unix ignored)
- `OTP_UNLOCK:<code>[:unix]` -> same as `UNLOCK:OTP:<code>` (unix ignored)

Errors to expect:
- `ERROR:MODE_FINAL`
- `ERROR:MODE_UNSET`
- `ERROR:AUTH_MODE_MISMATCH`
- `ERROR:AUTH_FAILED`
- `ERROR:AUTH_LOCKED`
- `ERROR:BAD_PIN_FORMAT`
- `ERROR:BAD_OTP_FORMAT`
- `ERROR:OTP_NOT_PENDING`
- `ERROR:OTP_BAD_CODE`
- `ERROR:PAYLOAD_TOO_LARGE`
- `ERROR:BUTTON_TIMEOUT`
- `ERROR:LOCKED`
- `ERROR:Unknown command`
- `ERROR:Command too long`
- `ERROR:Invalid base64 encoding`

Lockout semantics:
- After 10 failed auth attempts, device enters locked state.
- In locked state, only these commands are allowed: `PING`, `GET_INFO`, `GET_PUBKEY`.
- All other commands return `ERROR:AUTH_LOCKED`.

## 5. What Changed vs Legacy Protocol (and What Did Not)

Changed:
- New setup, status, and unlock commands were added.
- `SIGN` can now return `MODE_UNSET`, `LOCKED`, or `AUTH_LOCKED` depending on state.

Unchanged (compatibility-critical):
- `GET_PUBKEY` and `SIGN` command syntax unchanged.
- Signature/public key response formats unchanged.
- Errors still use `ERROR:<...>` style.

Impact:
- Older app versions can still connect to already-configured devices for simple flows.
- Fresh new devices require setup before signing (`MODE_UNSET` until finalized).

## 6. Current App State Review (As-Is)

Current integration points:
- `src/hardware/protocol.rs`: only `GetPubkey` and `SignMessage`; parser only `PUBKEY`, `SIGNATURE`, `ERROR`.
- `src/hardware/mod.rs`: `connect_esp32()` directly does `GET_PUBKEY`; no capability/setup/auth model.
- `src/hardware/serial.rs`: one-command/one-line parser, 1024-byte response guard, no boot-noise filtering.
- `src/hardware/android_usb.rs`: single read buffer (1024) and immediate parse; no read-until-line semantics.
- `src/components/modals/hardware_modal.rs`: connect/disconnect only; no setup wizard.
- `src/components/modals/send_modal.rs`: hardware approval overlay exists; no unlock orchestration.
- `src/signing/hardware.rs`: still has heavy transaction introspection logging; should be simplified for clean prod/dev flow.
- `src/main.rs`: onboarding currently hardcoded on (`show_onboarding = true`).

## 7. Target App Integration Architecture

### 7.1 Capability and device state model

Add firmware capability class:
- `LegacyV0`: only `GET_PUBKEY` + `SIGN` expected.
- `NewV1`: `GET_INFO` + setup/unlock commands supported.

Add auth state model:
- `AuthMode = Unset | None | Pin | Otp`
- `DeviceInfoState { version, auth_mode, finalized, locked, retries_left }`

Add runtime session fields on hardware wallet context:
- `capability`
- `device_info` (last known)
- `unlocked_until_hint` (optional, UI optimization only)

### 7.2 Protocol surface in app

Extend `src/hardware/protocol.rs` command enum:
- `Ping`, `GetInfo`
- `SetModeNone`, `SetModePin(String)`, `SetModeOtpBegin`, `SetModeOtpConfirm(String)`
- `UnlockPin(String)`, `UnlockOtp(String)`
- Existing `GetPubkey`, `SignMessage(Vec<u8>)`

Extend response enum:
- `Pong`
- `Info(DeviceInfoState)`
- `ModeSet(AuthMode)`
- `OtpSetup { secret, uri, algo, digits, period }`
- `UnlockedUntil(u64)`
- Existing `Pubkey`, `Signature`, `Error(String)`

Parser requirements:
- Trim CR/LF.
- Parse `INFO;` key/value list and ignore unknown keys.
- Parse OTP setup payload (`OTP_SECRET:...;URI=...;ALGO=...;DIGITS=...;PERIOD=...`).
- Keep unknown lines non-fatal during probe/read loops.

## 8. Capability Detection and Backward Compatibility

Detection algorithm on connect (ESP32 path):
1. Open transport.
2. Drain/ignore startup boot noise lines for a short window.
3. Try `GET_INFO`.
4. If parseable `INFO;...` -> `NewV1`.
5. If `ERROR:Unknown command` or timeout, try `GET_PUBKEY`.
6. If `GET_PUBKEY` succeeds -> `LegacyV0`.
7. If neither works -> connection failure.

Compatibility rules:
- Always prefer fallback to legacy behavior when probing is ambiguous.
- Do not require host-side OTP/PIN implementation for legacy devices.
- For new firmware, app should still parse legacy alias success (`OTP_CONFIRMED`) to simplify rollout.

## 9. First-Connect Onboarding Flow (New Firmware)

Trigger condition:
- `capability == NewV1` and `AUTH_MODE=UNSET` and `FINALIZED=0`.

Flow screens:
1. Intro: one-time auth selection, no mode switching without physical wipe.
2. Mode choice: `None`, `Device PIN`, `Authenticator OTP`.
3. Mode-specific setup.
4. Completion confirmation.

### 9.1 NONE setup
- Send: `SET_MODE:NONE`
- Expect: `MODE_SET:NONE`
- UX: prompt user to hold device button for confirmation.

### 9.2 PIN setup
- Collect + confirm 6-digit device PIN.
- Send: `SET_MODE:PIN:<pin>`
- Expect: `MODE_SET:PIN`
- UX: prompt button hold.
- Security: never persist device PIN in app storage.

### 9.3 OTP setup
- Send: `SET_MODE:OTP_BEGIN`
- Expect setup payload with `URI` and `OTP_SECRET`.
- Render QR from `URI`; show manual secret fallback.
- User enters current 6-digit OTP code.
- Send: `SET_MODE:OTP_CONFIRM:<code>`
- Expect: `MODE_SET:OTP` (also accept `OTP_CONFIRMED` for compatibility)

OTP notes:
- Firmware time is authoritative.
- Host-supplied unix timestamp is not required and ignored by new firmware aliases.
- If `OTP_BEGIN` is repeated, use the latest returned secret/URI.

## 10. Signing and Unlock Runtime Flow

Recommended simple runtime behavior:
1. Attempt `SIGN` directly.
2. If success -> done.
3. If `ERROR:LOCKED` -> prompt for PIN/OTP based on current mode.
4. Send mode-specific unlock command.
5. On `UNLOCKED_UNTIL:<unix>`, retry `SIGN` once automatically.

Mode-specific unlock commands:
- PIN mode: `UNLOCK:PIN:<6digits>`
- OTP mode: `UNLOCK:OTP:<6digits>`

Special cases:
- `ERROR:MODE_UNSET` during sign -> launch setup flow.
- `ERROR:AUTH_LOCKED` -> show lockout screen; only physical wipe can recover.
- `ERROR:AUTH_MODE_MISMATCH` -> stale app mode cache; refresh `GET_INFO` and retry UX branch.

## 11. Transport/Parser Hardening Requirements

### 11.1 Desktop serial (`src/hardware/serial.rs`)
- Replace strict single-line parse with read-loop that ignores non-protocol boot lines.
- Increase response line budget above 1024 (use at least >= `MAX_CMD_LEN` safety margin).
- Continue reading until a parseable protocol response or timeout.

### 11.2 Android USB (`src/hardware/android_usb.rs`)
- Replace one-shot read with loop/read-until-newline behavior.
- Preserve partial reads and concatenate until complete line.
- Ignore non-protocol noise lines during probe.

Reason:
- ESP32 emits boot logs after power/reset; parser must not treat those as fatal protocol errors.

## 12. UX and Copy Rules

Critical distinction in UI copy:
- `App PIN` = app-local unlock.
- `Device PIN` = ESP32 signing unlock.

UI requirements:
- All hardware auth prompts must explicitly say `Device PIN` or `Authenticator Code`.
- OTP setup screen must show QR and manual secret.
- Explain lockout: 10 failed attempts -> physical wipe required.
- Explain blind signing risk clearly (no on-device screen).

## 13. Local Storage Additions (App)

Add a per-device profile keyed by pubkey:
- `pubkey`
- `capability` (`LegacyV0 | NewV1`)
- `last_seen_auth_mode`
- `last_seen_firmware_version`
- `last_seen_at`

Rules:
- Cache is advisory only; firmware is source of truth.
- Never store PIN/OTP code/secret.

## 14. Error-to-UX Mapping

| Firmware error | App behavior |
|---|---|
| `MODE_UNSET` | Start setup wizard |
| `LOCKED` | Show unlock modal and retry sign on success |
| `AUTH_FAILED` | Show incorrect PIN/OTP message |
| `OTP_BAD_CODE` | Show incorrect OTP message |
| `AUTH_LOCKED` | Show hard lockout + factory wipe instruction |
| `BAD_PIN_FORMAT` | Enforce/re-prompt 6-digit PIN |
| `BAD_OTP_FORMAT` | Enforce/re-prompt 6-digit OTP |
| `BUTTON_TIMEOUT` | Show retry/cancel |
| `PAYLOAD_TOO_LARGE` | Show unsupported transaction size error |
| `Invalid base64 encoding` | Internal protocol bug path (log + fail gracefully) |

## 15. File-Level Implementation Plan

Primary files:
- `src/hardware/protocol.rs`
- `src/hardware/serial.rs`
- `src/hardware/android_usb.rs`
- `src/hardware/mod.rs`
- `src/components/modals/hardware_modal.rs`
- `src/components/modals/send_modal.rs`
- `src/storage.rs`

Secondary cleanup:
- `src/signing/hardware.rs`: remove/trim heavy introspection logging for cleaner production/dev behavior.
- `src/main.rs`: restore onboarding gate to persisted state (`storage::has_completed_onboarding()`).

Suggested API additions in `src/hardware/mod.rs`:
- `get_info()`
- `detect_capability()`
- `setup_mode_none()`
- `setup_mode_pin(pin)`
- `setup_mode_otp_begin()`
- `setup_mode_otp_confirm(code)`
- `unlock_pin(pin)`
- `unlock_otp(code)`
- `sign_with_auto_unlock(...)` (or keep in modal layer)

## 16. E2E Test Plan (Dev Device Freeze)

### 16.1 Fresh board / first boot path
1. Trigger physical wipe (boot-button hold at startup).
2. Verify `FACTORY_WIPE_DONE` and reboot.
3. Connect app.
4. App detects `NewV1` and `UNSET`.
5. Complete setup flow in each mode (`NONE`, `PIN`, `OTP`) on separate runs.

Expected:
- `GET_INFO` reflects selected mode and `FINALIZED=1`.
- Mode cannot be changed via protocol (`MODE_FINAL`).

### 16.2 Sign flow
- `NONE`: sign directly with physical button press.
- `PIN`: first sign returns `LOCKED`, unlock prompt appears, retry sign succeeds.
- `OTP`: same as PIN path with OTP input.

### 16.3 Failure and lockout
- Enter wrong PIN/OTP repeatedly.
- Validate retries decrement via `GET_INFO`.
- At 10 failures, confirm `AUTH_LOCKED` and only `PING/GET_INFO/GET_PUBKEY` remain usable.

### 16.4 Backward compatibility
- Connect legacy firmware unit.
- Ensure connect/sign still work without setup wizard.
- Ensure no regression in existing Ledger/software wallet paths.

### 16.5 Payload boundary
- Validate app behavior for oversized serialized tx payloads (>2048 decoded bytes).
- Confirm graceful error mapping for `PAYLOAD_TOO_LARGE`.

## 17. Rollout Plan

Phase 1:
- Protocol/transport changes + capability detection + no-regression legacy behavior.

Phase 2:
- Setup wizard and unlock UX in hardware/send modals.

Phase 3:
- Cleanup polish, telemetry, and reliability hardening.

## 18. Implementation Checklist

- [x] Extend protocol command/response enums and parser.
- [x] Add robust line filtering/read-loop on desktop serial.
- [x] Add robust read-until-line on Android USB.
- [x] Add capability detection (`LegacyV0` vs `NewV1`) in connect flow.
- [x] Add `GET_INFO` model and runtime state tracking.
- [x] Add first-connect setup wizard for `UNSET` devices.
- [x] Add PIN/OTP unlock prompt + automatic sign retry.
- [ ] Add per-device non-secret profile cache.
- [x] Remove/trim transaction introspection logging from hardware signer path.
- [x] Restore persisted onboarding gate in app main flow.
- [ ] Run E2E matrix for fresh/new/legacy devices before release.

## 19. Final Notes

This spec intentionally keeps the host integration simple:
- Backward compatibility is preserved by defaulting to legacy behavior when uncertain.
- New auth features are additive and isolated to ESP32 paths.
- Device setup and unlock are explicit UX steps, while core send/sign flows remain mostly unchanged.
