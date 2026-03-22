# Install all Ember stack as Windows services with auto-restart.
# Requires: Run as Administrator, NSSM (winget install NSSM.NSSM)
#
# Usage: .\install-all-services.ps1 [-Uninstall]
#   -Uninstall  Remove all services
#
# Order: EmberGrpcServer -> EmberServer -> EmberPromethosWorker -> EmberReminderWorker -> EmberPinggy

param([switch]$Uninstall)

$ErrorActionPreference = "Stop"
$emberDir = Split-Path -Parent $MyInvocation.MyCommand.Path

if ($Uninstall) {
    Write-Host "Removing all Ember services..." -ForegroundColor Yellow
    & "$emberDir\install-pinggy-service.ps1" -Uninstall
    & "$emberDir\install-ember-service.ps1" -Uninstall
    & "$emberDir\install-reminder-worker-service.ps1" -Uninstall
    & "$emberDir\install-promethos-worker-service.ps1" -Uninstall
    & "$emberDir\install-grpc-service.ps1" -Uninstall
    Write-Host "`nAll services removed." -ForegroundColor Green
    exit 0
}

Write-Host "=== Installing Ember Stack as Windows Services ===" -ForegroundColor Cyan
Write-Host ""

# 1. grpc_server (inference) - no dependencies
& "$emberDir\install-grpc-service.ps1"
Write-Host ""

# 2. ember-server - depends on grpc (ember retries if grpc not ready)
& "$emberDir\install-ember-service.ps1"
Write-Host ""

# 3. promethos_worker - extracts reminders from interactions, inserts into reminders table
& "$emberDir\install-promethos-worker-service.ps1"
Write-Host ""

# 4. reminder_worker - polls Promethos reminders, delivers to push-queue.txt
& "$emberDir\install-reminder-worker-service.ps1"
Write-Host ""

# 5. pinggy - depends on ember-server
& "$emberDir\install-pinggy-service.ps1"
Write-Host ""

Write-Host "=== All services installed ===" -ForegroundColor Green
Write-Host "  EmberGrpcServer      - LLM inference (port 50051)" -ForegroundColor Gray
Write-Host "  EmberServer          - QUIC bridge (port 4433, --promethos)" -ForegroundColor Gray
Write-Host "  EmberPromethosWorker - Cold-path: extract reminders from interactions" -ForegroundColor Gray
Write-Host "  EmberReminderWorker  - Reminder delivery (push-queue.txt)" -ForegroundColor Gray
Write-Host "  EmberPinggy          - Remote tunnel (xxx.a.pinggy.link:443)" -ForegroundColor Gray
Write-Host ""
Write-Host "Activation URL: Check ember\pinggy.log for the pinggy link, or run pinggy manually once to see it." -ForegroundColor Yellow
