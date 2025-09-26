#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# Env
# Source the environment file only if not in a CI environment
if [ -z "${CI-}" ]; then
  SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
  source "$SCRIPT_DIR/android.env"
fi

# --- Configuration ---
APP_NAME="unruggable"
# Relative path to the root of the Dioxus-generated Android project
ANDROID_PROJECT_DIR="target/dx/${APP_NAME}/release/android"
# Relative path to the desired output directory for the AAB
OUTPUT_DIR="dist/android"
# The final name for the AAB file
FINAL_AAB_NAME="${APP_NAME}-release.aab"
# Path to the source icon (largest size from your Dioxus.toml assets; adjust if needed)
SOURCE_ICON_PATH="assets/icons/icon.png"
# --- End Configuration ---

# Get the absolute path to the project root (where the script is located)
PROJECT_ROOT=$(pwd)

# Absolute paths based on the project root
ABS_ANDROID_PROJECT_DIR="$PROJECT_ROOT/$ANDROID_PROJECT_DIR"
ABS_OUTPUT_DIR="$PROJECT_ROOT/$OUTPUT_DIR"
ABS_FINAL_AAB_PATH="$ABS_OUTPUT_DIR/$FINAL_AAB_NAME"
ABS_SOURCE_ICON="$PROJECT_ROOT/$SOURCE_ICON_PATH"

# Check if the Android project directory exists
if [ ! -d "$ABS_ANDROID_PROJECT_DIR" ]; then
  echo "Error: Android project directory not found at $ABS_ANDROID_PROJECT_DIR"
  echo "Please ensure you have run 'dx build --platform android' at least once."
  exit 1
fi

echo "Navigating to Android project root: $ABS_ANDROID_PROJECT_DIR"
cd "$ABS_ANDROID_PROJECT_DIR"

# Navigate into the actual Gradle project directory
echo "Navigating into Gradle project: app"
cd "app"

echo "Patching Gradle/SDK versions to target API 35..."

# Top-level (we're in android/app)
TOP_BUILD_GRADLE="build.gradle"
TOP_SETTINGS_GRADLE_KTS="settings.gradle.kts"
TOP_SETTINGS_GRADLE="settings.gradle"
WRAPPER_PROPS="gradle/wrapper/gradle-wrapper.properties"

# Module-level (android/app/app)
MOD_DIR="app"
MOD_GRADLE_GROOVY="$MOD_DIR/build.gradle"
MOD_GRADLE_KTS="$MOD_DIR/build.gradle.kts"

# 1) Update module compile/target/min SDK (Groovy)
if [ -f "$MOD_GRADLE_GROOVY" ]; then
  echo "Updating $MOD_GRADLE_GROOVY (Groovy)..."
  # compile/target forms: 'compileSdkVersion 33' or 'compileSdk 33'
  sed -i '' -E 's/(compileSdkVersion|compileSdk)[[:space:]]+[0-9]+/\1 35/g' "$MOD_GRADLE_GROOVY"
  sed -i '' -E 's/(targetSdkVersion|targetSdk)[[:space:]]+[0-9]+/\1 35/g' "$MOD_GRADLE_GROOVY"
  # optional: minSdk >= 24
  sed -i '' -E 's/(minSdkVersion|minSdk)[[:space:]]+[0-9]+/\1 24/g' "$MOD_GRADLE_GROOVY"
fi

# 2) Update module compile/target/min SDK (KTS)
if [ -f "$MOD_GRADLE_KTS" ]; then
  echo "Updating $MOD_GRADLE_KTS (KTS)..."
  sed -i '' -E 's/compileSdk[[:space:]]*=[[:space:]]*[0-9]+/compileSdk = 35/g' "$MOD_GRADLE_KTS"
  sed -i '' -E 's/targetSdk[[:space:]]*=[[:space:]]*[0-9]+/targetSdk = 35/g' "$MOD_GRADLE_KTS"
  sed -i '' -E 's/minSdk[[:space:]]*=[[:space:]]*[0-9]+/minSdk = 24/g' "$MOD_GRADLE_KTS"
fi

