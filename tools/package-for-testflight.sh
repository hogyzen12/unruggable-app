#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

# =============================
# Unruggable: Deterministic TestFlight packager
# =============================

# ---- APP METADATA ----
APP_NAME="unruggable"
BUNDLE_ID="com.unruggable.app"
MARKETING_VERSION="${MARKETING_VERSION:-1.0.0}"
BUILD_NUMBER="${BUILD_NUMBER:-}"          # auto-increment if empty
MIN_IOS="${MIN_IOS:-12.0}"

# ---- HARD-CODED SIGNING ASSETS (YOUR REAL ONES) ----
PROFILE_PATH="$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles/f505bb28-7305-4b0b-98f6-f0cd402b599f.mobileprovision"
P12_PATH="/Users/hogyzen12/Apple_Distribution_unruggable.p12"   # << set this
P12_PASSWORD="${P12_PASSWORD:-YOURPASSWORD HERE}"

# The exact Apple Distribution certificate SHA-1 you showed:
EXPECTED_CERT_SHA1="091FDBB65071C0DA236CF44B2229330B95AD814F"

# Private keychain just for this build
KEYCHAIN_PATH="$HOME/Library/Keychains/unruggable-dist.keychain-db"
KEYCHAIN_PASSWORD="${KEYCHAIN_PASSWORD:-your-password-here}"
ICON_PATH="/Users/hogyzen12/coding-project-folders/unruggable-app/assets/icon.png"  # Renamed for clarity (single file)

# Optional Transporter creds (pick ONE method):
# 1) App Store Connect API key:
APPSTORE_API_KEY_ID="${APPSTORE_API_KEY_ID:-}"         # e.g. ABCDE12345
APPSTORE_API_ISSUER_ID="${APPSTORE_API_ISSUER_ID:-}"   # GUID from App Store Connect
# 2) Or Apple ID + app-specific password:
APPLE_ID="${APPLE_ID:-}"                               
APP_SPECIFIC_PASSWORD="${APP_SPECIFIC_PASSWORD:-}"

# ---- Derived paths ----
APP_PATH="target/aarch64-apple-ios/release/bundle/ios/${APP_NAME}.app"
ENT_PATH="target/entitlements-dist.xcent"
TMP_DIR="$(mktemp -d -t unruggable_build_XXXXXX)"
PROFILE_PLIST="${TMP_DIR}/profile.plist"

die()  { echo "❌ $*" >&2; exit 1; }
ok()   { echo "✅ $*"; }
info() { echo "— $*"; }
need_cmd(){ command -v "$1" >/dev/null 2>&1 || die "Missing command: $1"; }

cleanup() {
  security delete-keychain "$KEYCHAIN_PATH" >/dev/null 2>&1 || true
  rm -rf "$TMP_DIR" Payload
}
trap cleanup EXIT

# ---- Tooling sanity ----
for c in cargo security /usr/libexec/PlistBuddy plutil openssl xcodebuild xcrun ditto sips magick; do need_cmd "$c"; done  # Added 'convert' check for ImageMagick


# Must be a release/RC Xcode (not Beta path)
XCODE_PATH="$(xcode-select -p || true)"
[[ "$XCODE_PATH" != *"Beta.app"* ]] || die "Using Xcode Beta at: $XCODE_PATH. Install/select RC/GM."

# ---- Assets sanity ----
[[ -f "$PROFILE_PATH" ]] || die "Missing provisioning profile: $PROFILE_PATH"
[[ -f "$P12_PATH"     ]] || die "Missing .p12: $P12_PATH"
ok "Found profile and .p12"

# ---- Private keychain setup ----
if [[ ! -f "$KEYCHAIN_PATH" ]]; then
  info "Creating private keychain…"
  security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
  security set-keychain-settings -l -u -t 3600 "$KEYCHAIN_PATH"
fi
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"

# Put our keychain first in the search list to avoid ambiguity
security list-keychains -s "$KEYCHAIN_PATH" login.keychain-db System.keychain >/dev/null

