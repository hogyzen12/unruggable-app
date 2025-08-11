# get the mobile provisioning profile
export APP_DEV_NAME=$(xcrun security find-identity -v -p codesigning | grep "Apple Development: " | sed -E 's/.*"([^"]+)".*/\1/')

# Find the provisioning profile from ~/Library/MobileDevice/Provisioning\ Profiles
export PROVISION_FILE=$(ls ~/Library/Developer/Xcode/UserData/Provisioning\ Profiles | grep mobileprovision)

# Convert the provisioning profile to json so we can use jq to extract the important bits
security cms -D \
	-i ~/Library/Developer/Xcode/UserData/Provisioning\ Profiles/${PROVISION_FILE} | \
	python3 -c 'import plistlib,sys,json; print(json.dumps(plistlib.loads(sys.stdin.read().encode("utf-8")), default=lambda o:"<not serializable>"))' \
	> target/provisioning.json

# jq out the important bits of the provisioning profile
export TEAM_IDENTIFIER=$(jq -r '.TeamIdentifier[0]' target/provisioning.json)
export APPLICATION_IDENTIFIER_PREFIX=$(jq -r '.ApplicationIdentifierPrefix[0]' target/provisioning.json)
export APPLICATION_IDENTIFIER=$(jq -r '.Entitlements."application-identifier"' target/provisioning.json)
export APP_ID_ACCESS_GROUP=$(jq -r '.Entitlements."keychain-access-groups"[0]' target/provisioning.json)

# now build the entitlements file
cat <<EOF > target/entitlements.xcent
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
		<key>application-identifier</key>
		<string>${APPLICATION_IDENTIFIER}</string>
		<key>keychain-access-groups</key>
		<array>
			<string>${APP_ID_ACCESS_GROUP}.*</string>
		</array>
		<key>get-task-allow</key>
		<true/>
		<key>com.apple.developer.team-identifier</key>
		<string>${TEAM_IDENTIFIER}</string>
</dict></plist>
EOF

# ----- Add ATS (App Transport Security) for dev -----
APP_PATH="target/aarch64-apple-ios/debug/bundle/ios/unruggable.app"
#APP_PATH="target/dx/unruggable/debug/ios/Unruggable.app"
APP_PLIST="$APP_PATH/Info.plist"

# Create NSAppTransportSecurity dict if missing
/usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity dict' "$APP_PLIST" 2>/dev/null || true

# Easiest dev-time setting: allow web content & local networking in WKWebView
/usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSAllowsArbitraryLoadsInWebContent bool true' "$APP_PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :NSAppTransportSecurity:NSAllowsArbitraryLoadsInWebContent true' "$APP_PLIST"

/usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSAllowsLocalNetworking bool true' "$APP_PLIST" 2>/dev/null || \
/usr/libexec/PlistBuddy -c 'Set :NSAppTransportSecurity:NSAllowsLocalNetworking true' "$APP_PLIST"

# (Optional) If you want a targeted exception instead of broad allow, uncomment this block:
# /usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSExceptionDomains dict' "$APP_PLIST" 2>/dev/null || true
# /usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSExceptionDomains:cdn.jsdelivr.net dict' "$APP_PLIST" 2>/dev/null || true
# /usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSExceptionDomains:cdn.jsdelivr.net:NSIncludesSubdomains bool true' "$APP_PLIST" 2>/dev/null || true
# /usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSExceptionDomains:raw.githubusercontent.com dict' "$APP_PLIST" 2>/dev/null || true
# /usr/libexec/PlistBuddy -c 'Add :NSAppTransportSecurity:NSExceptionDomains:raw.githubusercontent.com:NSIncludesSubdomains bool true' "$APP_PLIST" 2>/dev/null || true
# ----------------------------------------------------


# sign the app
codesign --force \
	--entitlements target/entitlements.xcent \
	--sign "${APP_DEV_NAME}" \
	target/aarch64-apple-ios/debug/bundle/ios/unruggable.app
