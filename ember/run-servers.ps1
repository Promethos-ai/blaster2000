# Restart ember-server and Feb17 grpc_server with detailed logging.
# Run each in a separate terminal for live output.
#
# Usage:
#   Terminal 1: cd d:\rust\Feb17; cargo run --bin grpc_server
#   Terminal 2: cd d:\rust\ember; .\target\debug\ember-server.exe

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$feb17Dir = Join-Path (Split-Path -Parent $rootDir) "Feb17"

# Remove file locks before running
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}

Write-Host ""
Write-Host "Start both servers in separate terminals for detailed logging:" -ForegroundColor Green
Write-Host ""
Write-Host "  Terminal 1 (inference - model load ~30s):" -ForegroundColor Cyan
Write-Host "    cd $feb17Dir" -ForegroundColor Gray
Write-Host "    cargo run --bin grpc_server" -ForegroundColor Gray
Write-Host ""
Write-Host "  Terminal 2 (QUIC bridge):" -ForegroundColor Cyan
Write-Host "    cd $rootDir" -ForegroundColor Gray
Write-Host "    .\target\debug\ember-server.exe" -ForegroundColor Gray
Write-Host ""
