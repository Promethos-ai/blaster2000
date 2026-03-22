# Build Ember server release bundle: ember-server + grpc_server + pinggy + activation URL
# Output: ember/release/ember-server-bundle/
#
# Usage: .\release-ember-server.ps1 [-Zip]
#   -Zip  Create ember-server-bundle.zip for GitHub release

param([switch]$Zip)

$ErrorActionPreference = "Stop"
$emberDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$feb17Dir = "d:\rust\Feb17"
$releaseDir = Join-Path $emberDir "release\ember-server-bundle"
$pinggyUrl = "https://s3.ap-south-1.amazonaws.com/public.pinggy.binaries/cli/v0.2.5/windows/amd64/pinggy.exe"

Write-Host "=== Ember Server Release Bundle ===" -ForegroundColor Cyan

# 1. Build ember-server
Write-Host "`n1. Building ember-server..." -ForegroundColor Yellow
Push-Location $emberDir
cargo build -p ember-server --release
if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }
Pop-Location

# 2. Build grpc_server (Feb17)
Write-Host "2. Building grpc_server (Feb17)..." -ForegroundColor Yellow
if (-not (Test-Path $feb17Dir)) {
    Write-Host "Feb17 not found at $feb17Dir. Skipping grpc_server." -ForegroundColor Yellow
    $grpcBuilt = $false
} else {
    Push-Location $feb17Dir
    cargo build --bin grpc_server --release
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit 1 }
    Pop-Location
    $grpcBuilt = $true
}

# 3. Create release dir
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

# 4. Copy ember-server
Copy-Item (Join-Path $emberDir "target\release\ember-server.exe") (Join-Path $releaseDir "ember-server.exe") -Force
Write-Host "   Copied ember-server.exe" -ForegroundColor Gray

# 5. Copy grpc_server
if ($grpcBuilt) {
    Copy-Item (Join-Path $feb17Dir "target\release\grpc_server.exe") (Join-Path $releaseDir "grpc_server.exe") -Force
    Write-Host "   Copied grpc_server.exe" -ForegroundColor Gray
}

# 6. Download pinggy.exe
$pinggyPath = Join-Path $releaseDir "pinggy.exe"
Write-Host "3. Downloading pinggy.exe..." -ForegroundColor Yellow
try {
    Invoke-WebRequest -Uri $pinggyUrl -OutFile $pinggyPath -UseBasicParsing
    Write-Host "   Downloaded pinggy.exe" -ForegroundColor Gray
} catch {
    Write-Host "   Pinggy download failed: $_" -ForegroundColor Red
    Write-Host "   Get it from: $pinggyUrl" -ForegroundColor Gray
}

# 7. Copy config files
$configFiles = @("instructions.txt", "server\chat-style.css", "server\rich-placeholder.html")
if (Test-Path (Join-Path $emberDir "pinggy.bat")) {
    Copy-Item (Join-Path $emberDir "pinggy.bat") (Join-Path $releaseDir "pinggy.bat") -Force
    Write-Host "   Copied pinggy.bat" -ForegroundColor Gray
}
if (Test-Path (Join-Path $emberDir "pinggy.ps1")) {
    Copy-Item (Join-Path $emberDir "pinggy.ps1") (Join-Path $releaseDir "pinggy.ps1") -Force
    Write-Host "   Copied pinggy.ps1" -ForegroundColor Gray
}
if (Test-Path (Join-Path $emberDir "pinggy.secret")) {
    Copy-Item (Join-Path $emberDir "pinggy.secret") (Join-Path $releaseDir "pinggy.secret") -Force
    Write-Host "   Copied pinggy.secret" -ForegroundColor Gray
}
foreach ($f in $configFiles) {
    $src = Join-Path $emberDir $f
    if (Test-Path $src) {
        $destDir = Join-Path $releaseDir (Split-Path $f)
        if ($destDir -ne $releaseDir) { New-Item -ItemType Directory -Force -Path $destDir | Out-Null }
        Copy-Item $src (Join-Path $releaseDir $f) -Force -ErrorAction SilentlyContinue
    }
}

# 8. Create install-and-start.ps1
$installScript = @'
# Install and start Ember stack: grpc service + ember-server + pinggy
# Run as Administrator to install the grpc service.
# Usage: .\install-and-start.ps1

param([switch]$NoService, [switch]$NoPinggy)

