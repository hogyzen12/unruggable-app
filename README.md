ANDROID 
buid, install on device, prepare for play store upload

Android will crash - use the scripts to force enviroments.
 - build for android apk cleanly
sh scripts/android.build.sh && sh scripts/android.bundle.sh && sh scripts/android.update.sh && jarsigner -verbose \
  -sigalg SHA256withRSA -digestalg SHA-256 \
  -keystore ~/keys/unruggable-upload.jks \
  "/Users/hogyzen12/coding-project-folders/unruggable-app/dist/android/unruggable-release.aab" \
  unruggable-upload

For self hosted release via github and not through store.
sh scripts/android.build.sh && sh scripts/android.bundle.sh && sh scripts/android.github.sh

bundle and push to device. APK available for sideloading release as well.

For other ditris use hosted assets to make sure it builds
IPHONES
tools and cargo make are in charge of building for real iphone deployment. 

cargo make build_ios_device && cargo make code-sign-ios-device && cargo make run-ios-device

build for device, sign and install. Will not load from local assets. 

MacOS
simply execute macos_package.sh and let the script deal with it.
Ensure parameters are correctly set.

Windows
cross compile from mac/use cargo stuff
export X86_64_PC_WINDOWS_MSVC_OPENSSL_DIR=/Users/hogyzen12/Downloads/openssl-3.5.3/x64 && export X86_64_PC_WINDOWS_MSVC_OPENSSL_NO_VENDOR=1 && cargo xwin build --target x86_64-pc-windows-msvc --release
ls target/x86_64-pc-windows-msvc/release
add the dll file to the release
zip dlls with the exe for functional exe


Manual installation.
hogyzen12@anons-MBP unruggable-app % find ./target -name "*.apk"
./target/dx/unruggable/release/android/app/app/build/outputs/apk/debug/app-debug.apk
./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk

hogyzen12@anons-MacBook-Pro unruggable-app % adb install -r ./target/dx/unruggable/release/android/app/app/build/outputs/apk/debug/app-debug.apk
Performing Streamed Install
Success 


