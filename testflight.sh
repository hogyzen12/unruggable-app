#!/usr/bin/env bash
set -euo pipefail

### --- CONFIG (edit if needed) ----------------------------------------------
: "${PROFILE_PATH_DIST:="$HOME/Downloads/unruggable.mobileprovision"}"   # App Store Connect profile
: "${BUNDLE_ID:="com.unruggable.app"}"                                   # must match your App ID
: "${MARKETING_VERSION:="1.0.0"}"                                        # CFBundleShortVersionString
: "${MIN_IOS:="14.0"}"                                                   # Minimum iOS supported
: "${DIOXUS_ASSET_DIR:=assets}"                                          # set if you bundle assets locally
# Optional: force a specific signing identity (SHA-1). Otherwise auto-pick by TEAM_ID.
DIST_ID_SHA="${DIST_ID_SHA:-}"
# Optional: point to a square PNG to seed the AppIcon set (>=180px recommended; 1024px ideal)
APP_ICON_SRC="${APP_ICON_SRC:-}"
### --------------------------------------------------------------------------

die() { echo "✗ $*" >&2; exit 1; }
step() { echo; echo "→ $*"; }

# Basic checks
command -v cargo >/dev/null        || die "cargo not found"
command -v security >/dev/null     || die "security not found"
command -v plutil >/dev/null       || die "plutil not found"
command -v zip >/dev/null          || die "zip not found"
command -v xcrun >/dev/null        || die "xcrun not found (install Xcode / CLT)"
xcrun --find actool >/dev/null     || die "xcrun actool not found (open Xcode once and accept license)"
[[ -f "$PROFILE_PATH_DIST" ]]      || die "Profile not found: $PROFILE_PATH_DIST"

step "Building Release bundle (arm64)…"
DIOXUS_ASSET_DIR="$DIOXUS_ASSET_DIR" cargo bundle --target aarch64-apple-ios --release

APP_DIR="target/aarch64-apple-ios/release/bundle/ios"
APP="$(find "$APP_DIR" -maxdepth 1 -name "*.app" | head -n1)"
[[ -d "$APP" ]] || die ".app not found under $APP_DIR"
PLIST="$APP/Info.plist"
EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST")"

step "Setting bundle id + marketing/build versions…"
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string $BUNDLE_ID" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $BUNDLE_ID" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $MARKETING_VERSION" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $MARKETING_VERSION" "$PLIST"