$ErrorActionPreference = "Stop"
$bundleDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# 1. Install grpc_server as Windows service (requires Admin)
if (-not $NoService -and (Test-Path "$bundleDir\grpc_server.exe")) {
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin) {
        Write-Host "Installing grpc_server as Windows service..." -ForegroundColor Cyan
        $nssm = Get-Command nssm -ErrorAction SilentlyContinue
        if ($nssm) {
            & nssm stop EmberGrpcServer 2>$null
            & nssm remove EmberGrpcServer confirm 2>$null
            & nssm install EmberGrpcServer "$bundleDir\grpc_server.exe" --log-file "$bundleDir\grpc_server.log"
            & nssm set EmberGrpcServer AppDirectory $bundleDir
            & nssm set EmberGrpcServer Start SERVICE_AUTO_START
            Start-Service EmberGrpcServer
            Write-Host "  EmberGrpcServer service started." -ForegroundColor Green
        } else {
            Write-Host "  NSSM not found. Run: winget install NSSM.NSSM" -ForegroundColor Yellow
        }
    } else {
        Write-Host "Run as Administrator to install grpc_server service. Or start manually." -ForegroundColor Yellow
    }
}

# 2. Start ember-server
Write-Host "`nStarting ember-server..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$bundleDir'; `$Host.UI.RawUI.WindowTitle='ember-server'; .\ember-server.exe --port 4433 --inference http://127.0.0.1:50051 --web-search --web-search-always --instructions-file instructions.txt"

# 3. Start pinggy and show activation URL
if (-not $NoPinggy -and (Test-Path "$bundleDir\pinggy.exe")) {
    Write-Host "`nStarting pinggy tunnel..." -ForegroundColor Cyan
    $pinggyBat = Join-Path $bundleDir "pinggy.bat"
    if (Test-Path $pinggyBat) {
        Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$bundleDir'; `$Host.UI.RawUI.WindowTitle='pinggy'; Write-Host 'Copy the activation URL (xxx.a.pinggy.link:port) into the Ember Android app:' -ForegroundColor Green; .\pinggy.bat"
    } else {
        Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$bundleDir'; `$Host.UI.RawUI.WindowTitle='pinggy'; Write-Host 'Copy the activation URL (xxx.a.pinggy.link:port) into the Ember Android app:' -ForegroundColor Green; .\pinggy.exe -p 443 -R0:127.0.0.1:4433"
    }
    Write-Host "  Pinggy window opened. Copy the activation URL (xxx.a.pinggy.link:port) into the Ember app." -ForegroundColor Green
}

Write-Host "`nDone. Ember stack running." -ForegroundColor Green
'@
$installScript | Set-Content (Join-Path $releaseDir "install-and-start.ps1") -Encoding UTF8

# 9. Create README
$readme = @"
# Ember Server Bundle

Combined release: ember-server + grpc_server + pinggy.

## Contents

- ember-server.exe — QUIC bridge (port 4433)
- grpc_server.exe — LLM inference (port 50051)
- pinggy.exe — Tunnel for remote access
- install-and-start.ps1 — Install service, start all, show activation URL

## Quick Start

1. **Install grpc_server as service** (run PowerShell as Administrator):
   ``.\install-and-start.ps1``

2. **Or start without service** (manual grpc_server):
   ``.\install-and-start.ps1 -NoService``

3. **Activation URL**: When pinggy starts, copy the URL (e.g. ``xxx.a.pinggy.link:443``) from its window into the Ember Android app.

## Manual Start

- grpc_server: ``.\grpc_server.exe`` (or use NSSM service)
- ember-server: ``.\ember-server.exe --port 4433 --inference http://127.0.0.1:50051``
- pinggy: ``.\pinggy.exe -p 443 -R0:127.0.0.1:4433``
"@
$readme | Set-Content (Join-Path $releaseDir "README.md") -Encoding UTF8

# 10. Create zip if requested
if ($Zip) {
    $zipPath = Join-Path $emberDir "release\ember-server-bundle.zip"
    Write-Host "`nCreating zip..." -ForegroundColor Yellow
    Compress-Archive -Path (Join-Path $releaseDir "*") -DestinationPath $zipPath -Force
    Write-Host "  $zipPath" -ForegroundColor Green
}

Write-Host "`nRelease bundle: $releaseDir" -ForegroundColor Green
Get-ChildItem $releaseDir | ForEach-Object { Write-Host "  $($_.Name)" -ForegroundColor Gray }
