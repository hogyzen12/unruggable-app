#!/usr/bin/env bash
# macos_package.sh — Build, sign, notarize, staple, and package a macOS app (cargo-bundle)
set -euo pipefail

### ───────────────────────── CONFIG ─────────────────────────
APP_NAME="${APP_NAME:-unruggable}"                 # Must match .app name produced by cargo-bundle
BINARY_NAME="${BINARY_NAME:-unruggable}"           # Executable name
BUNDLE_ID="${BUNDLE_ID:-com.unruggable.app}"       # Bundle identifier

TEAM_ID="${TEAM_ID:-AX8C7PY24C}"
IDENTITY="${IDENTITY:-}"

# Toggle sandbox: 0 = no sandbox (Developer ID distribution), 1 = sandbox with network client/server
SANDBOX="${SANDBOX:-0}"

# Notarization
NOTARY_PROFILE="${NOTARY_PROFILE:-AC_ASP}"
MODE="${MODE:-APPLE}"  # APPLE or API

# If MODE=API (CI), set these:
API_KEY_PATH="${API_KEY_PATH:-/path/to/AuthKey_XXXXXX.p8}"
API_KEY_ID="${API_KEY_ID:-XXXXXX}"
API_ISSUER_ID="${API_ISSUER_ID:-00000000-0000-0000-0000-000000000000}"

# If MODE=APPLE (local):
APPLE_ID_EMAIL="${APPLE_ID_EMAIL:-billypapas12@gmail.com}"
APPLE_APP_SPECIFIC_PW="${APPLE_APP_SPECIFIC_PW:-}" # leave empty to be prompted
### ──────────────────────────────────────────────────────────

log(){ printf "\n\033[1;36m▶ %s\033[0m\n" "$*"; }

resolve_signing_identity() {
  local identities
  identities="$(security find-identity -v -p codesigning 2>/dev/null || true)"

  if [ -n "${IDENTITY}" ]; then
    if printf '%s\n' "$identities" | grep -Fq "\"${IDENTITY}\""; then
      printf '%s\n' "$IDENTITY"
      return 0
    fi

    echo "❌ Signing identity not found in Keychain: $IDENTITY" >&2
    printf '%s\n' "$identities" >&2
    return 1
  fi

  local detected
  detected="$(
    printf '%s\n' "$identities" |
      sed -n "s/.*\"\\(Developer ID Application: .* (${TEAM_ID})\\)\".*/\\1/p" |
      head -n 1
  )"

  if [ -z "$detected" ]; then
    detected="$(
      printf '%s\n' "$identities" |
        sed -n 's/.*"\(Developer ID Application: .*\)".*/\1/p' |
        head -n 1
    )"
  fi

  if [ -z "$detected" ]; then
    echo "❌ No Developer ID Application signing identity found for team ${TEAM_ID}" >&2
    printf '%s\n' "$identities" >&2
    return 1
  fi

  printf '%s\n' "$detected"
}

### 0) Pre-flight checks
log "Pre-flight checks"
command -v xcrun >/dev/null || { echo "❌ Xcode Command Line Tools required"; exit 1; }
command -v cargo-bundle >/dev/null || { echo "❌ cargo-bundle not found. Install with: cargo install cargo-bundle"; exit 1; }
IDENTITY="$(resolve_signing_identity)"
echo "Using signing identity: $IDENTITY"
case "$(uname -m)" in arm64) : ;; *) echo "❌ Script is configured for Apple Silicon (arm64)."; exit 1;; esac

### 1) Build (release, desktop features, arm64)
log "Building binary with Cargo (release, desktop, arm64)"
cargo build --release --no-default-features --features desktop --target aarch64-apple-darwin

### 2) Bundle into .app with cargo-bundle
log "Bundling into .app with cargo-bundle"
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
cargo bundle --release --no-default-features --features desktop --target aarch64-apple-darwin

APP_PATH="target/aarch64-apple-darwin/release/bundle/osx/${APP_NAME}.app"
[ -d "$APP_PATH" ] || { echo "❌ Could not find ${APP_NAME}.app at $APP_PATH"; exit 1; }
echo "Found app: $APP_PATH"

