# Build Flutter app for all platforms and create GitHub release.
# Platforms: Android Flutter (APK), Android Native (APK), Web (zip), Windows (zip). iOS/macOS/Linux: IN PROGRESS.
#
# Usage: .\release-flutter.ps1 [-Version "v0.1.35"] [-SkipBuild]

param(
    [string]$Version = "v0.1.37",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$flutterDir = Join-Path $rootDir "flutter_app"
$jniLibs = Join-Path $flutterDir "android\app\src\main\jniLibs"
$assets = @()
$platformStatus = @{}

# Remove file locks
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}

if (-not $SkipBuild) {
    # 1. Build Rust library for Android (cargo ndk writes to stderr; avoid PowerShell treating it as error)
    Write-Host "`n=== 1. Building Rust library (Flutter/Android) ===" -ForegroundColor Cyan
    New-Item -ItemType Directory -Force -Path $jniLibs | Out-Null
    Push-Location $rootDir
    $prevErrPref = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o $jniLibs build -p ember-client --features flutter --release 2>&1 | ForEach-Object { Write-Host $_ }
    $ndkOk = ($LASTEXITCODE -eq 0)
    $ErrorActionPreference = $prevErrPref
    Pop-Location
    if (-not $ndkOk) {
        Write-Host "cargo ndk failed (exit $LASTEXITCODE). Skipping Flutter APK." -ForegroundColor Red
        $platformStatus["Android Flutter (APK)"] = "FAILED"
    }

    # 2. Build Flutter Android APK (only if Rust lib built)
    if ($ndkOk) {
    Write-Host "`n=== 2. Building Flutter Android APK ===" -ForegroundColor Cyan
    Push-Location $flutterDir
    flutter pub get
    flutter build apk --release
    if ($LASTEXITCODE -ne 0) { $platformStatus["Android Flutter (APK)"] = "FAILED" }
    Pop-Location
    }

    $apkPath = Join-Path $flutterDir "build\app\outputs\flutter-apk\app-release.apk"
    if ($ndkOk -and (Test-Path $apkPath)) {
        $destApk = Join-Path $rootDir "ember-flutter-app-release.apk"
        Copy-Item $apkPath $destApk -Force
        $assets += $destApk
        $platformStatus["Android Flutter (APK)"] = "BUILT"
    } else {
        $platformStatus["Android Flutter (APK)"] = "FAILED"
    }

    # 2b. Build native Android APK
    Write-Host "`n=== 2b. Building native Android APK ===" -ForegroundColor Cyan
    & (Join-Path $rootDir "build-android.ps1")
    $nativeApk = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release.apk"
    if (-not (Test-Path $nativeApk)) { $nativeApk = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release-unsigned.apk" }
    if (Test-Path $nativeApk) {
        $destNative = Join-Path $rootDir "ember-android-app-release.apk"
        Copy-Item $nativeApk $destNative -Force
        $assets += $destNative
        $platformStatus["Android Native (APK)"] = "BUILT"
    } else {
        $platformStatus["Android Native (APK)"] = "FAILED"
    }

    # 3. Build Flutter Web
    Write-Host "`n=== 3. Building Flutter Web ===" -ForegroundColor Cyan
    Push-Location $flutterDir
    flutter build web --release
    if ($LASTEXITCODE -eq 0) {
        $webDir = Join-Path $flutterDir "build\web"
        if (Test-Path $webDir) {
            $webZip = Join-Path $rootDir "ember-flutter-web.zip"
            Compress-Archive -Path (Join-Path $webDir "*") -DestinationPath $webZip -Force
            $assets += $webZip
            $platformStatus["Web (zip)"] = "BUILT"
        } else {
            $platformStatus["Web (zip)"] = "FAILED"
        }
    } else {
        $platformStatus["Web (zip)"] = "FAILED"
    }
    Pop-Location

    # 4. Build Flutter Windows (may fail if Rust lib for Windows not built)
    Write-Host "`n=== 4. Building Flutter Windows ===" -ForegroundColor Cyan
    Push-Location $flutterDir
    flutter build windows --release
    if ($LASTEXITCODE -eq 0) {
        $winDir = Join-Path $flutterDir "build\windows\x64\runner\Release"
        if (Test-Path $winDir) {
            $winZip = Join-Path $rootDir "ember-flutter-windows.zip"
            Compress-Archive -Path (Join-Path $winDir "*") -DestinationPath $winZip -Force
            $assets += $winZip
            $platformStatus["Windows (zip)"] = "BUILT"
        } else {
            $platformStatus["Windows (zip)"] = "FAILED"
        }
    } else {
        $platformStatus["Windows (zip)"] = "FAILED"
    }
    Pop-Location
} else {
    # Skip build - collect existing artifacts
    $apkPath = Join-Path $rootDir "ember-flutter-app-release.apk"
    if (Test-Path $apkPath) { $assets += $apkPath; $platformStatus["Android Flutter (APK)"] = "BUILT" }
    $nativeApk = Join-Path $rootDir "ember-android-app-release.apk"
    if (Test-Path $nativeApk) { $assets += $nativeApk; $platformStatus["Android Native (APK)"] = "BUILT" }
    $webZip = Join-Path $rootDir "ember-flutter-web.zip"
    if (Test-Path $webZip) { $assets += $webZip; $platformStatus["Web (zip)"] = "BUILT" }
    $winZip = Join-Path $rootDir "ember-flutter-windows.zip"
    if (Test-Path $winZip) { $assets += $winZip; $platformStatus["Windows (zip)"] = "BUILT" }
}

# Platforms not built on Windows
$platformStatus["iOS"] = "IN PROGRESS"
$platformStatus["macOS"] = "IN PROGRESS"
$platformStatus["Linux"] = "IN PROGRESS"

# Build release notes with platform status
$statusLines = $platformStatus.GetEnumerator() | ForEach-Object { "- **$($_.Key)**: $($_.Value)" }
$statusBlock = $statusLines -join "`n"

$notes = @"
Ember Flutter v0.1.37 - Push polling, reminder logging, reminder worker service.

**Changes:** Flutter app polls for push every 30s in foreground; ember server logs reminder text from push-queue; EmberReminderWorker installable as NSSM service.

**Platform status:**
$statusBlock

**Install (Android):** Copy APK to phone, enable 'Install from unknown sources'. Two APKs: Flutter (voice, TTS, geolocation) and Native (Kotlin/WebView).

**Web:** Extract zip, serve the folder (requires HTTP proxy for QUIC). Default: https://localhost:8443.

**Server:** Run ember-server and HTTP proxy (cargo run --bin http_proxy -p ember-client --features http_proxy).
"@

$notesFile = Join-Path $env:TEMP "ember-flutter-release-notes.md"
$notes | Set-Content -Path $notesFile -Encoding UTF8

# QR code if exists
$promqrPath = Join-Path $rootDir "promqr.png"
if (Test-Path $promqrPath) { $assets += $promqrPath }

if ($assets.Count -eq 0) {
    Write-Host "No assets to upload. Build first or use -SkipBuild with existing artifacts." -ForegroundColor Red
    exit 1
}

$repoRoot = (git -C $rootDir rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    Write-Host "Not in a git repository." -ForegroundColor Red
    exit 1
}

Write-Host "`n=== Creating release $Version ===" -ForegroundColor Cyan
$assets | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }

Push-Location $repoRoot
try {
    gh release create $Version --title "Ember Flutter $Version" --notes-file $notesFile @assets
    Write-Host "Release created: $Version" -ForegroundColor Green
} catch {
    Write-Host "Release may exist. Uploading assets..." -ForegroundColor Yellow
    gh release upload $Version @assets
    Write-Host "Assets uploaded." -ForegroundColor Green
} finally {
    Pop-Location
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}

# Cleanup temp zips only if we built them this run
if (-not $SkipBuild) {
    Remove-Item (Join-Path $rootDir "ember-flutter-web.zip") -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $rootDir "ember-flutter-windows.zip") -ErrorAction SilentlyContinue
}
