# Start both servers in separate logging windows.
# 1. grpc_server (Feb17) - TCP 50051, LLM inference
# 2. ember-server - UDP 4433, QUIC bridge to grpc_server
#
# Uses TCP for ember→Feb17 (avoids QUIC handshake timeout / cert mismatch).
#
# Run from ember directory or project root.

$emberDir = if (Test-Path "d:\rust\ember") { "d:\rust\ember" } else { $PSScriptRoot }
$feb17Dir = "d:\rust\Feb17"

# Remove file locks before starting (kills cargo/rustc/servers, clears .cargo-lock)
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}

# Start grpc_server first (ember-server connects to it) - TCP mode
Write-Host "Starting grpc_server (Feb17) on TCP 50051..." -ForegroundColor Cyan
$grpcCmd = if (Test-Path "$feb17Dir\target\release\grpc_server.exe") { "& .\target\release\grpc_server.exe" } else { "cargo run --bin grpc_server" }
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$feb17Dir'; `$Host.UI.RawUI.WindowTitle='grpc_server (Feb17)'; Write-Host 'grpc_server - Feb17 LLM inference (TCP 50051)' -ForegroundColor Green; $grpcCmd"
)

# Wait for grpc_server to accept connections (model load can take 1-2 min)
Write-Host "Waiting for grpc_server (port 50051)..." -ForegroundColor Yellow
$maxWait = 120
$waited = 0
$ready = $false
while ($waited -lt $maxWait) {
    try {
        if ((Test-NetConnection -ComputerName 127.0.0.1 -Port 50051 -WarningAction SilentlyContinue).TcpTestSucceeded) {
            $ready = $true
            break
        }
    } catch {}
    Start-Sleep -Seconds 5
    $waited += 5
    Write-Host "  ... still waiting ($waited s)" -ForegroundColor Gray
}
if ($ready) { Write-Host "grpc_server ready." -ForegroundColor Green }
else { Write-Host "WARNING: grpc_server may not be ready. Ember will retry on first request." -ForegroundColor Yellow }

# Start ember-server (TCP to Feb17 - reliable, no QUIC handshake issues)
# Load BRAVE_API_KEY from .env for realtime web search
$loadBrave = "if (Test-Path '.env') { Get-Content '.env' | ForEach-Object { if (`$_ -match '^\s*BRAVE_API_KEY=(.+)$') { `$env:BRAVE_API_KEY = `$matches[1].Trim() } } }; "
Write-Host "Starting ember-server on UDP 4433 (Brave realtime search)..." -ForegroundColor Cyan
$emberCmd = if (Test-Path "$emberDir\target\release\ember-server.exe") { "& .\target\release\ember-server.exe --port 4433 --inference http://127.0.0.1:50051 --web-search --web-search-always" } else { "cargo run -p ember-server -- --port 4433 --inference http://127.0.0.1:50051 --web-search --web-search-always" }
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$emberDir'; $loadBrave `$Host.UI.RawUI.WindowTitle='ember-server'; Write-Host 'ember-server - QUIC bridge + Brave realtime (port 4433)' -ForegroundColor Green; $emberCmd"
)

Write-Host "`nBoth servers started. Close the windows to stop them." -ForegroundColor Yellow