log "Syncing bundled assets into app resources"
rm -rf "$APP_PATH/Contents/Resources/assets"
ditto assets "$APP_PATH/Contents/Resources/assets"

for required_asset in \
  "Contents/Resources/assets/main.css" \
  "Contents/Resources/assets/pin-premium.css" \
  "Contents/Resources/assets/lock.png" \
  "Contents/Resources/assets/key_screen_1.png"
do
  [ -f "$APP_PATH/$required_asset" ] || {
    echo "❌ Missing bundled asset: $required_asset"
    echo "   Check [package.metadata.bundle].resources in Cargo.toml"
    exit 1
  }
done

### 3) Entitlements
ENTITLEMENTS="$(pwd)/entitlements.generated.plist"
log "Writing entitlements to $ENTITLEMENTS (SANDBOX=$SANDBOX)"
if [ "$SANDBOX" = "1" ]; then
  # App Sandbox + allow both client & server sockets
  cat > "$ENTITLEMENTS" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.app-sandbox</key><true/>
  <key>com.apple.security.network.client</key><true/>
  <key>com.apple.security.network.server</key><true/>
</dict></plist>
PLIST
else
  # No App Sandbox (Hardened Runtime still enabled via --options runtime)
  cat > "$ENTITLEMENTS" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict></dict></plist>
PLIST
fi

### 4) Ensure Info.plist has CFBundleIdentifier and auto versioning
INFO_PLIST="${APP_PATH}/Contents/Info.plist"