# Bump CFBundleVersion intelligently (integer default; override with BUILD_NUMBER)
CUR_BUILD=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$PLIST" 2>/dev/null || echo "")
NEW_BUILD="${BUILD_NUMBER:-}"
if [[ -z "$NEW_BUILD" ]]; then
  if [[ "$CUR_BUILD" =~ ^[0-9]+$ ]]; then
    NEW_BUILD=$((10#$CUR_BUILD + 1))
  else
    NEW_BUILD=1
  fi
fi
/usr/libexec/PlistBuddy -c "Add :CFBundleVersion string $NEW_BUILD" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $NEW_BUILD" "$PLIST"
echo "   CFBundleVersion -> $NEW_BUILD"

# --- Core required keys + icon mappings (drop-in block) ---
/usr/libexec/PlistBuddy -c "Add :MinimumOSVersion string $MIN_IOS" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :MinimumOSVersion $MIN_IOS" "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundlePackageType string APPL' "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :CFBundlePackageType APPL' "$PLIST"
/usr/libexec/PlistBuddy -c 'Delete :CFBundleSupportedPlatforms' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :CFBundleSupportedPlatforms array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleSupportedPlatforms:0 string iPhoneOS' "$PLIST"
/usr/libexec/PlistBuddy -c 'Delete :UIDeviceFamily' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :UIDeviceFamily array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :UIDeviceFamily:0 integer 1' "$PLIST"   # iPhone only
/usr/libexec/PlistBuddy -c 'Delete :UISupportedInterfaceOrientations' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :UISupportedInterfaceOrientations array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :UISupportedInterfaceOrientations:0 string UIInterfaceOrientationPortrait' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :UISupportedInterfaceOrientations:1 string UIInterfaceOrientationLandscapeLeft' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :UISupportedInterfaceOrientations:2 string UIInterfaceOrientationLandscapeRight' "$PLIST"
/usr/libexec/PlistBuddy -c 'Delete :UIRequiredDeviceCapabilities' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :UIRequiredDeviceCapabilities array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :UIRequiredDeviceCapabilities:0 string arm64' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :DTPlatformName string iphoneos' "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :DTPlatformName iphoneos' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :LSRequiresIPhoneOS bool true' "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :LSRequiresIPhoneOS true' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleName string Unruggable' "$PLIST" 2>/dev/null || true

# Asset-catalog icon name (required on iOS 11+)
/usr/libexec/PlistBuddy -c 'Add :CFBundleIconName string AppIcon' "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :CFBundleIconName AppIcon' "$PLIST"

# Legacy icon mappings so Transporter can “see” the 120×120 explicitly
/usr/libexec/PlistBuddy -c 'Delete :CFBundleIcons' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:UIPrerenderedIcon bool false' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:0 string AppIcon60x60' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:1 string AppIcon60x60@2x' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:2 string AppIcon' "$PLIST"

# iPhone-specific variant (some ASC paths check this)
/usr/libexec/PlistBuddy -c 'Delete :CFBundleIcons~iphone' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:UIPrerenderedIcon bool false' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles:0 string AppIcon60x60' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles:1 string AppIcon60x60@2x' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles:2 string AppIcon' "$PLIST"


step "Creating AppIcon asset catalog (AppIcon -> Assets.car)…"
ICON_SRC="$APP_ICON_SRC"
if [[ -z "$ICON_SRC" ]]; then
  for CAND in \
    "$APP/icon_256x256.png" \
    "$APP/assets/icons/256x256.png" \
    "$APP/assets/icon.png" \
    "$APP/assets/icons/icon.png" \
    "assets/icons/256x256.png" \
    "assets/icons/icon.png"; do
    [[ -f "$CAND" ]] && ICON_SRC="$CAND" && break
  done
fi
[[ -f "$ICON_SRC" ]] || die "No source icon found. Set APP_ICON_SRC=/path/to/square_png and re-run."
echo "   Using source icon: $ICON_SRC"

TMP_XC="target/AppAssets.xcassets/AppIcon.appiconset"
rm -rf "target/AppAssets.xcassets" && mkdir -p "$TMP_XC"

# iPhone icon sizes (includes the 120 & 180 Transporter requires)
/usr/bin/sips -s format png -z 40  40  "$ICON_SRC" --out "$TMP_XC/appicon-20@2x.png"  >/dev/null
/usr/bin/sips -s format png -z 60  60  "$ICON_SRC" --out "$TMP_XC/appicon-20@3x.png"  >/dev/null
/usr/bin/sips -s format png -z 58  58  "$ICON_SRC" --out "$TMP_XC/appicon-29@2x.png"  >/dev/null
/usr/bin/sips -s format png -z 87  87  "$ICON_SRC" --out "$TMP_XC/appicon-29@3x.png"  >/dev/null
/usr/bin/sips -s format png -z 80  80  "$ICON_SRC" --out "$TMP_XC/appicon-40@2x.png"  >/dev/null
/usr/bin/sips -s format png -z 120 120 "$ICON_SRC" --out "$TMP_XC/appicon-40@3x.png"  >/dev/null
/usr/bin/sips -s format png -z 120 120 "$ICON_SRC" --out "$TMP_XC/appicon-60@2x.png"  >/dev/null
/usr/bin/sips -s format png -z 180 180 "$ICON_SRC" --out "$TMP_XC/appicon-60@3x.png"  >/dev/null
# ios-marketing (App Store) 1024
/usr/bin/sips -s format png -z 1024 1024 "$ICON_SRC" --out "$TMP_XC/appicon-marketing-1024.png" >/dev/null

cat > "$TMP_XC/Contents.json" <<'JSON'
{
  "images": [
    { "size":"20x20","idiom":"iphone","scale":"2x","filename":"appicon-20@2x.png" },
    { "size":"20x20","idiom":"iphone","scale":"3x","filename":"appicon-20@3x.png" },
    { "size":"29x29","idiom":"iphone","scale":"2x","filename":"appicon-29@2x.png" },
    { "size":"29x29","idiom":"iphone","scale":"3x","filename":"appicon-29@3x.png" },
    { "size":"40x40","idiom":"iphone","scale":"2x","filename":"appicon-40@2x.png" },
    { "size":"40x40","idiom":"iphone","scale":"3x","filename":"appicon-40@3x.png" },
    { "size":"60x60","idiom":"iphone","scale":"2x","filename":"appicon-60@2x.png" },
    { "size":"60x60","idiom":"iphone","scale":"3x","filename":"appicon-60@3x.png" },
    { "size":"1024x1024","idiom":"ios-marketing","scale":"1x","filename":"appicon-marketing-1024.png" }
  ],
  "info": { "version": 1, "author": "xcode" }
}
JSON

xcrun actool "target/AppAssets.xcassets" \
  --compile "$APP" \
  --platform iphoneos \
  --minimum-deployment-target "$MIN_IOS" \
  --app-icon AppIcon \
  --output-partial-info-plist "$APP/assetcatalog_generated_info.plist"

[[ -f "$APP/Assets.car" ]] || die "Assets.car not generated"

# CFBundleIconName (asset-catalog icon name)
/usr/libexec/PlistBuddy -c 'Add :CFBundleIconName string AppIcon' "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :CFBundleIconName AppIcon' "$PLIST"

# Optional: basic UILaunchScreen (allowed for iOS 14+)
if [[ -f "$ICON_SRC" ]]; then
  /usr/bin/sips -s format png -z 1024 1024 "$ICON_SRC" --out "$APP/LaunchScreen.png" >/dev/null || true
  /usr/libexec/PlistBuddy -c 'Delete :UILaunchScreen' "$PLIST" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c 'Add :UILaunchScreen dict' "$PLIST"
  /usr/libexec/PlistBuddy -c 'Add :UILaunchScreen:UIImageName string LaunchScreen' "$PLIST"
  /usr/libexec/PlistBuddy -c 'Add :UILaunchScreen:UIImageRespectsSafeAreaInsets bool true' "$PLIST"
fi

# --- Extra safety: explicit iPhone icons + legacy mapping (helps picky validators) ---
/usr/bin/sips -s format png -z 60  60  "$ICON_SRC" --out "$APP/AppIcon60x60.png"    >/dev/null
/usr/bin/sips -s format png -z 120 120 "$ICON_SRC" --out "$APP/AppIcon60x60@2x.png" >/dev/null
/usr/bin/sips -s format png -z 180 180 "$ICON_SRC" --out "$APP/AppIcon60x60@3x.png" >/dev/null

/usr/libexec/PlistBuddy -c 'Delete :CFBundleIcons' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:UIPrerenderedIcon bool false' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:0 string AppIcon60x60' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:1 string AppIcon' "$PLIST"

# iPhone-only variant (some ASC paths check this key)
/usr/libexec/PlistBuddy -c 'Delete :CFBundleIcons~iphone' "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon dict' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:UIPrerenderedIcon bool false' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles array' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles:0 string AppIcon60x60' "$PLIST"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~iphone:CFBundlePrimaryIcon:CFBundleIconFiles:1 string AppIcon' "$PLIST"

step "Stamping DT* keys from local Xcode/SDK…"
XCODE_VER=$(xcodebuild -version | awk '/Xcode/ {print $2}')
XCODE_BUILD=$(xcodebuild -version | awk '/Build version/ {print $3}')
SDK_VER=$(xcrun --sdk iphoneos --show-sdk-version)
SDK_BUILD=$(xcrun --sdk iphoneos --show-sdk-build-version 2>/dev/null || echo "")
PLAT_VER=$(xcrun --sdk iphoneos --show-sdk-platform-version 2>/dev/null || echo "$SDK_VER")
OS_BUILD=$(sw_vers -buildVersion)
MAJOR=${XCODE_VER%%.*}; MINOR=${XCODE_VER#*.}; MINOR=${MINOR%%.*}; [[ "$MINOR" =~ ^[0-9]+$ ]] || MINOR=0
DTXCODE=$((10#$MAJOR*100 + 10#$MINOR*10))
/usr/libexec/PlistBuddy -c "Add :DTXcode string $DTXCODE" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :DTXcode $DTXCODE" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :DTXcodeBuild string $XCODE_BUILD" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :DTXcodeBuild $XCODE_BUILD" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :DTPlatformVersion string $PLAT_VER" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :DTPlatformVersion $PLAT_VER" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :DTSDKName string iphoneos$SDK_VER" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :DTSDKName iphoneos$SDK_VER" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :DTSDKBuild string $SDK_BUILD" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :DTSDKBuild $SDK_BUILD" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :DTPlatformBuild string $SDK_BUILD" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :DTPlatformBuild $SDK_BUILD" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :BuildMachineOSBuild string $OS_BUILD" "$PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Set :BuildMachineOSBuild $OS_BUILD" "$PLIST"

step "Removing dev ATS overrides…"
/usr/libexec/PlistBuddy -c 'Delete :NSAppTransportSecurity' "$PLIST" 2>/dev/null || true

step "Parsing provisioning profile…"
TEAM_ID=$(security cms -D -i "$PROFILE_PATH_DIST" | plutil -extract TeamIdentifier.0 raw -o - -)
APP_ID=$(  security cms -D -i "$PROFILE_PATH_DIST" | plutil -extract Entitlements.application-identifier raw -o - -)
K_GROUP=$( security cms -D -i "$PROFILE_PATH_DIST" | plutil -extract Entitlements.keychain-access-groups.0 raw -o - -)
GET_TASK=$(security cms -D -i "$PROFILE_PATH_DIST" | plutil -extract Entitlements.get-task-allow raw -o - - 2>/dev/null || echo false)
[[ "$GET_TASK" = "false" ]] || die "Profile is not App Store/TF (get-task-allow must be false)"
case "$APP_ID" in
  *".$BUNDLE_ID"|"$TEAM_ID.$BUNDLE_ID") : ;;
  *) die "Profile App ID ($APP_ID) does not match bundle id ($BUNDLE_ID)";;
esac

step "Embedding provisioning profile…"
cp "$PROFILE_PATH_DIST" "$APP/embedded.mobileprovision"

step "Writing distribution entitlements…"
mkdir -p target
cat > target/entitlements.dist.xcent <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>application-identifier</key><string>${APP_ID}</string>
  <key>com.apple.developer.team-identifier</key><string>${TEAM_ID}</string>
  <key>keychain-access-groups</key><array><string>${K_GROUP}</string></array>
  <key>get-task-allow</key><false/>
</dict></plist>
EOF

step "Selecting Distribution signing identity…"
if [[ -z "$DIST_ID_SHA" ]]; then
  LINE=$(security find-identity -v -p codesigning | awk -v team="$TEAM_ID" '/Apple Distribution:/ && $0 ~ team {print; exit}')
  [[ -z "$LINE" ]] && LINE=$(security find-identity -v -p codesigning | grep 'Apple Distribution' | head -n1 || true)
  DIST_ID_SHA=$(printf '%s' "$LINE" | grep -oE '[A-F0-9]{40}')
fi
[[ -n "$DIST_ID_SHA" ]] || die "No Apple Distribution identity found"
echo "   Using identity: $DIST_ID_SHA"

step "Codesigning app…"
codesign --force --timestamp \
  --entitlements target/entitlements.dist.xcent \
  --sign "$DIST_ID_SHA" \
  "$APP"

# Ensure CodeResources symlink exists *after* codesign (ASC checks this)
( cd "$APP" && rm -f CodeResources && ln -hfs "_CodeSignature/CodeResources" "CodeResources" )

step "Verifying signature & arch…"
codesign --verify --strict --deep "$APP"
codesign -dv --entitlements :- "$APP" >/dev/null 2>&1 || true
lipo -info "$APP/$EXECUTABLE" || true
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$PLIST" || true
/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$PLIST" || true
/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$PLIST" || true
/usr/libexec/PlistBuddy -c 'Print :CFBundleIconName' "$PLIST" || true
/usr/libexec/PlistBuddy -c 'Print :MinimumOSVersion' "$PLIST" || true
/usr/libexec/PlistBuddy -c 'Print :DTPlatformName' "$PLIST" || true

step "Packaging IPA…"
WORK="target/ipa"
rm -rf "$WORK" && mkdir -p "$WORK/Payload"
cp -R "$APP" "$WORK/Payload/"
( cd "$WORK" && zip -y -r Unruggable.ipa Payload >/dev/null )
IPA="$WORK/Unruggable.ipa"

echo
echo "✓ IPA ready: $IPA"
echo "   Open Transporter and upload the IPA. (After processing, enable TestFlight.)"
