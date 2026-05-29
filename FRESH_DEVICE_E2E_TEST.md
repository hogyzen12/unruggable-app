# Fresh Device E2E Test (App + ESP32 Firmware)

Date: 2026-02-15
Target: `unruggable-app` + new ESP32 signer firmware (auth setup support)

## 1. Preconditions

- ESP32 flashed with latest firmware build.
- App built from current integration branch.
- USB serial connection working.
- You can open a monitor for device logs.

App launch (desktop dev):

```bash
cd /Users/hogyzen12/coding-project-folders/unruggable-app
CARGO_TARGET_DIR=/tmp/unruggable-app-target cargo run --no-default-features --features desktop
```

Optional serial check:

```bash
espflash monitor --port /dev/tty.usbserial-0001
```

## 2. Fresh Device Reset (Factory Wipe)

Goal: force `AUTH_MODE=UNSET` for first-connect setup flow.

1. Power off device.
2. Hold boot button while powering device on.
3. Keep holding ~10 seconds (factory wipe hold threshold).
4. Confirm wipe signal (`FACTORY_WIPE_DONE` in serial output).
5. Reboot device.

Expected result:
- Device key is regenerated.
- Public key changes from previous identity.
- Setup is required on next app connect.

## 3. App Connect + Onboarding Trigger

1. Open app.
2. Open Hardware Wallet modal.
3. Connect ESP32.

Expected:
- App detects ESP32 and connects.
- If firmware is NewV1 and fresh: setup screen appears in modal.
- Setup screen explains one-time choice (`NONE`, `PIN`, `OTP`) and immutability.

If setup screen does not appear:
- Device is likely already finalized or detected as legacy.
- Re-run factory wipe and reconnect.

If device is already finalized in `PIN` or `OTP` mode:
- App should show a connect-time unlock step before finalizing connection.
- Successful unlock starts session; no unlock prompt should appear again until session expiry.

## 4. Setup Path Tests

Run each path from a fresh wipe (separate run each):

### 4.1 NONE mode

1. Choose `No Auth (NONE)`.
2. Confirm in app.
3. Hold hardware button when prompted.

Expected:
- Setup completes.
- Continue button appears.
- Connection finalizes.
- Future signs do not require unlock code.

### 4.2 PIN mode

1. Choose `Device PIN`.
2. Enter same 6-digit PIN twice.
3. Confirm in app.
4. Hold hardware button when prompted.

Expected:
- Setup completes.
- App immediately requests Device PIN verification before setup flow completes.
- Signing within active session should not require extra unlock.

### 4.3 OTP mode

1. Choose `Authenticator OTP`.
2. Tap `Generate OTP Pairing`.
  - Hold hardware button when prompted (setup command waits for physical confirmation).
3. Scan the in-app QR code with authenticator app (or use URI/secret manual fallback).
4. Enter current 6-digit OTP.
5. Confirm in app.

Expected:
- Setup completes.
- QR and manual secret are both visible in setup.
- App immediately requests OTP verification before setup flow completes.
- Signing within active session should not require extra unlock.

## 5. Signing + Unlock Tests

Use Send modal after setup.

### 5.1 NONE mode sign

1. Send SOL with hardware wallet.
2. Approve physically on device.

Expected:
- No unlock modal.
- Sign succeeds after device button press.

### 5.2 PIN mode sign

1. Attempt send.
2. On `LOCKED`, unlock modal appears.
3. Enter 6-digit Device PIN.
4. App retries send automatically.

Expected:
- Unlock succeeds.
- Transaction continues and signs.

### 5.3 OTP mode sign

1. Attempt send.
2. On `LOCKED`, unlock modal appears.
3. Enter 6-digit authenticator code.
4. App retries send automatically.

Expected:
- Unlock succeeds.
- Transaction continues and signs.

## 6. Failure/Recovery Cases

### Wrong PIN/OTP

- Enter wrong code and verify unlock error is shown.
- Retry with correct code and confirm it recovers.

### Setup not finalized

- If sign fails with setup-related error, app should direct user to complete hardware setup.

### Device lockout

- After many wrong attempts, firmware may return `AUTH_LOCKED`.
- App should show recovery guidance (physical factory wipe).

## 7. Legacy Compatibility Smoke Test

Using a legacy firmware board:

1. Connect from hardware modal.
2. Ensure no setup wizard appears.
3. Send/sign flow still works via existing path.

Expected:
- Legacy connect/sign unchanged.
- No onboarding regression for legacy users.

## 8. Regression Notes

- Full repo still has many existing warnings unrelated to this integration.
- `cargo check` should pass with no errors on this branch.

## 9. Post-Test Device Reset

If you want to fully clear device state after test runs, use either:

### Option A: Fast reset (recommended for repeated QA)

- Use physical factory wipe (boot-button hold at startup).
- Keeps firmware flashed, resets key + auth state.

### Option B: Full flash erase (cleanest baseline)

```bash
espflash erase-flash --port /dev/tty.usbserial-0001
espflash flash --port /dev/tty.usbserial-0001 target/xtensa-esp32-espidf/release/esp32-solana-signer
espflash monitor --port /dev/tty.usbserial-0001
```

Expected after Option B:
- Device boots as fresh install.
- Setup flow triggers again on next app connect.
