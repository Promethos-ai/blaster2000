# Install ember-server as a Windows service with auto-restart on failure.
# Requires: Run as Administrator, NSSM (winget install NSSM.NSSM)
#
# Usage: .\install-ember-service.ps1 [-Uninstall]
#   -Uninstall  Remove the service instead of installing

param([switch]$Uninstall)

$ErrorActionPreference = "Stop"
$serviceName = "EmberServer"
$emberDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$nssmDir = Join-Path $emberDir "nssm"
$nssmUrls = @(
    "https://nssm.cc/release/nssm-2.24.zip",
    "https://raw.githubusercontent.com/scavin/nssm-2.24/master/nssm-2.24.zip"
)
$nssmZip = Join-Path $nssmDir "nssm.zip"

$emberExe = if (Test-Path "$emberDir\target\release\ember-server.exe") {
    "$emberDir\target\release\ember-server.exe"
} else {
    Write-Error "ember-server.exe not found. Build first: cd $emberDir; cargo build -p ember-server --release"
}

$logFile = Join-Path $emberDir "ember-server.log"

function Ensure-Nssm {
    $nssmInPath = Get-Command nssm -ErrorAction SilentlyContinue
    if ($nssmInPath) { return $nssmInPath.Source }
    $nssmExe = Get-ChildItem $nssmDir -Filter "nssm.exe" -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "win64" } | Select-Object -First 1
    if (-not $nssmExe) {
        $nssmExe = Get-ChildItem $nssmDir -Filter "nssm.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    }
    if ($nssmExe) { return $nssmExe.FullName }
    Write-Host "Downloading NSSM..." -ForegroundColor Cyan
    New-Item -ItemType Directory -Force -Path $nssmDir | Out-Null
    $downloaded = $false
    foreach ($url in $nssmUrls) {
        try {
            Invoke-WebRequest -Uri $url -OutFile $nssmZip -UseBasicParsing -ErrorAction Stop
            $downloaded = $true
            break
        } catch { Write-Host "  $url failed, trying next..." -ForegroundColor Gray }
    }
    if (-not $downloaded) { Write-Error "NSSM download failed from all sources" }
    Expand-Archive -Path $nssmZip -DestinationPath $nssmDir -Force
    $nssmExe = Get-ChildItem $nssmDir -Filter "nssm.exe" -Recurse | Select-Object -First 1
    if (-not $nssmExe) { Write-Error "NSSM download failed" }
    Remove-Item $nssmZip -Force -ErrorAction SilentlyContinue
    return $nssmExe.FullName
}

if ($Uninstall) {
    $nssm = Ensure-Nssm
    Write-Host "Removing service $serviceName..." -ForegroundColor Yellow
    & $nssm stop $serviceName 2>$null
    & $nssm remove $serviceName confirm
    Write-Host "Service removed." -ForegroundColor Green
    exit 0
}

$nssm = Ensure-Nssm
Write-Host "Installing $serviceName as Windows service..." -ForegroundColor Cyan
Write-Host "  Executable: $emberExe" -ForegroundColor Gray
Write-Host "  Log file: $logFile" -ForegroundColor Gray

& $nssm stop $serviceName 2>$null
& $nssm remove $serviceName confirm 2>$null

& $nssm install $serviceName $emberExe
& $nssm set $serviceName AppParameters "--port 4433 --inference http://127.0.0.1:50051 --instructions-file instructions.txt --promethos --promethos-base d:\rust\Feb17"
& $nssm set $serviceName AppDirectory $emberDir
& $nssm set $serviceName DisplayName "Ember QUIC Server"
& $nssm set $serviceName Description "QUIC bridge for Ember Android app. Listens on UDP 4433, forwards to grpc_server."
& $nssm set $serviceName Start SERVICE_AUTO_START
& $nssm set $serviceName AppStdout $logFile
& $nssm set $serviceName AppStderr $logFile
& $nssm set $serviceName AppStdoutCreationDisposition 4
& $nssm set $serviceName AppStderrCreationDisposition 4
& $nssm set $serviceName AppExit Default Restart
& $nssm set $serviceName AppRestartDelay 5000

# Start after EmberGrpcServer if that service exists
$grpcSvc = Get-Service -Name "EmberGrpcServer" -ErrorAction SilentlyContinue
if ($grpcSvc) {
    sc.exe config $serviceName depend= EmberGrpcServer 2>$null
    if ($LASTEXITCODE -eq 0) { Write-Host "  Depends on EmberGrpcServer (starts after grpc)" -ForegroundColor Gray }
}

Write-Host "`nService installed. Starting..." -ForegroundColor Green
Start-Service $serviceName

Write-Host "`nEmberServer is now a Windows service (auto-restart on failure)." -ForegroundColor Green
Write-Host "  Start:  Start-Service EmberServer" -ForegroundColor Gray
Write-Host "  Stop:   Stop-Service EmberServer" -ForegroundColor Gray
Write-Host "  Log:    $logFile" -ForegroundColor Gray
