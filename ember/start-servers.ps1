# Start both servers in separate logging windows.
# 1. grpc_server (Feb17) - TCP 50051, LLM inference
# 2. ember-server - UDP 4433, QUIC bridge to grpc_server
#
# Uses TCP for ember→Feb17 (avoids QUIC handshake timeout / cert mismatch).
#
# Run from ember directory or project root.

$emberDir = if (Test-Path "d:\rust\ember") { "d:\rust\ember" } else { $PSScriptRoot }
$feb17Dir = "d:\rust\Feb17"

# Start grpc_server first (ember-server connects to it) - TCP mode
Write-Host "Starting grpc_server (Feb17) on TCP 50051..." -ForegroundColor Cyan
$grpcCmd = if (Test-Path "$feb17Dir\target\release\grpc_server.exe") { "& .\target\release\grpc_server.exe" } else { "cargo run --bin grpc_server" }
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$feb17Dir'; `$Host.UI.RawUI.WindowTitle='grpc_server (Feb17)'; Write-Host 'grpc_server - Feb17 LLM inference (TCP 50051)' -ForegroundColor Green; $grpcCmd"
)

Start-Sleep -Seconds 2

# Start ember-server (TCP to Feb17 - reliable, no QUIC handshake issues)
Write-Host "Starting ember-server on UDP 4433..." -ForegroundColor Cyan
$emberCmd = if (Test-Path "$emberDir\target\release\ember-server.exe") { "& .\target\release\ember-server.exe --inference http://127.0.0.1:50051 --web-search" } else { "cargo run -p ember-server -- --inference http://127.0.0.1:50051 --web-search" }
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$emberDir'; `$Host.UI.RawUI.WindowTitle='ember-server'; Write-Host 'ember-server - QUIC bridge (port 4433)' -ForegroundColor Green; $emberCmd"
)

Write-Host "`nBoth servers started. Close the windows to stop them." -ForegroundColor Yellow
