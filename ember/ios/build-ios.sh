#!/bin/bash
# Build the Rust library for iOS.
# Run from project root or ios/. Outputs to ios/Ember/lib/.
# Use this to pre-build before opening Xcode, or rely on Xcode's Run Script phase.

set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
LIB_DIR="$ROOT/ios/Ember/lib"
mkdir -p "$LIB_DIR"

# Add iOS targets if missing
rustup target add aarch64-apple-ios aarch64-apple-ios-sim 2>/dev/null || true

echo "Building for iOS device (aarch64-apple-ios)..."
cargo build --release -p ember-client --features ios --target aarch64-apple-ios
cp "$ROOT/target/aarch64-apple-ios/release/libember_native.a" "$LIB_DIR/libember_native.a"

echo "Building for iOS simulator (aarch64-apple-ios-sim)..."
cargo build --release -p ember-client --features ios --target aarch64-apple-ios-sim
cp "$ROOT/target/aarch64-apple-ios-sim/release/libember_native.a" "$LIB_DIR/libember_native_sim.a"

echo "Done. Device lib: $LIB_DIR/libember_native.a"
echo "Simulator lib: $LIB_DIR/libember_native_sim.a"
echo "Open ios/Ember.xcodeproj in Xcode and build (or use Run Script for automatic build)."
