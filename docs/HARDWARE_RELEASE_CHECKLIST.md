# v2.0.0 hardware compatibility release checklist

Status snapshot: 2026-10-06. Checked items have evidence from this working branch; unchecked items remain release blockers or manual qualification work.

## Implementation and automated verification

- [x] Parse the finalized MLH firmware v0.3.3 `GET_INFO` response and classify it as `CurrentV2`.
- [x] Preserve `LegacyV0`, legacy NONE/PIN/OTP (`NewV1`), and Ledger paths.
- [x] Implement `LOCK`, `GENERATE`, `SHOW_RECEIVE_QR`, `HIDE_RECEIVE_QR`, `UNLOCK:DEVICE`, and `WIPE_KEYS` compatibility.
- [x] Implement typed firmware errors and permanent-lockout-safe messages.
- [x] Implement PIN → unlock → generate → validate/read-back/compare public key → require ready key state.
- [x] Resume PIN-finalized/key-uninitialized setup safely.
- [x] Validate hardware public keys as real 32-byte Solana public keys.
- [x] Apply negotiated CurrentV2 signing limits and sensitive-buffer clearing.
- [x] Invalidate uncertain serial sessions after timeout, cancellation, disconnect, or malformed/oversized responses.
- [x] Prefer computer PIN entry throughout connect and transaction flows, with on-wallet entry minimized as a CurrentV2 option.
- [x] Lock only after five minutes of actual inactivity; changing window focus no longer immediately locks or disconnects the wallet.
- [x] Add the firmware marker guard only to CurrentV2 tip-bearing transactions, with a 512-slot hardware review window; keep software and legacy transaction shapes unchanged.
- [x] Keep fitting swaps on v0 and rebuild oversized Jupiter, DFlow, and Titan routes as Solana v1 only when the signer advertises v1 and sufficient signing capacity.
- [x] Convert v0 compute-budget instructions into v1 transaction configuration and submit v1 as base64 with preflight.
- [x] Refresh the active hardware address after send, token-send, stake, and swap rather than falling back to the selected software wallet.
- [x] Require three consecutive failed USB-presence scans before clearing a connected hardware session.
- [x] Add on-device receive QR show/hide behavior.
- [x] Add Windows COM, USB-ID, open-error, PnP/no-COM, and manual-port diagnostics.
- [x] Update to `serialport` 4.9 and remove the desktop Windows OpenSSL build/runtime requirement.
- [x] Make desktop CSS and essential images local rather than CDN-dependent.
- [x] Desktop check passes.
- [x] Windows MSVC cross-check passes.
- [x] Locked Windows MSVC release cross-build passes with the v1 transaction stack.
- [x] Solana v1 focused tests pass: 6 passed, 0 failed.
- [x] Focused protocol/serial tests pass: 9 passed, 0 failed.
- [x] Deterministic desktop suite passes serially: 41 passed, 0 failed, 1 ignored physical test; four explicitly live-network tests are excluded from the offline run.
- [x] Staking/RPC regressions pass for the current parsed-account shape, 1 SOL floor, delegated balances, and epoch-boundary states: 4 passed, 0 failed.
- [x] Show capability-aware button guidance: First Edition holds during PIN setup but uses a single press for transactions; CurrentV2/MLH holds during transaction review.
- [x] Explicit configured-MLH USB identity test passes on macOS without a PIN attempt or signing action.

## Swap, stake, and RPC qualification

- [x] Route Jupiter quote/order and execute requests through the managed API.
- [x] Route DFlow quote and instruction requests through the managed API.
- [x] Route Titan route requests through the managed API and remove embedded provider credentials/direct WebSocket code.
- [x] Use managed RPC for swap transaction construction/submission by default while preserving custom RPC selection.
- [x] Match the finalized tip amounts and account allow-list: deterministic 4,200-lamport Jito tip plus 100,000-lamport Jules tip, selected without a slot RPC call.
- [x] Isolate provider failures so a 502 from one provider cannot replace a valid route from another.
- [x] Build Jupiter swaps from managed gateway instructions and submit them through the configured RPC instead of Ultra execute.
- [x] Verify live Jupiter quote/order/build and DFlow quote responses from the production gateway.
- [x] Verify live managed RPC latest-blockhash and epoch calls.
- [x] Use managed RPC for native staking by default while preserving custom RPC selection.
- [x] Read the live native-stake minimum and enforce the greater of that value and 1 SOL in both the UI and transaction layer.
- [x] Parse current stake RPC responses without the removed legacy warmup field.
- [x] Discover both stake-authority and withdrawal-authority accounts and de-duplicate accounts returned by both filters.
- [x] Display delegated stake rather than total account lamports, preserve inactive withdrawable balances, and authority-gate each stake action.
- [x] Distinguish activating, active, deactivating, and inactive epoch-boundary states in the staking UI.
- [x] Verify the configured staking RPC and the preselected Unruggable validator vote account.
- [x] Verify native stake-account discovery and display against a hot wallet with stake accounts.
- [ ] Confirm the production gateway's supported desktop authentication/access policy and release rate limits.
- [ ] Complete a small controlled software-wallet swap on macOS.
- [ ] Complete a small controlled CurrentV2 hardware-wallet swap on macOS.
- [ ] Complete an oversized CurrentV2 hardware-wallet swap that falls back to v1 and no longer returns `decoded too large`.
- [ ] Complete a small controlled CurrentV2 hardware-wallet stake on macOS.
- [ ] Exercise deactivate, withdraw, partial-unstake, and merge flows against controlled accounts.

