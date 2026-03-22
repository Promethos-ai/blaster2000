# Build the Ember Android app and produce an APK.
# Requires: Rust, cargo-ndk, Android SDK/NDK (via Android Studio)
#
# Usage: .\build-android.ps1 [-Clean]
#   -Clean  Run cargo clean first (fixes "Blocking waiting for file lock" errors)

param([switch]$Clean)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Server address is hardcoded in android/app/src/main/res/values/server_defaults.xml

# Remove file locks before building (kills cargo/rustc/servers, clears .cargo-lock, Gradle locks)
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}
if ($Clean) {
    Write-Host "Running cargo clean (removes target/)..." -ForegroundColor Yellow
    Push-Location $rootDir
    cargo clean
    Pop-Location
}

# Build Rust library for Android
Write-Host "Building Rust library for Android..." -ForegroundColor Cyan
$jniLibs = Join-Path $rootDir "android\app\src\main\jniLibs"
New-Item -ItemType Directory -Force -Path $jniLibs | Out-Null

Push-Location $rootDir
try {
    $prevErr = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o $jniLibs build -p ember-client --features android --release 2>&1 | ForEach-Object { Write-Host $_ }
    $ErrorActionPreference = $prevErr
    if ($LASTEXITCODE -ne 0) { throw "cargo ndk failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}

# Copy QR asset for rich area placeholder (if present)
$assetsDir = Join-Path $rootDir "android\app\src\main\assets"
$promqrSrc = Join-Path $rootDir "promqr.png"
if (Test-Path $promqrSrc) {
    New-Item -ItemType Directory -Force -Path $assetsDir | Out-Null
    Copy-Item $promqrSrc (Join-Path $assetsDir "promqr.png") -Force
}

# Build Android APK
Write-Host "`nBuilding Android APK..." -ForegroundColor Cyan
Push-Location (Join-Path $rootDir "android")
try {
    # Set ANDROID_HOME if not already set (Gradle needs this)
    if (-not $env:ANDROID_HOME) {
        $sdkPath = Join-Path $env:LOCALAPPDATA "Android\Sdk"
        if (Test-Path $sdkPath) {
            $env:ANDROID_HOME = $sdkPath
        }
    }
    # Use gradlew.bat on Windows; ensure JAVA_HOME for Android Studio's JBR if not set
    if (-not $env:JAVA_HOME) {
        $jbrPaths = @(
            "C:\Program Files\Android\Android Studio\jbr",
            "$env:LOCALAPPDATA\Programs\Android Studio\jbr"
        )
        foreach ($p in $jbrPaths) {
            if (Test-Path (Join-Path $p "bin\java.exe")) {
                $env:JAVA_HOME = $p
                break
            }
        }
    }
    if (Test-Path "gradlew.bat") {
        .\gradlew.bat assembleRelease
    } else {
        .\gradlew assembleRelease
    }
    if ($LASTEXITCODE -ne 0) { throw "gradle build failed" }
} finally {
    Pop-Location
}

$apkPath = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release-unsigned.apk"
$signedPath = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release.apk"
if (Test-Path $signedPath) {
    $apkPath = $signedPath
}
Write-Host "`nDone! APK: $apkPath" -ForegroundColor Green
if (-not (Test-Path $signedPath)) {
    Write-Host "To sign: create android/keystore.properties (see keystore.properties.example), then rebuild. Or run .\sign-apk.ps1" -ForegroundColor Yellow
}
