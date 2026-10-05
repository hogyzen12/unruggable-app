# Unruggable App

The Unruggable Solana wallet application for desktop and mobile, with support
for software wallets, Ledger devices, and Unruggable hardware wallets.

## Repository status

- `main` is the stable desktop-app lineage. Its current release is `v1.1.1`.
- `dev/v2.0.0` is reserved for the next desktop update: MLH hardware V2
  compatibility, Windows USB diagnostics, and related reliability fixes.
- Historical experiments are retained as descriptive `archive/*` tags rather
  than permanent development branches.
- Firmware is versioned independently. The shipped MLH firmware is
  `firmware-v0.3.3-mlh-2026` in the firmware repositories.

See [Repository workflow](docs/REPOSITORY_WORKFLOW.md) for the branch and
release policy.

## Desktop development

Install Rust and the platform dependencies required by Dioxus, then use the
locked dependency graph:

```sh
cargo check --locked --no-default-features --features desktop
cargo test --locked --no-default-features --features desktop
```

Hardware-focused tests can be run with:

```sh
cargo test --locked --offline --no-default-features --features desktop hardware::
```

## Platform builds

macOS packaging:

```sh
sh macos_package.sh
```

Windows cross-builds require `cargo-xwin` and a Windows OpenSSL SDK. Set
`X86_64_PC_WINDOWS_MSVC_OPENSSL_DIR` to that SDK before building:

```sh
export X86_64_PC_WINDOWS_MSVC_OPENSSL_NO_VENDOR=1
cargo xwin build --locked --target x86_64-pc-windows-msvc --release \
  --no-default-features --features desktop
sh scripts/package-windows-release.sh
```

Linux packaging:

```sh
cross build --locked --target x86_64-unknown-linux-gnu --release \
  --no-default-features --features desktop
sh scripts/package-linux-release.sh
```

Android and iOS builds require their platform toolchains. The existing scripts
under `scripts/` and `tools/` remain the source of the platform-specific build
steps.

## Releases

Generated packages, installers, checksums, and mobile bundles are attached to
GitHub Releases; they are not committed to the source branch. Release builds
must come from a clean, tested commit and use an annotated version tag.

