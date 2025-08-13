Android will crash - use the scripts to force enviroments.
sh scripts/android.build.sh - build for android apk cleanly
sh scripts/android.bundle.sh && sh scripts/android.update.sh

bundle and push to device. APK available for release as well.

tools and cargo make are in charge of building for real iphone deployment. 
cargo make build_ios_device && cargo make code-sign-ios-device && cargo make run-ios-device
build for device, sign and install.
make sure to resolve paths/read the errors.
Will not load from local assets 

these will serve on local device and emulator no problem 
dxalpha serve --platform ios
dxalpha serve --platform macos

LOCAL installation on device - preferably use scripts.
dxalpha build --platform android
dxalpha bundle --platform android

hogyzen12@anons-MBP unruggable-app % find ./target -name "*.apk"
./target/dx/unruggable/release/android/app/app/build/outputs/apk/debug/app-debug.apk
./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk

hogyzen12@anons-MacBook-Pro unruggable-app % adb install -r ./target/dx/unruggable/release/android/app/app/build/outputs/apk/debug/app-debug.apk
Performing Streamed Install
Success 

#dioxus = { version = "0.6.0", features = ["fullstack", "router"] }
#BELOW is for old dx - deprecetiaed
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/29.0.13599879"
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin:$PATH"
dx serve --platform android
** need to be running the emulator with:
emulator -avd Pixel_6_API34  -netdelay none -netspeed full

dx bundle --platform ios
dx bundle --platform macos
dx bundle --platform android 

