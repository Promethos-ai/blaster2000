# Build Feb17 grpc_server. Removes file locks first.
# Usage: .\build-grpc.ps1 [--run]
#   --run  Start grpc_server after building

param([switch]$Run)

$ErrorActionPreference = "Stop"
$feb17Dir = "d:\rust\Feb17"

# Remove file locks before building
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}

Write-Host "Building grpc_server..." -ForegroundColor Cyan
Push-Location $feb17Dir
try {
    cargo build --bin grpc_server --features cuda
    if ($LASTEXITCODE -ne 0) { exit 1 }
    if ($Run) {
        Write-Host "`nStarting grpc_server..." -ForegroundColor Green
        Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$feb17Dir'; .\target\debug\grpc_server.exe"
    }
} finally {
    Pop-Location
}