# Import .p12 and allow codesign/security to use it
security import "$P12_PATH" -k "$KEYCHAIN_PATH" -P "$P12_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security >/dev/null
security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH" >/dev/null
ok "Imported signing identity into private keychain"

# ---- Decode profile; verify bundle & that it contains EXPECTED_CERT_SHA1 ----
security cms -D -i "$PROFILE_PATH" > "$PROFILE_PLIST" || die "Failed to decode provisioning profile"

TEAM_ID=$(/usr/libexec/PlistBuddy -c 'Print :TeamIdentifier:0' "$PROFILE_PLIST") || die "Profile missing TeamIdentifier"
APPID=$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:application-identifier' "$PROFILE_PLIST") || die "Profile missing application-identifier"
DERIVED_BUNDLE_ID="${APPID#${TEAM_ID}.}"
[[ "$DERIVED_BUNDLE_ID" == "$BUNDLE_ID" ]] || die "Profile bundleId=${DERIVED_BUNDLE_ID} != ${BUNDLE_ID}"

# Must be App Store (no device UDIDs)
if /usr/libexec/PlistBuddy -c 'Print :ProvisionedDevices' "$PROFILE_PLIST" >/dev/null 2>&1; then
  die "Profile has ProvisionedDevices (Dev/Ad Hoc). Need App Store profile."
fi

# Extract DeveloperCertificates fingerprints from profile
found_match=""
i=0
while /usr/libexec/PlistBuddy -c "Print :DeveloperCertificates:${i}" "$PROFILE_PLIST" >/dev/null 2>&1; do
  CERT_DER="$TMP_DIR/devcert_${i}.der"
  /usr/libexec/PlistBuddy -c "Print :DeveloperCertificates:${i}" "$PROFILE_PLIST" > "$CERT_DER"
  FP=$(openssl x509 -inform der -in "$CERT_DER" -noout -fingerprint -sha1 \
    | awk -F= '/Fingerprint/ {gsub(":","",$2); print toupper($2)}')
  if [[ "$FP" == "$EXPECTED_CERT_SHA1" ]]; then found_match="yes"; fi
  i=$((i+1))
done
[[ -n "$found_match" ]] || die "Profile does NOT include cert $EXPECTED_CERT_SHA1. Recreate profile for this certificate."

# Ensure the identity actually exists in our keychain
if ! security find-identity -v -p codesigning "$KEYCHAIN_PATH" | grep -q "$EXPECTED_CERT_SHA1"; then
  die "Keychain does not contain identity $EXPECTED_CERT_SHA1 (did import fail?)."
fi
ok "Profile ↔︎ Certificate match confirmed ($EXPECTED_CERT_SHA1)"
info "Team: $TEAM_ID  BundleID: $BUNDLE_ID"

# ---- Build (aarch64-apple-ios) ----
info "Building release app…"
cargo bundle --target aarch64-apple-ios --release
[[ -d "$APP_PATH" ]] || die "Expected app not found at $APP_PATH"
ok "Built: $APP_PATH"

# ---- Stamp versions ----
APP_INFO_PLIST="${APP_PATH}/Info.plist"
[[ -n "${MARKETING_VERSION}" ]] || die "MARKETING_VERSION empty"

/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString ${MARKETING_VERSION}" "$APP_INFO_PLIST" \
  || /usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string ${MARKETING_VERSION}" "$APP_INFO_PLIST"

if [[ -n "$BUILD_NUMBER" ]]; then
  NEW_BUILD="$BUILD_NUMBER"
else
  CUR_BUILD=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP_INFO_PLIST" 2>/dev/null || echo "")
  if [[ "$CUR_BUILD" =~ ^[0-9]+$ ]]; then NEW_BUILD=$((CUR_BUILD + 1)); else NEW_BUILD=1; fi
fi
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion ${NEW_BUILD}" "$APP_INFO_PLIST" \
  || /usr/libexec/PlistBuddy -c "Add :CFBundleVersion string ${NEW_BUILD}" "$APP_INFO_PLIST"
ok "Stamped versions → CFBundleShortVersionString=${MARKETING_VERSION}  CFBundleVersion=${NEW_BUILD}"

