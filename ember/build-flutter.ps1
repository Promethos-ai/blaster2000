# Build the Ember Flutter app (Android).
# Requires: Rust, cargo-ndk, Flutter SDK, Android SDK/NDK
#
# Usage: .\build-flutter.ps1 [-Clean] [-AppBundle]
#   -Clean     Run cargo clean first
#   -AppBundle Build .aab for Play Store (default: APK for direct install)
#
# For Play Store: create android/key.properties from key.properties.example,
# then run: .\build-flutter.ps1 -AppBundle

param([switch]$Clean, [switch]$AppBundle)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$flutterDir = Join-Path $rootDir "flutter_app"
$jniLibs = Join-Path $flutterDir "android\app\src\main\jniLibs"

# Remove file locks before building
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}
if ($Clean) {
    Write-Host "Running cargo clean..." -ForegroundColor Yellow
    Push-Location $rootDir
    cargo clean
    Pop-Location
}

# Build Rust library for Android (C FFI for Flutter, not JNI)
Write-Host "Building Rust library for Flutter (Android)..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $jniLibs | Out-Null

Push-Location $rootDir
try {
    $prevErr = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o $jniLibs build -p ember-client --features flutter --release 2>&1 | ForEach-Object { Write-Host $_ }
    $ErrorActionPreference = $prevErr
    if ($LASTEXITCODE -ne 0) { throw "cargo ndk failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}

# Build Flutter APK or App Bundle
$buildTarget = if ($AppBundle) { "App Bundle" } else { "APK" }
Write-Host "`nBuilding Flutter $buildTarget..." -ForegroundColor Cyan
Push-Location $flutterDir
try {
    flutter pub get
    if ($AppBundle) {
        flutter build appbundle --release
        if ($LASTEXITCODE -ne 0) { throw "flutter build failed" }
    } else {
        flutter build apk --release
        if ($LASTEXITCODE -ne 0) { throw "flutter build failed" }
    }
} finally {
    Pop-Location
}

if ($AppBundle) {
    $outPath = Join-Path $flutterDir "build\app\outputs\bundle\release\app-release.aab"
    Write-Host "`nDone! App Bundle: $outPath" -ForegroundColor Green
    Write-Host "Upload to Play Console: https://play.google.com/console" -ForegroundColor Cyan
} else {
    $apkPath = Join-Path $flutterDir "build\app\outputs\flutter-apk\app-release.apk"
    Write-Host "`nDone! APK: $apkPath" -ForegroundColor Green
}
