# Start both servers in separate logging windows.
# 1. grpc_server (Feb17) - TCP 50051, LLM inference
# 2. ember-server - UDP 4433, QUIC bridge to grpc_server
#
# Run from ember directory or project root.

$emberDir = if (Test-Path "d:\rust\ember") { "d:\rust\ember" } else { $PSScriptRoot }
$feb17Dir = "d:\rust\Feb17"

# Start grpc_server first (ember-server connects to it)
Write-Host "Starting grpc_server (Feb17) on TCP 50051..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$feb17Dir'; Write-Host 'grpc_server - Feb17 LLM inference (port 50051)' -ForegroundColor Green; cargo run --bin grpc_server"
)

Start-Sleep -Seconds 2

# Start ember-server
Write-Host "Starting ember-server on UDP 4433..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$emberDir'; Write-Host 'ember-server - QUIC bridge (port 4433)' -ForegroundColor Green; cargo run -p ember-server"
)

Write-Host "`nBoth servers started. Close the windows to stop them." -ForegroundColor Yellow
