# Repository workflow

## Branches

- `main` contains the stable application baseline and is the source for public
  releases.
- `dev/v2.0.0` contains the active desktop V2 implementation until it passes
  the release acceptance matrix.
- Short-lived feature branches should branch from the active development
  branch and be removed after integration.

Do not use permanent catch-all development branches. A branch that must remain
for historical reference should be replaced with an annotated `archive/*` tag
whose message explains its contents and status.

## Current archives

- `archive/colosseum-main-2026-10-05` preserves the former experimental
  Colosseum-era `main`.
- `archive/extension-integration-2026-10-05` preserves the unmerged browser
  extension experiment.
- `archive/pre-v2-hardware-compat-2026-10-05` preserves the first preliminary
  Windows/CurrentV2 compatibility checkpoint. It is reference material, not a
  qualified release.

## Versioning

Application releases use semantic versions and annotated tags such as
`v2.0.0`. Firmware has its own version and immutable firmware tags. Application
and firmware versions must not be treated as interchangeable.

Update the package version only when preparing the corresponding release. The
active V2 development branch starts from the stable v1.1.1 baseline and becomes
v2.0.0 after implementation and qualification.

## Generated artifacts

Do not commit application packages or checksums. Attach DMGs, installers,
archives, mobile bundles, checksums, and build metadata to the matching GitHub
Release. Build from a clean commit and record the commit identifier in each
package.

## V2 release gate

Before merging desktop V2 into `main`:

1. Run protocol and transport tests.
2. Test the shipped MLH firmware over physical USB.
3. Regress older Unruggable hardware and Ledger.
4. Test the packaged application on Windows, macOS, and Linux.
5. Verify installer signatures, artifact checksums, and release notes.
6. Merge the qualified development commit and create annotated tag `v2.0.0`.
