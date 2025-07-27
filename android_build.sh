export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/23.1.7779620"
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin:$PATH"
dx serve --platform android
** need to be running the emulator with:
emulator -avd Pixel_6_API34  -netdelay none -netspeed full


dxalpha build --platform android
dxalpha bundle --platform android

hogyzen12@anons-MacBook-Pro unruggable-app % find ./target -name "*.apk"   

./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk

hogyzen12@anons-MacBook-Pro unruggable-app % adb install -r ./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
Performing Streamed Install
Success 

dx build --platform ios && dx build --platform macos && dx build --platform android

dx bundle --platform ios
dx bundle --platform macos
dx bundle --platform android

#dioxus = { version = "0.6.0", features = ["fullstack", "router"] }
dioxus = { version = "0.7.0-alpha.0", features = ["fullstack", "router"] }

dxalpha serve --platform ios
dxalpha serve --platform macos
dxalpha serve --platform android


apktool d \
  ./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk \
  -o decoded-manifest

grep '<uses-permission' -n decoded-manifest/AndroidManifest.xml
