# Install pinggy tunnel as a Windows service with auto-restart on failure.
# Requires: Run as Administrator, NSSM (winget install NSSM.NSSM)
#
# Usage: .\install-pinggy-service.ps1 [-Uninstall] [-PinggyAuth "token@pro.pinggy.io"]
#   -Uninstall    Remove the service instead of installing
#   -PinggyAuth   Auth token (or set env PINGGY_AUTH). If omitted, reads from pinggy.bat.
#
# Tunnel forwards remote connections to 127.0.0.1:4433 (ember-server).

param([switch]$Uninstall, [string]$PinggyAuth = "")

$ErrorActionPreference = "Stop"
$serviceName = "EmberPinggy"
$emberDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$nssmDir = Join-Path $emberDir "nssm"
$nssmUrls = @(
    "https://nssm.cc/release/nssm-2.24.zip",
    "https://raw.githubusercontent.com/scavin/nssm-2.24/master/nssm-2.24.zip"
)
$nssmZip = Join-Path $nssmDir "nssm.zip"
$pinggyUrl = "https://s3.ap-south-1.amazonaws.com/public.pinggy.binaries/cli/v0.2.5/windows/amd64/pinggy.exe"

$pinggyExe = if (Test-Path "$emberDir\pinggy.exe") {
    "$emberDir\pinggy.exe"
} elseif (Test-Path "$emberDir\release\ember-server-bundle\pinggy.exe") {
    "$emberDir\release\ember-server-bundle\pinggy.exe"
} else {
    Write-Host "Downloading pinggy.exe..." -ForegroundColor Cyan
    $path = Join-Path $emberDir "pinggy.exe"
    try {
        Invoke-WebRequest -Uri $pinggyUrl -OutFile $path -UseBasicParsing
    } catch {
        Write-Error "pinggy.exe not found and download failed. Get it from: $pinggyUrl"
    }
    $path
}

$logFile = Join-Path $emberDir "pinggy.log"

# Parse auth: param > env > pinggy.secret > pinggy.bat
$auth = $PinggyAuth
if ([string]::IsNullOrEmpty($auth) -and $env:PINGGY_AUTH) {
    $auth = $env:PINGGY_AUTH
}
if ([string]::IsNullOrEmpty($auth) -and (Test-Path "$emberDir\pinggy.secret")) {
    $line = Get-Content "$emberDir\pinggy.secret" | Where-Object { $_ -match '\S' -and $_ -notmatch '^\s*#' } | Select-Object -First 1
    if ($line) {
        $token = $line.Trim()
        $auth = if ($token -match '@pro\.pinggy\.io$') { $token } else { "$token@pro.pinggy.io" }
        Write-Host "  Using auth from pinggy.secret" -ForegroundColor Gray
    }
}
if ([string]::IsNullOrEmpty($auth) -and (Test-Path "$emberDir\pinggy.bat")) {
    $batContent = Get-Content "$emberDir\pinggy.bat" -Raw
    if ($batContent -match '\s+([^\s]+@pro\.pinggy\.io)\s*$') {
        $auth = $matches[1].Trim()
        Write-Host "  Using auth from pinggy.bat" -ForegroundColor Gray
    }
}

$appParams = "-p 443 -R0:127.0.0.1:4433"
if (-not [string]::IsNullOrEmpty($auth)) {
    $appParams = "$appParams $auth"
}

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
Write-Host "  Executable: $pinggyExe" -ForegroundColor Gray
Write-Host "  Params: $appParams" -ForegroundColor Gray
Write-Host "  Log file: $logFile" -ForegroundColor Gray

& $nssm stop $serviceName 2>$null
& $nssm remove $serviceName confirm 2>$null

& $nssm install $serviceName $pinggyExe
& $nssm set $serviceName AppParameters $appParams
& $nssm set $serviceName AppDirectory $emberDir
& $nssm set $serviceName DisplayName "Ember Pinggy Tunnel"
& $nssm set $serviceName Description "Pinggy tunnel for remote Ember access. Forwards to 127.0.0.1:4433."
& $nssm set $serviceName Start SERVICE_AUTO_START
& $nssm set $serviceName AppStdout $logFile
& $nssm set $serviceName AppStderr $logFile
& $nssm set $serviceName AppStdoutCreationDisposition 4
& $nssm set $serviceName AppStderrCreationDisposition 4
& $nssm set $serviceName AppExit Default Restart
& $nssm set $serviceName AppRestartDelay 5000

# Start after EmberServer if that service exists
$emberSvc = Get-Service -Name "EmberServer" -ErrorAction SilentlyContinue
if ($emberSvc) {
    sc.exe config $serviceName depend= EmberServer 2>$null
    if ($LASTEXITCODE -eq 0) { Write-Host "  Depends on EmberServer (starts after ember-server)" -ForegroundColor Gray }
}

Write-Host "`nService installed. Starting..." -ForegroundColor Green
Start-Service $serviceName

Write-Host "`nEmberPinggy is now a Windows service (auto-restart on failure)." -ForegroundColor Green
Write-Host "  Start:  Start-Service EmberPinggy" -ForegroundColor Gray
Write-Host "  Stop:   Stop-Service EmberPinggy" -ForegroundColor Gray
Write-Host "  Log:    $logFile" -ForegroundColor Gray
Write-Host "  Activation URL: Check $logFile for xxx.a.pinggy.link:443 (no window when run as service)" -ForegroundColor Gray
