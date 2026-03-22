# Install grpc_server (Feb17 inference) as a Windows service.
# Requires: Run as Administrator
#
# Usage: .\install-grpc-service.ps1 [-Uninstall]
#   -Uninstall  Remove the service instead of installing

param([switch]$Uninstall)

$ErrorActionPreference = "Stop"

# Require Administrator (NSSM needs it for service install)
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "Run as Administrator. Right-click PowerShell -> Run as administrator, then: cd d:\rust\ember; .\install-grpc-service.ps1"
}
$serviceName = "EmberGrpcServer"
$feb17Dir = "d:\rust\Feb17"
$nssmDir = "d:\rust\ember\nssm"
$nssmUrls = @(
    "https://nssm.cc/release/nssm-2.24.zip",
    "https://raw.githubusercontent.com/scavin/nssm-2.24/master/nssm-2.24.zip"
)
$nssmZip = Join-Path $nssmDir "nssm.zip"

# Resolve grpc_server path (prefer release)
$grpcExe = if (Test-Path "$feb17Dir\target\release\grpc_server.exe") {
    "$feb17Dir\target\release\grpc_server.exe"
} elseif (Test-Path "$feb17Dir\target\debug\grpc_server.exe") {
    "$feb17Dir\target\debug\grpc_server.exe"
} else {
    Write-Error "grpc_server.exe not found. Build first: cd $feb17Dir; cargo build --bin grpc_server --features cuda"
}

$logFile = "$feb17Dir\grpc_server.log"

function Ensure-Nssm {
    # Prefer nssm from PATH (e.g. winget install NSSM.NSSM)
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

# Install
$nssm = Ensure-Nssm
$wrapperScript = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "scripts\grpc-wrapper.ps1"
if (-not (Test-Path $wrapperScript)) { Write-Error "Wrapper not found: $wrapperScript" }

Write-Host "Installing $serviceName as Windows service..." -ForegroundColor Cyan
Write-Host "  Wrapper: $wrapperScript (logs crash, NSSM restarts)" -ForegroundColor Gray
Write-Host "  Log file: $logFile" -ForegroundColor Gray

# Remove if exists (idempotent). Suppress "Can't open service!" when service doesn't exist.
cmd /c "`"$nssm`" stop $serviceName 2>NUL"
cmd /c "`"$nssm`" remove $serviceName confirm 2>NUL"

# Use wrapper: on crash, writes to log and exits so NSSM restarts
$powershellExe = (Get-Command powershell.exe).Source
& $nssm install $serviceName $powershellExe "-NoProfile" "-ExecutionPolicy" "Bypass" "-File" $wrapperScript
& $nssm set $serviceName AppDirectory $feb17Dir
& $nssm set $serviceName DisplayName "Ember gRPC Inference Server"
& $nssm set $serviceName Description "Feb17 LLM inference (llama.cpp). Serves on TCP 50051 for ember-server."
& $nssm set $serviceName Start SERVICE_AUTO_START
& $nssm set $serviceName AppStdout $logFile
& $nssm set $serviceName AppStderr $logFile
& $nssm set $serviceName AppStdoutCreationDisposition 4
& $nssm set $serviceName AppStderrCreationDisposition 4
& $nssm set $serviceName AppExit Default Restart
& $nssm set $serviceName AppRestartDelay 5000

Write-Host "`nService installed. Starting..." -ForegroundColor Green
Start-Service $serviceName

Write-Host "`nEmberGrpcServer is now a Windows service (auto-restart on failure)." -ForegroundColor Green
Write-Host "  Start:  Start-Service EmberGrpcServer" -ForegroundColor Gray
Write-Host "  Stop:   Stop-Service EmberGrpcServer" -ForegroundColor Gray
Write-Host "  Status: Get-Service EmberGrpcServer" -ForegroundColor Gray
Write-Host "  Log:    $logFile" -ForegroundColor Gray
