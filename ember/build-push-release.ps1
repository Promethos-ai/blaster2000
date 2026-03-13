# Build APK, push repo, and create GitHub release.
# Prerequisites: Rust, cargo-ndk, Android SDK/NDK, gh CLI, git auth.
#
# Usage: .\build-push-release.ps1 [-Version "ember-v0.1.1"] [-SkipPush] [-SkipRelease]

param(
    [string]$Version = "ember-v0.1.15",
    [switch]$SkipPush,
    [switch]$SkipRelease
)

$ErrorActionPreference = "Stop"
$emberDir = if (Test-Path "d:\rust\ember") { "d:\rust\ember" } else { $PSScriptRoot }
$repoRoot = "d:\rust"

# 1. Build APK
Write-Host "=== 1. Building APK ===" -ForegroundColor Cyan
Push-Location $emberDir
try {
    & .\build-android.ps1
    if ($LASTEXITCODE -ne 0) { exit 1 }
} finally {
    Pop-Location
}

# 2. Git add, commit, push
if (-not $SkipPush) {
    Write-Host "`n=== 2. Pushing repo ===" -ForegroundColor Cyan
    Push-Location $repoRoot
    try {
        git status --short
        git add -A
        $status = git status --porcelain
        if ($status) {
            git commit -m "Ember: build $Version - WebView chat, device capabilities, server CSS"
            git push
        } else {
            Write-Host "Nothing to commit." -ForegroundColor Yellow
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "`n=== 2. Skipping push (--SkipPush) ===" -ForegroundColor Gray
}

# 3. Create GitHub release
if (-not $SkipRelease) {
    Write-Host "`n=== 3. Creating release $Version ===" -ForegroundColor Cyan
    Push-Location $emberDir
    try {
        & .\release-android.ps1 -Version $Version
    } finally {
        Pop-Location
    }
} else {
    Write-Host "`n=== 3. Skipping release (--SkipRelease) ===" -ForegroundColor Gray
}

Write-Host "`nDone." -ForegroundColor Green
