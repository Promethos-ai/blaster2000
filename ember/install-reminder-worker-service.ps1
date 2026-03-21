# Install reminder_worker (Feb17) as a Windows service with auto-restart on failure.
# Requires: Run as Administrator, NSSM (winget install NSSM.NSSM)
#
# Usage: .\install-reminder-worker-service.ps1 [-Uninstall]
#   -Uninstall  Remove the service instead of installing
#
# reminder_worker polls reminders from Promethos DB and delivers to ember via push-queue.txt.

param([switch]$Uninstall)

$ErrorActionPreference = "Stop"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "Run as Administrator. Right-click PowerShell -> Run as administrator, then: cd d:\rust\ember; .\install-reminder-worker-service.ps1"
}

$serviceName = "EmberReminderWorker"
$emberDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$feb17Dir = "d:\rust\Feb17"
$nssmDir = Join-Path $emberDir "nssm"
$nssmUrls = @(
    "https://nssm.cc/release/nssm-2.24.zip",
    "https://raw.githubusercontent.com/scavin/nssm-2.24/master/nssm-2.24.zip"
)
$nssmZip = Join-Path $nssmDir "nssm.zip"

$reminderExe = if (Test-Path "$feb17Dir\target\release\reminder_worker.exe") {
    "$feb17Dir\target\release\reminder_worker.exe"
} elseif (Test-Path "$feb17Dir\target\debug\reminder_worker.exe") {
    "$feb17Dir\target\debug\reminder_worker.exe"
} else {
    Write-Error "reminder_worker.exe not found. Build first: cd $feb17Dir; cargo build --bin reminder_worker"
}

$pushQueueFile = Join-Path $emberDir "push-queue.txt"
$logFile = Join-Path $feb17Dir "reminder_worker.log"

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
Write-Host "  Executable: $reminderExe" -ForegroundColor Gray
Write-Host "  REMINDER_PUSH_FILE: $pushQueueFile" -ForegroundColor Gray
Write-Host "  Log file: $logFile" -ForegroundColor Gray

cmd /c "`"$nssm`" stop $serviceName 2>NUL"
cmd /c "`"$nssm`" remove $serviceName confirm 2>NUL"

& $nssm install $serviceName $reminderExe
& $nssm set $serviceName AppDirectory $feb17Dir
& $nssm set $serviceName AppEnvironmentExtra "REMINDER_PUSH_FILE=$pushQueueFile"
& $nssm set $serviceName DisplayName "Ember Reminder Worker"
& $nssm set $serviceName Description "Polls Promethos reminders and delivers to ember via push-queue.txt."
& $nssm set $serviceName Start SERVICE_AUTO_START
& $nssm set $serviceName AppStdout $logFile
& $nssm set $serviceName AppStderr $logFile
& $nssm set $serviceName AppStdoutCreationDisposition 4
& $nssm set $serviceName AppStderrCreationDisposition 4
& $nssm set $serviceName AppExit Default Restart
& $nssm set $serviceName AppRestartDelay 5000

Write-Host "`nService installed. Starting..." -ForegroundColor Green
Start-Service $serviceName

Write-Host "`nEmberReminderWorker is now a Windows service (auto-restart on failure)." -ForegroundColor Green
Write-Host "  Start:  Start-Service EmberReminderWorker" -ForegroundColor Gray
Write-Host "  Stop:   Stop-Service EmberReminderWorker" -ForegroundColor Gray
Write-Host "  Log:    $logFile" -ForegroundColor Gray
