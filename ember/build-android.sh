#!/bin/bash
# Build the Ember Android app and produce an APK.
# Requires: Rust, cargo-ndk, Android SDK/NDK (via Android Studio)

set -e
ROOT="$(cd "$(dirname "$0")" && pwd)"
JNI_LIBS="$ROOT/android/app/src/main/jniLibs"

echo "Building Rust library for Android..."
mkdir -p "$JNI_LIBS"
cargo ndk -t arm64-v8a -t armeabi-v7a -o "$JNI_LIBS" build -p ember-client --features android --release

echo ""
echo "Building Android APK..."
cd "$ROOT/android"
./gradlew assembleRelease

APK="$ROOT/android/app/build/outputs/apk/release/app-release-unsigned.apk"
echo ""
echo "Done! APK: $APK"
echo "For distribution, sign the APK (e.g. with jarsigner or Android Studio)."
