# Create a GitHub release and upload the Ember Android APK.
# Prerequisites: Build the APK first (.\build-android.ps1), gh CLI, and git auth.
#
# Usage: .\release-android.ps1 [-Version "v0.1.0"] [-ApkPath "path\to\app.apk"]

param(
    [string]$Version = "ember-v0.1.0",
    [string]$ApkPath = ""
)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Find APK (prefer signed if available)
if ($ApkPath -and (Test-Path $ApkPath)) {
    $apk = Resolve-Path $ApkPath
} else {
    $signedApk = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release-signed.apk"
    $gradleSigned = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release.apk"
    $defaultApk = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release-unsigned.apk"
    if (Test-Path $gradleSigned) { $defaultApk = $gradleSigned }
    elseif (Test-Path $signedApk) { $defaultApk = $signedApk }
    if (-not (Test-Path $defaultApk)) {
        Write-Host "APK not found. Build it first:" -ForegroundColor Red
        Write-Host "  .\build-android.ps1" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Requires: Rust, cargo-ndk, Android SDK/NDK (Android Studio)" -ForegroundColor Gray
        exit 1
    }
    $apk = $defaultApk
}

$apkName = [System.IO.Path]::GetFileName($apk)
Write-Host "Using APK: $apk" -ForegroundColor Cyan
Write-Host ""

# Repo root is parent of ember (git root)
$repoRoot = (git -C $rootDir rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    Write-Host "Not in a git repository." -ForegroundColor Red
    exit 1
}

# Create release
$notes = @"
Ember QUIC client for Android. Connect to your ember server from a smartphone.

**Install:** Copy APK to phone and install (enable 'Install from unknown sources' if needed).

**Usage:** Enter server address (e.g. 192.168.1.100:4433) and tap Connect.

**Server:** Run: cargo run -p ember-server on your PC.
"@
$notesFile = Join-Path $env:TEMP "ember-release-notes.md"
$notes | Set-Content -Path $notesFile -Encoding UTF8

Push-Location $repoRoot
try {
    Write-Host "Creating release $Version..." -ForegroundColor Cyan
    gh release create $Version --title "Ember Android $Version" --notes-file $notesFile $apk
    Write-Host "Release created: $Version" -ForegroundColor Green
} catch {
    Write-Host "Release may already exist. Trying to upload asset only..." -ForegroundColor Yellow
    gh release upload $Version $apk
    Write-Host "Asset uploaded." -ForegroundColor Green
} finally {
    Pop-Location
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}
