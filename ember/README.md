# Ember — Quinn QUIC Example

A rudimentary Quinn/QUIC echo server and client for connecting from an Android or iPhone smartphone to a home computer.

## Quick Start

**Terminal 1 — run the server (on your home PC):**
```bash
cargo run -p ember-server
```

**Terminal 2 — run the client:**
```bash
cargo run -p ember-client -- 127.0.0.1:4433
```

You should see the client send a message and receive it echoed back.

## Architecture

- **`ember-server`**: Quinn QUIC echo server. Listens on `0.0.0.0:4433` (all interfaces). Uses a self-signed certificate.
- **`ember-client`**: Quinn QUIC client. Connects to the server, sends a message, prints the echo. Skips certificate verification (development only).

## Android App

A simple Android app is included in `android/`. It provides a UI to connect to the ember server.

### Build the APK

**Prerequisites:** Rust, [cargo-ndk](https://github.com/bbqsrc/cargo-ndk) (`cargo install cargo-ndk`), Android Studio (for SDK/NDK).

```powershell
# Windows
.\build-android.ps1
```

```bash
# Linux/macOS
./build-android.sh
```

The APK is at `android/app/build/outputs/apk/release/app-release-unsigned.apk`.

### Sign for distribution

```powershell
.\sign-apk.ps1
```

First run creates `ember.keystore` and prompts for a password. Subsequent runs sign with the existing keystore. The signed APK is written as `app-release-signed.apk`.

Or manually:
1. Create keystore: `keytool -genkey -v -keystore ember.keystore -alias ember -keyalg RSA -keysize 2048 -validity 10000`
2. Sign: `jarsigner -verbose -sigalg SHA256withRSA -digestalg SHA-256 -keystore ember.keystore app-release-unsigned.apk ember`
3. Or use Android Studio: Build → Generate Signed Bundle/APK

### Install

- **USB:** `adb install app-release-unsigned.apk`
- **Direct:** Copy APK to phone, open it (enable "Install from unknown sources" if needed)

### Run the server

On your home PC, forward UDP port **4433** and run `cargo run -p ember-server`. Enter your PC's IP (e.g. `192.168.1.100:4433`) in the app.

---

## iOS App

A simple iOS app is included in `ios/`. It provides a SwiftUI interface to connect to the ember server.

### Prerequisites

- macOS with Xcode
- Rust (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`)

### Build and run

1. Open `ios/Ember.xcodeproj` in Xcode.
2. Select your development team in **Signing & Capabilities** (required for device/simulator).
3. Build and run (⌘R). The first build will compile the Rust library via the "Build Rust Library" run script phase.

### Alternative: pre-build the library

To build the Rust library separately (e.g. for CI or faster iteration):

```bash
# From project root, on macOS
cd ios && ./build-ios.sh
```

Then open the Xcode project and build. The `build-ios.sh` script creates an XCFramework; you may need to add it to the project or use the Run Script phase (default).

### Run the server

On your home PC, forward UDP port **4433** and run `cargo run -p ember-server`. Enter your PC's IP (e.g. `192.168.1.100:4433`) in the app. For local network testing, `NSAllowsLocalNetworking` is enabled in Info.plist.

---

## Connecting from Android (manual build)

### 1. Port forwarding

On your home router, forward UDP port **4433** to your PC’s local IP.

### 2. Find your public IP or hostname

Use your public IP or a dynamic DNS hostname (e.g. DuckDNS, No-IP).

For a **Pinggy-like tunnel** via eagleoneonline.ca, see [docs/PGROK-EAGLEONE-README.md](docs/PGROK-EAGLEONE-README.md). For UDP/QUIC (ember), see [docs/PORT-FORWARDING-AND-TUNNEL-SETUP.md](docs/PORT-FORWARDING-AND-TUNNEL-SETUP.md).

### 3. Cross-compile for Android

Install the Android NDK and Rust targets:

```bash
# Install cargo-ndk
cargo install cargo-ndk

# Add Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

Build the client:

```bash
cd client
cargo ndk -t arm64-v8a -t armeabi-v7a -o ../android/app/src/main/jniLibs build --release
```

Or build for a specific target:

```bash
cargo build -p ember-client --release --target aarch64-linux-android
```

### 4. Integrate into an Android app

- Build the client as a `cdylib` and call it via JNI from Kotlin/Java.
- Or use a Rust-on-Android framework (e.g. Tauri for Android when available).

### 5. Run the client on the phone

Point the client at your home IP or hostname and port:

```bash
# Example (replace with your public IP or hostname)
cargo run -p ember-client -- 192.168.1.100:4433   # local network
cargo run -p ember-client -- yourname.duckdns.org:4433   # via internet
```

## Security note

The client disables certificate verification for development. For production, use proper certificates (e.g. Let’s Encrypt) or pin the server’s certificate.

## Development

For engineering notes (removing file locks, build commands, troubleshooting), see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Dependencies

- [quinn](https://github.com/quinn-rs/quinn) — QUIC implementation
- [rustls](https://github.com/rustls/rustls) — TLS
- [rcgen](https://github.com/rustls/rcgen) — certificate generation (server)
- [tokio](https://tokio.rs) — async runtime
