# Source the environment file only if not in a CI environment
if [ -z "${CI-}" ]; then
  SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
  source "$SCRIPT_DIR/android.env"
fi
dxalpha build --platform android --release --verbose --target aarch64-linux-android
#Make sure to remove the target/dx folder when building with the script for each cli version.
#dx build --platform android --release --verbose --target aarch64-linux-android