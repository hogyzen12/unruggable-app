export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/23.1.7779620"
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin:$PATH"
dx serve --platform android

** need to be running the emulator with:
emulator -avd Pixel_6_API34  -netdelay none -netspeed full


hogyzen12@anons-MacBook-Pro unruggable-app % find ./target -name "*.apk"   

./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk

hogyzen12@anons-MacBook-Pro unruggable-app % adb install -r ./target/dx/unruggable/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
Performing Streamed Install
Success 