# ---- App Icon (asset catalog) ----
# Absolute path to your 256x256 PNG:
MASTER_ICON_SRC="${MASTER_ICON_SRC:-/Users/hogyzen12/coding-project-folders/unruggable-app/assets/icon.png}"
[[ -f "$MASTER_ICON_SRC" ]] || die "Missing base app icon at $MASTER_ICON_SRC"

ASSETS_DIR="$TMP_DIR/Assets.xcassets"
ICON_DIR="$ASSETS_DIR/AppIcon.appiconset"
mkdir -p "$ICON_DIR"

# Generate required iPhone icon sizes + iOS marketing
sips -s format png -Z 120  "$MASTER_ICON_SRC" --out "$ICON_DIR/AppIcon-60@2x.png"  >/dev/null
sips -s format png -Z 180  "$MASTER_ICON_SRC" --out "$ICON_DIR/AppIcon-60@3x.png"  >/dev/null
sips -s format png -Z 1024 "$MASTER_ICON_SRC" --out "$ICON_DIR/AppIcon-1024.png"   >/dev/null

# Minimal Contents.json for iPhone + ios-marketing
cat > "$ICON_DIR/Contents.json" <<'JSON'
{
  "images": [
    { "size": "60x60",     "idiom": "iphone",        "filename": "AppIcon-60@2x.png", "scale": "2x" },
    { "size": "60x60",     "idiom": "iphone",        "filename": "AppIcon-60@3x.png", "scale": "3x" },
    { "size": "1024x1024", "idiom": "ios-marketing", "filename": "AppIcon-1024.png",  "scale": "1x" }
  ],
  "info": { "version": 1, "author": "xcode" }
}
JSON

# Flatten a PNG to an opaque PNG (alpha removed) using ImageMagick
flatten_to_opaque_png() {
  local src="$1"; local out="$2"
  magick "$src" -background white -alpha remove -alpha off "$out"
}