SHORT_VER="$(awk '
  BEGIN{inpkg=0}
  /^\[/{inpkg=($0 ~ /^\[package\]/)}
  inpkg && $0 ~ /^\s*version\s*=/ {
    match($0, /"[^\"]+"/, m); if (m[0]!="") { gsub(/"/,"",m[0]); print m[0]; exit }
  }' Cargo.toml 2>/dev/null || true)"
SHORT_VER="${SHORT_VER:-0.1.0}"
BUILD_NUM="$(git rev-list --count HEAD 2>/dev/null || echo 1)"

if ! /usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$INFO_PLIST" >/dev/null 2>&1; then
  log "Setting CFBundleIdentifier=$BUNDLE_ID"
  /usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string ${BUNDLE_ID}" "$INFO_PLIST"
fi
if /usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$INFO_PLIST" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString ${SHORT_VER}" "$INFO_PLIST"
else
  /usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string ${SHORT_VER}" "$INFO_PLIST"
fi
if /usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$INFO_PLIST" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleVersion ${BUILD_NUM}" "$INFO_PLIST"
else
  /usr/libexec/PlistBuddy -c "Add :CFBundleVersion string ${BUILD_NUM}" "$INFO_PLIST"
fi

### 5) Clear extended attributes
log "Clearing extended attributes on app bundle"
xattr -rc "$APP_PATH" || true

### 6) Codesign internals then outer .app
log "Code-signing internals (executables & libraries) with Hardened Runtime"
# a) Executables (+x)
while IFS= read -r -d '' f; do
  codesign --force --timestamp --options runtime \
           --entitlements "$ENTITLEMENTS" \
           --sign "$IDENTITY" "$f"
done < <(find "$APP_PATH" -type f -perm -111 -print0)

# b) Mach-O libraries (that may not be +x)
while IFS= read -r -d '' f; do
  if file "$f" | grep -q "Mach-O"; then
    codesign --force --timestamp --options runtime \
             --entitlements "$ENTITLEMENTS" \
             --sign "$IDENTITY" "$f"
  fi
done < <(find "$APP_PATH" -type f \( -name "*.dylib" -o -name "*.so" \) -print0)

# c) Framework directories
while IFS= read -r -d '' fw; do
  codesign --force --timestamp --options runtime \
           --entitlements "$ENTITLEMENTS" \
           --sign "$IDENTITY" "$fw"
done < <(find "$APP_PATH" -type d -name "*.framework" -print0)

# d) Finally, sign the app bundle
log "Code-signing outer .app"
codesign --force --timestamp --options runtime \
         --entitlements "$ENTITLEMENTS" \
         --sign "$IDENTITY" "$APP_PATH"

log "Verifying signature (deep/strict)"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

log "Effective entitlements on main executable:"
/usr/bin/codesign -d --entitlements :- "$APP_PATH/Contents/MacOS/$BINARY_NAME" 2>/dev/null || true

### 7) Fresh DMG from the signed app
log "Creating fresh DMG from signed app"
DMG_FILE="${APP_NAME}.dmg"
rm -f "$DMG_FILE"

STAGING="$(mktemp -d)"
VOLNAME="${APP_NAME} ${SHORT_VER:-}"
RW_DMG="$(mktemp -u /private/tmp/${APP_NAME}-rw.XXXXXX.dmg)"
MOUNT_DIR="$(mktemp -d /private/tmp/${APP_NAME}-mount.XXXXXX)"
cleanup_dmg_staging() {
  if mount | grep -q "on ${MOUNT_DIR} "; then
    hdiutil detach "$MOUNT_DIR" -force >/dev/null 2>&1 || true
  fi
  rm -rf "$STAGING" "$MOUNT_DIR"
  rm -f "$RW_DMG"
}
trap cleanup_dmg_staging EXIT

ditto "$APP_PATH" "$STAGING/${APP_NAME}.app"
ln -s /Applications "$STAGING/Applications"

DMG_SIZE_MB="$(du -sm "$STAGING" | awk '{print $1 + 64}')"
hdiutil create -size "${DMG_SIZE_MB}m" -fs HFS+ -volname "$VOLNAME" -ov "$RW_DMG"
hdiutil attach "$RW_DMG" -mountpoint "$MOUNT_DIR" -nobrowse
ditto "$STAGING/${APP_NAME}.app" "$MOUNT_DIR/${APP_NAME}.app"
ln -s /Applications "$MOUNT_DIR/Applications"
hdiutil detach "$MOUNT_DIR"
hdiutil convert "$RW_DMG" -ov -format UDZO -o "$DMG_FILE"

log "Code-signing DMG (optional)"
codesign --force --timestamp --sign "$IDENTITY" "$DMG_FILE" || true

### 8) Store/refresh notarytool credentials
log "Storing/refreshing notarytool credentials profile: $NOTARY_PROFILE"
if [ "$MODE" = "API" ]; then
  [ -f "$API_KEY_PATH" ] || { echo "❌ Missing API_KEY_PATH: $API_KEY_PATH"; exit 1; }
  xcrun notarytool store-credentials "$NOTARY_PROFILE" \
    --key "$API_KEY_PATH" --key-id "$API_KEY_ID" --issuer "$API_ISSUER_ID"
else
  if [ -n "$APPLE_APP_SPECIFIC_PW" ]; then
    xcrun notarytool store-credentials "$NOTARY_PROFILE" \
      --apple-id "$APPLE_ID_EMAIL" --team-id "$TEAM_ID" <<<"$APPLE_APP_SPECIFIC_PW"
  else
    xcrun notarytool store-credentials "$NOTARY_PROFILE" \
      --apple-id "$APPLE_ID_EMAIL" --team-id "$TEAM_ID"
    echo "Paste your APP-SPECIFIC password when prompted."
  fi
fi

### 9) Notarize & wait
log "Submitting DMG for notarization"
xcrun notarytool submit "$DMG_FILE" --keychain-profile "$NOTARY_PROFILE" --wait

### 10) Staple tickets
log "Stapling notarization tickets"
xcrun stapler staple "$DMG_FILE" || true
xcrun stapler staple "$APP_PATH" || true

### 11) Gatekeeper assessment
log "Gatekeeper (spctl) assessment"
spctl --assess --type execute --verbose "$APP_PATH"

### 12) Checksums for release notes
log "Checksums"
shasum -a 256 "$DMG_FILE" | tee "${DMG_FILE}.sha256"

log "✅ Finished. Upload ${DMG_FILE} (and .sha256) to GitHub Releases."