# 3) Bump Android Gradle Plugin to 8.6.0 (plugins DSL or buildscript)
if [ -f "$TOP_SETTINGS_GRADLE_KTS" ]; then
  echo "Updating $TOP_SETTINGS_GRADLE_KTS..."
  sed -i '' -E 's/id\\("com\\.android\\.application"\\) version "[0-9.]+"\\)/id("com.android.application") version "8.6.0")/g' "$TOP_SETTINGS_GRADLE_KTS"
  sed -i '' -E 's/id\\("com\\.android\\.library"\\) version "[0-9.]+"\\)/id("com.android.library") version "8.6.0")/g' "$TOP_SETTINGS_GRADLE_KTS"
fi
if [ -f "$TOP_BUILD_GRADLE" ]; then
  echo "Updating $TOP_BUILD_GRADLE (classpath fallback)..."
  sed -i '' -E 's/com\\.android\\.tools\\.build:gradle:[0-9.]+/com.android.tools.build:gradle:8.6.0/g' "$TOP_BUILD_GRADLE"
fi
if [ -f "$TOP_SETTINGS_GRADLE" ]; then
  echo "Updating $TOP_SETTINGS_GRADLE..."
  sed -i '' -E 's/id\\("com\\.android\\.application"\\) version "[0-9.]+"\\)/id("com.android.application") version "8.6.0")/g' "$TOP_SETTINGS_GRADLE"
  sed -i '' -E 's/id\\("com\\.android\\.library"\\) version "[0-9.]+"\\)/id("com.android.library") version "8.6.0")/g' "$TOP_SETTINGS_GRADLE"
fi

# 4) Ensure Gradle wrapper is compatible (8.7)
if [ -f "$WRAPPER_PROPS" ]; then
  echo "Updating $WRAPPER_PROPS..."
  sed -i '' -E 's#distributionUrl=.*gradle-[0-9.]+-all\\.zip#distributionUrl=https\\://services.gradle.org/distributions/gradle-8.7-all.zip#g' "$WRAPPER_PROPS"
fi

echo "Patch complete."

echo "Ensuring unique versionCode/versionName..."

MOD_DIR="app"
GROOVY="$MOD_DIR/build.gradle"
KTS="$MOD_DIR/build.gradle.kts"

# Use env overrides if provided; otherwise use date-based code (YYMMDDHH) — always increasing & < 2,147,483,647
VERSION_CODE="${VERSION_CODE:-$(date +%y%m%d%H)}"
VERSION_NAME="${VERSION_NAME:-0.1.$VERSION_CODE}"

if [ -f "$GROOVY" ]; then
  echo "Patching $GROOVY..."
  # Insert defaultConfig block if missing
  grep -q "defaultConfig" "$GROOVY" || sed -i '' -E $'s/android \\{/android {\\n    defaultConfig {\\n    }/g' "$GROOVY"
  # versionCode
  if grep -qE 'versionCode[[:space:]]+[0-9]+' "$GROOVY"; then
    sed -i '' -E "s/versionCode[[:space:]]+[0-9]+/versionCode ${VERSION_CODE}/g" "$GROOVY"
  else
    sed -i '' -E "s/defaultConfig \\{/defaultConfig {\\n        versionCode ${VERSION_CODE}/" "$GROOVY"
  fi
  # versionName
  if grep -qE 'versionName[[:space:]]+"[^"]+"' "$GROOVY"; then
    sed -i '' -E "s/versionName[[:space:]]+\"[^\"]+\"/versionName \"${VERSION_NAME}\"/g" "$GROOVY"
  else
    sed -i '' -E "s/versionCode ${VERSION_CODE}/versionCode ${VERSION_CODE}\\n        versionName \"${VERSION_NAME}\"/" "$GROOVY"
  fi
fi

if [ -f "$KTS" ]; then
  echo "Patching $KTS..."
  grep -q "defaultConfig" "$KTS" || sed -i '' -E $'s/android \\{/android {\\n    defaultConfig {\\n    }/g' "$KTS"
  if grep -qE 'versionCode[[:space:]]*=' "$KTS"; then
    sed -i '' -E "s/versionCode[[:space:]]*=[[:space:]]*[0-9]+/versionCode = ${VERSION_CODE}/g" "$KTS"
  else
    sed -i '' -E "s/defaultConfig \\{/defaultConfig {\\n        versionCode = ${VERSION_CODE}/" "$KTS"
  fi
  if grep -qE 'versionName[[:space:]]*=' "$KTS"; then
    sed -i '' -E "s/versionName[[:space:]]*=[[:space:]]*\"[^\"]+\"/versionName = \"${VERSION_NAME}\"/g" "$KTS"
  else
    sed -i '' -E "s/versionCode = ${VERSION_CODE}/versionCode = ${VERSION_CODE}\\n        versionName = \"${VERSION_NAME}\"/" "$KTS"
  fi