# --- Make every generated icon opaque (no alpha allowed by App Store) ---
# (Moved here, after generation, targeting the icon dir)
for P in "$ICON_DIR"/*.png; do
  flatten_to_opaque_png "$P" "$P"
done

ACTOOL_LOG="$TMP_DIR/actool.log"
PARTIAL_PLIST="$TMP_DIR/asset-partial.plist"

# Compile the asset catalog DIRECTLY into the .app (produces <app>/Assets.car)
# and emit a partial Info.plist we will merge.
xcrun actool \
  --output-format human-readable-text \
  --notices --warnings \
  --platform iphoneos \
  --product-type com.apple.product-type.application \
  --target-device iphone \
  --minimum-deployment-target "$MIN_IOS" \
  --app-icon "AppIcon" \
  --output-partial-info-plist "$PARTIAL_PLIST" \
  --compress-pngs \
  --compile "$APP_PATH" "$ASSETS_DIR" > "$ACTOOL_LOG" 2>&1 \
  || { echo "----- actool output -----"; cat "$ACTOOL_LOG" >&2; die "actool failed"; }

# Verify Assets.car ended up in the app bundle
[[ -f "$APP_PATH/Assets.car" ]] || { echo "----- actool output -----"; cat "$ACTOOL_LOG" >&2; die "Assets.car not produced by actool"; }

# Merge actool’s partial Info (adds CFBundleIcons/CFBundlePrimaryIcon, etc.)
/usr/libexec/PlistBuddy -c "Merge $PARTIAL_PLIST" "$APP_INFO_PLIST" || true

# Force CFBundleIconName to AppIcon (Transporter checks this key explicitly)
/usr/libexec/PlistBuddy -c "Add :CFBundleIconName string AppIcon" "$APP_INFO_PLIST" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Set :CFBundleIconName AppIcon" "$APP_INFO_PLIST"

# Keep CFBundleIcons that actool added; just remove legacy keys if present
/usr/libexec/PlistBuddy -c "Delete :CFBundleIconFiles"  "$APP_INFO_PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Delete :CFBundleIcons~ipad" "$APP_INFO_PLIST" 2>/dev/null || true

ok "App icon asset catalog compiled, merged, and CFBundleIconName set"

# ---- Stamp DT* keys so App Store Connect knows what built this ----
XCODE_VER="$(/usr/bin/xcodebuild -version | awk '/Xcode/ {print $2}')"
XCODE_BUILD="$(/usr/bin/xcodebuild -version | awk '/Build version/ {print $3}')"
SDK_VER="$(/usr/bin/xcrun --sdk iphoneos --show-sdk-version)"                # e.g. 18.5
SDK_BUILD="$(/usr/bin/xcrun --sdk iphoneos --show-sdk-build-version)"        # e.g. 22F76
# DTXcode is “major+minor(without dot)+0”, e.g. 16.4 -> 1640
DTXCODE="${XCODE_VER//./}0"

set_plist() {
  local key="$1" val="$2"
  /usr/libexec/PlistBuddy -c "Set :$key $val" "$APP_INFO_PLIST" \
    || /usr/libexec/PlistBuddy -c "Add :$key string $val" "$APP_INFO_PLIST"
}

set_plist DTPlatformName     "iphoneos"
set_plist DTPlatformVersion  "$SDK_VER"
set_plist DTPlatformBuild    "$SDK_BUILD"
set_plist DTSDKName          "iphoneos${SDK_VER}"
set_plist DTSDKBuild         "$SDK_BUILD"
set_plist DTXcode            "$DTXCODE"
set_plist DTXcodeBuild       "$XCODE_BUILD"

# Ensure Bundle ID
CUR_BID=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP_INFO_PLIST" 2>/dev/null || echo "")
if [[ "$CUR_BID" != "$BUNDLE_ID" ]]; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier ${BUNDLE_ID}" "$APP_INFO_PLIST" \
    || /usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string ${BUNDLE_ID}" "$APP_INFO_PLIST"
  info "Set CFBundleIdentifier to ${BUNDLE_ID}"
fi

# Required metadata (avoid manual DT* keys)
/usr/libexec/PlistBuddy -c "Set :MinimumOSVersion ${MIN_IOS}" "$APP_INFO_PLIST" \
  || /usr/libexec/PlistBuddy -c "Add :MinimumOSVersion string ${MIN_IOS}" "$APP_INFO_PLIST"
/usr/libexec/PlistBuddy -c "Set :CFBundlePackageType APPL" "$APP_INFO_PLIST" \
  || /usr/libexec/PlistBuddy -c "Add :CFBundlePackageType string APPL" "$APP_INFO_PLIST"
/usr/libexec/PlistBuddy -c "Delete :CFBundleSupportedPlatforms" "$APP_INFO_PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms array" "$APP_INFO_PLIST"
/usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms:0 string iPhoneOS" "$APP_INFO_PLIST"

# Device capabilities & family
if ! /usr/libexec/PlistBuddy -c "Print :UIRequiredDeviceCapabilities" "$APP_INFO_PLIST" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Add :UIRequiredDeviceCapabilities array" "$APP_INFO_PLIST"
fi
if ! /usr/libexec/PlistBuddy -c "Print :UIRequiredDeviceCapabilities" "$APP_INFO_PLIST" | grep -q "arm64"; then
  /usr/libexec/PlistBuddy -c "Add :UIRequiredDeviceCapabilities:0 string arm64" "$APP_INFO_PLIST" 2>/dev/null || \
  /usr/libexec/PlistBuddy -c "Add :UIRequiredDeviceCapabilities string arm64" "$APP_INFO_PLIST"
fi
if ! /usr/libexec/PlistBuddy -c "Print :UIDeviceFamily" "$APP_INFO_PLIST" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Add :UIDeviceFamily array" "$APP_INFO_PLIST"
  /usr/libexec/PlistBuddy -c "Add :UIDeviceFamily:0 integer 1" "$APP_INFO_PLIST" # iPhone
fi

# ---- Minimal distribution entitlements ----
cat > "$ENT_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>application-identifier</key>
  <string>${TEAM_ID}.${BUNDLE_ID}</string>
  <key>com.apple.developer.team-identifier</key>
  <string>${TEAM_ID}</string>
  <key>keychain-access-groups</key>
  <array>
    <string>${TEAM_ID}.${BUNDLE_ID}</string>
  </array>
</dict></plist>
EOF
ok "Wrote entitlements → $ENT_PATH"

# ---- Embed provisioning profile ----
cp "$PROFILE_PATH" "$APP_PATH/embedded.mobileprovision"
ok "Embedded provisioning profile"

# ---- Clean any dev ATS relaxations ----
/usr/libexec/PlistBuddy -c 'Delete :NSAppTransportSecurity' "$APP_INFO_PLIST" 2>/dev/null || true

# ---- Signing helpers ----
sign_item() {
  local path="$1"
  [[ -e "$path" ]] || return 0
  info "Signing: $path"
  codesign --force --sign "$EXPECTED_CERT_SHA1" --keychain "$KEYCHAIN_PATH" --timestamp=none "$path"
}

# ---- Sign nested code first ----
if [[ -d "$APP_PATH/Frameworks" ]]; then
  while IFS= read -r -d '' fw;    do sign_item "$fw";    done < <(find "$APP_PATH/Frameworks" -type d -name "*.framework" -print0)
  while IFS= read -r -d '' dylib; do sign_item "$dylib"; done < <(find "$APP_PATH/Frameworks" -type f -name "*.dylib" -print0)
fi
if [[ -d "$APP_PATH/PlugIns" ]]; then
  while IFS= read -r -d '' appex; do sign_item "$appex"; done < <(find "$APP_PATH/PlugIns" -type d -name "*.appex" -print0)
fi

# ---- Sign main app with entitlements ----
codesign --force --sign "$EXPECTED_CERT_SHA1" --keychain "$KEYCHAIN_PATH" --entitlements "$ENT_PATH" --timestamp=none "$APP_PATH"

# ---- Verify signature & no get-task-allow ----
codesign -vvv --strict --deep "$APP_PATH" || die "codesign strict verification failed"
if codesign -d --entitlements :- "$APP_PATH" 2>/dev/null | grep -q 'get-task-allow'; then
  die "Entitlements contain get-task-allow; distribution builds must NOT include it."
fi
ok "Code signing complete with identity $EXPECTED_CERT_SHA1"

# ---- Package IPA ----
IPA_NAME="Unruggable-${MARKETING_VERSION}-${NEW_BUILD}.ipa"
rm -rf Payload "$IPA_NAME"
mkdir -p Payload
cp -R "$APP_PATH" Payload/
/usr/bin/ditto -c -k --sequesterRsrc --keepParent Payload "$IPA_NAME"
rm -rf Payload
ok "Created IPA → $(pwd)/${IPA_NAME}"

# ---- Optional upload via iTMSTransporter ----
if [[ -n "$APPSTORE_API_KEY_ID" && -n "$APPSTORE_API_ISSUER_ID" && -n "${APPSTORE_API_PRIVATE_KEY:-}" ]]; then
  info "Uploading via iTMSTransporter (API key)…"
  xcrun iTMSTransporter -m upload -assetFile "$IPA_NAME" \
    -apiKey "$APPSTORE_API_KEY_ID" -apiIssuer "$APPSTORE_API_ISSUER_ID" \
    -apiKeyFile "$APPSTORE_API_PRIVATE_KEY" \
    -v informational
  ok "Upload requested. Check App Store Connect → TestFlight."
elif [[ -n "$APPLE_ID" && -n "$APP_SPECIFIC_PASSWORD" ]]; then
  info "Uploading via iTMSTransporter (Apple ID)…"
  xcrun iTMSTransporter -m upload -assetFile "$IPA_NAME" \
    -u "$APPLE_ID" -p "$APP_SPECIFIC_PASSWORD" -v informational
  ok "Upload requested. Check App Store Connect → TestFlight."
else
  info "Upload skipped. Drag '${IPA_NAME}' into Transporter."
fi

echo -e "\n🎉 Done."