## Live-build asset incident

- [x] Confirm the deleted `dev-app-release` ref caused the installed app's CDN URL to return 404.
- [x] Publish compatibility tag `dev-app-release` at archived commit `dbb022b5bfd9a883ab005b54eb9de61004d6af2a`.
- [x] Keep the cleaned remote branch set unchanged; no compatibility branch was recreated.
- [x] Purge all affected jsDelivr CSS/image paths.
- [x] Verify main CSS, PIN CSS, lock image, and onboarding image return HTTP 200 with expected content types and sizes.
- [ ] Confirm one existing installed v1.1.1 app recovers after a full quit/reopen.

## CurrentV2 physical-device matrix

- [x] Configured wallet: detect `303A:1001`, read exact capability state, validate public key, and disconnect cleanly.
- [ ] Configured wallet: enter PIN on the computer and open an authenticated session.
- [ ] Configured wallet: expand the optional on-wallet PIN action and open an authenticated session.
- [ ] Configured wallet: show receive QR, verify it matches the app address, close it, and return to the wallet home screen.
- [ ] Configured wallet: sign and submit a small controlled transaction after reviewing it on-device.
- [ ] Configured wallet: complete two consecutive sends without reconnecting and confirm the hardware balance refreshes after each send.
- [ ] Configured wallet: verify the exact guard/action/Jito/Jules order produces **App markers verified** without separate marker/tip review screens.
- [ ] Configured wallet: verify an oversized v1 swap clearly enters the firmware's blind-review flow, then signs and submits successfully.
- [ ] Configured wallet: allow the session to expire, unlock again, disconnect, reconnect, and repeat signing.
- [ ] Blank wallet: set PIN, unlock, generate, validate/read back the public key, and reconnect.
- [ ] Interrupted setup: PIN finalized/key uninitialized resumes generation using the existing PIN.
- [ ] Confirmation timeout and user rejection return safely to a retryable state.
- [ ] Oversized signing request is rejected before transport.
- [ ] Secure-element fault displays support guidance and blocks signing.
- [ ] Permanent lockout displays no-reset/no-recovery guidance. Do not induce lockout on a production wallet.

## Windows physical USB matrix

- [ ] Native ESP32-S3 (`303A:1001`): detect, connect, unlock, sign, disconnect, and reconnect.
- [ ] Legacy CP210x (`10C4:EA60`): detect, connect, sign, disconnect, and reconnect.
- [ ] CH340 (`1A86:7523`) and FTDI (`0403:6001`), if shipped: detect and connect.
- [ ] Unknown-but-valid wallet COM metadata: confirm manual **Try COMx** connects.
- [ ] Port owned by another application: display an actionable open/busy error.
- [ ] PnP wallet with no COM port: display the matching Hardware ID and driver guidance.
- [ ] Install only the driver matching the detected Hardware ID, reconnect, and confirm the COM port appears.
- [ ] Verify diagnostics omit USB serial numbers and can be copied into a support message.

For each Windows run, record Windows version, PC model, cable/hub, Hardware ID, COM number, app commit/version, firmware build, and result.

## Legacy and Ledger regression matrix

- [x] Physical First Edition wallet: detect, connect, and complete PIN setup.
- [ ] `LegacyV0`: connect, retrieve public key, sign, disconnect, and reconnect.
- [ ] Legacy PIN (`NewV1`): connect, unlock, sign, handle failed authentication safely, and reconnect.
- [ ] Legacy OTP (`NewV1`): connect, unlock, sign, handle failed authentication safely, and reconnect.
- [ ] Ledger: connect, scan derivation paths, select a funded path, sign, disconnect, and reconnect.

## Packaging and release gates

- [x] Review and commit the initial v2 compatibility implementation on `dev/v2.0.0`.
- [ ] Bump `Cargo.toml` from `1.1.1` to `2.0.0` after physical qualification.
- [ ] Build all release artifacts from a clean commit and record the commit in each artifact.
- [ ] Build the macOS app/DMG, verify local assets, code-sign, notarize, and verify the checksum.
- [ ] Replace the transitional portable Windows zip with a signed installer.
- [ ] Have the installer detect the Hardware ID and offer only the matching signed USB driver when one is actually required.
- [ ] Authenticode-sign the Windows executable and installer and verify the signatures on a clean PC.
- [ ] Verify Windows installation, upgrade, uninstall, SmartScreen behavior, WebView2 availability, and local asset loading.
- [ ] Attach checksums, Windows USB support instructions, and release notes to the GitHub release.
- [ ] Merge the qualified v2 commit according to `docs/REPOSITORY_WORKFLOW.md`.
- [ ] Create and push annotated release tag `v2.0.0` only after artifact verification.

## Documentation

- [x] Hardware capability and protocol overview: `docs/HARDWARE_COMPATIBILITY.md`.
- [x] Windows customer/support guide: `docs/WINDOWS_USB_SUPPORT.md`.
- [x] v2 changelog: `CHANGELOG.md`.
- [x] Repository/branch policy: `docs/REPOSITORY_WORKFLOW.md`.

Never deliberately consume a PIN attempt on a production wallet. Keep the known-good configured test device available and use only its confirmed PIN during interactive testing.