fi

echo "Set versionCode=${VERSION_CODE}, versionName=${VERSION_NAME}"

# Ensure gradlew is executable (now relative to the 'app' directory)
GRADLEW_PATH="./gradlew"
if [ ! -x "$GRADLEW_PATH" ]; then
  echo "Making gradlew executable..."
  chmod +x "$GRADLEW_PATH"
fi

echo "Cleaning build artifacts..."
"$GRADLEW_PATH" clean

# Icon replacement step
echo "Replacing default app icons with custom ones..."
# Clean existing icons
find app/src/main/res -name "ic_launcher*.png" -type f -delete
find app/src/main/res -name "*.webp" -type f -delete
rm -rf app/src/main/res/mipmap-anydpi-v26
# Check for ImageMagick
if ! command -v convert &> /dev/null; then
  echo "Error: ImageMagick not installed. Please install it (e.g., brew install imagemagick) to generate icons automatically."
  exit 1
fi
if [ ! -f "$ABS_SOURCE_ICON" ]; then
  echo "Error: Source icon not found at $ABS_SOURCE_ICON"
  exit 1
fi
# Generate icons for different densities
for density_size in "mdpi 48" "hdpi 72" "xhdpi 96" "xxhdpi 144" "xxxhdpi 192"; do
  density=$(echo $density_size | cut -d' ' -f1)
  size=$(echo $density_size | cut -d' ' -f2)
  dir="app/src/main/res/mipmap-${density}"
  mkdir -p "$dir"
  convert "$ABS_SOURCE_ICON" -resize "${size}x${size}" "$dir/ic_launcher.png"
done
echo "Custom icons generated and placed in res/mipmap-* directories."

echo "Running Gradle bundleRelease task from $(pwd)..." # Should now be inside 'app'
# Run the bundleRelease task (clean already done)
echo "Running Gradle bundleRelease task..."
if "$GRADLEW_PATH" bundleRelease; then
  echo "Gradle bundleRelease successful."
else
  echo "Error: Gradle build failed."
  # Change back to project root before exiting on failure
  cd "$PROJECT_ROOT"
  exit 1
fi

# Define the expected location of the generated AAB (relative to the 'app' directory where gradle runs)
# The output is inside the 'app' module's build directory, which is nested
DEFAULT_AAB_PATH="app/build/outputs/bundle/release/app-release.aab"
EXPECTED_AAB_DIR="app/build/outputs/bundle/release" # Define the directory relative to 'app'

# List the contents of the expected output directory for debugging
ABS_EXPECTED_AAB_DIR="$(pwd)/$EXPECTED_AAB_DIR" # Absolute path for clarity in logs
echo "Checking contents of expected output directory: $ABS_EXPECTED_AAB_DIR"
# Create the directory path just in case Gradle didn't, though it should have
mkdir -p "$EXPECTED_AAB_DIR" # Create relative path
ls -l "$EXPECTED_AAB_DIR"    # List relative path

# Check if the AAB file was created (using the relative path from the current 'app' dir)
if [ ! -f "$DEFAULT_AAB_PATH" ]; then
  echo "Error: Expected AAB file not found at $(pwd)/$DEFAULT_AAB_PATH"
  # Change back to project root before exiting on failure
  cd "$PROJECT_ROOT"
  exit 1
fi

echo "AAB generated at: $(pwd)/$DEFAULT_AAB_PATH"

# Create the output directory if it doesn't exist
echo "Ensuring output directory exists: $ABS_OUTPUT_DIR"
mkdir -p "$ABS_OUTPUT_DIR"

# Copy the AAB to the final destination (using path relative to 'app' dir for source)
echo "Copying AAB from $(pwd)/$DEFAULT_AAB_PATH to $ABS_FINAL_AAB_PATH"
cp "$DEFAULT_AAB_PATH" "$ABS_FINAL_AAB_PATH" # Source path is relative to current dir

# Navigate back to the original directory (project root)
cd "$PROJECT_ROOT"

echo "-------------------------------------"
echo "Android AAB build complete!"
echo "Output available at: $ABS_FINAL_AAB_PATH"
echo "-------------------------------------"

exit 0