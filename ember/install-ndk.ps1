# Install Android NDK via command-line tools.
# Requires: Android SDK at $env:LOCALAPPDATA\Android\Sdk (default)
# Run as: .\install-ndk.ps1

$ErrorActionPreference = "Stop"
$sdkRoot = $env:ANDROID_HOME
if (-not $sdkRoot) {
    $sdkRoot = Join-Path $env:LOCALAPPDATA "Android\Sdk"
}
if (-not (Test-Path $sdkRoot)) {
    Write-Host "Android SDK not found at $sdkRoot" -ForegroundColor Red
    Write-Host "Install Android Studio first, or set ANDROID_HOME to your SDK path." -ForegroundColor Yellow
    exit 1
}

$cmdlineUrl = "https://dl.google.com/android/repository/commandlinetools-win-14742923_latest.zip"
$zipPath = Join-Path $env:TEMP "commandlinetools-win.zip"
$extractDir = Join-Path $env:TEMP "commandlinetools-win"
$cmdlineDest = Join-Path $sdkRoot "cmdline-tools\latest"

# Download command-line tools if not present
if (-not (Test-Path (Join-Path $cmdlineDest "bin\sdkmanager.bat"))) {
    Write-Host "Downloading Android command-line tools..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $cmdlineUrl -OutFile $zipPath -UseBasicParsing

    Write-Host "Extracting..." -ForegroundColor Cyan
    if (Test-Path $extractDir) { Remove-Item $extractDir -Recurse -Force }
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    # Move cmdline-tools to SDK (extracted zip has cmdline-tools\bin, etc. inside)
    $extracted = Get-ChildItem $extractDir -Directory | Select-Object -First 1
    if ($extracted.Name -eq "cmdline-tools") {
        $inner = Get-ChildItem $extracted.FullName -Directory | Select-Object -First 1
        New-Item -ItemType Directory -Path (Join-Path $sdkRoot "cmdline-tools") -Force | Out-Null
        Move-Item -Path $inner.FullName -Destination $cmdlineDest -Force
    } else {
        New-Item -ItemType Directory -Path (Split-Path $cmdlineDest) -Force | Out-Null
        Move-Item -Path (Join-Path $extractDir "*") -Destination $cmdlineDest -Force
    }

    Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
    Remove-Item $extractDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Command-line tools installed." -ForegroundColor Green
} else {
    Write-Host "Command-line tools already present." -ForegroundColor Green
}

$sdkmanager = Join-Path $cmdlineDest "bin\sdkmanager.bat"
if (-not (Test-Path $sdkmanager)) {
    Write-Host "sdkmanager not found at $sdkmanager" -ForegroundColor Red
    exit 1
}

# Set ANDROID_HOME for sdkmanager
$env:ANDROID_HOME = $sdkRoot

# Accept licenses if needed (run interactively - user may need to type 'y' for each)
Write-Host "Checking licenses (type 'y' and Enter for each prompt if asked)..." -ForegroundColor Cyan
$licenses = & $sdkmanager --licenses 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "Run manually if needed: $sdkmanager --licenses" -ForegroundColor Yellow
}

# Install NDK
Write-Host "Installing NDK (this may take a few minutes)..." -ForegroundColor Cyan
& $sdkmanager --install "ndk;27.1.12297018" 2>&1

Write-Host ""
$ndkDir = Join-Path $sdkRoot "ndk\27.1.12297018"
if (Test-Path $ndkDir) {
    Write-Host "NDK installed at: $ndkDir" -ForegroundColor Green
    Write-Host ""
    Write-Host "Set for this session:" -ForegroundColor Yellow
    Write-Host "  `$env:ANDROID_NDK_HOME = `"$ndkDir`"" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Or add ANDROID_NDK_HOME to your system environment variables." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Then run: .\build-android.ps1" -ForegroundColor Cyan
} else {
    Write-Host "NDK install may have failed. Check output above." -ForegroundColor Red
    exit 1
}
