#!/bin/bash
# Run from Xcode as a Build Phase. Builds the Rust library for the current target.
set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ "$PLATFORM_NAME" = "iphonesimulator" ]; then
  TARGET="aarch64-apple-ios-sim"
else
  TARGET="aarch64-apple-ios"
fi

echo "Building ember-client for $TARGET..."
cargo build --release -p ember-client --features ios --target "$TARGET"

LIB_DIR="${PROJECT_DIR}/Ember/lib"
mkdir -p "$LIB_DIR"
cp "$ROOT/target/$TARGET/release/libember_native.a" "$LIB_DIR/"
