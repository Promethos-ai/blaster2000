# Start grpc_server and ember-server with output logged to files for monitoring.
# Use: Get-Content ember-server.log -Wait -Tail 20  (in another terminal) to tail.
#
# Run from ember directory.

$emberDir = if (Test-Path "d:\rust\ember") { "d:\rust\ember" } else { $PSScriptRoot }
$feb17Dir = "d:\rust\Feb17"

if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "Removing file locks..." -ForegroundColor Yellow
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 2
}

$grpcLog = "$emberDir\grpc_server.log"
$emberLog = "$emberDir\ember-server.log"

Write-Host "Starting grpc_server (log: $grpcLog)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$feb17Dir'; `$Host.UI.RawUI.WindowTitle='grpc_server'; .\target\release\grpc_server.exe 2>&1 | Tee-Object -FilePath '$grpcLog'"
)

Write-Host "Waiting for grpc_server..." -ForegroundColor Yellow
$waited = 0
while ($waited -lt 120) {
    try {
        if ((Test-NetConnection -ComputerName 127.0.0.1 -Port 50051 -WarningAction SilentlyContinue).TcpTestSucceeded) { break }
    } catch {}
    Start-Sleep -Seconds 5
    $waited += 5
}
Write-Host "grpc_server ready." -ForegroundColor Green

$loadBrave = "if (Test-Path 'config\search.json') { `$j = Get-Content 'config\search.json' -Raw | ConvertFrom-Json; if (`$j.api_key) { `$env:BRAVE_API_KEY = `$j.api_key } elseif (`$j.brave_api_key) { `$env:BRAVE_API_KEY = `$j.brave_api_key } }; if (-not `$env:BRAVE_API_KEY -and (Test-Path '.env')) { Get-Content '.env' | ForEach-Object { if (`$_ -match '^\s*BRAVE_API_KEY=(.+)$') { `$env:BRAVE_API_KEY = `$matches[1].Trim() } } }; "
Write-Host "Starting ember-server (log: $emberLog)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$emberDir'; $loadBrave `$Host.UI.RawUI.WindowTitle='ember-server'; .\target\release\ember-server.exe --port 4433 --inference http://127.0.0.1:50051 --web-search --web-search-always 2>&1 | Tee-Object -FilePath '$emberLog'"
)

Write-Host "`nBoth servers started. Logs: $grpcLog, $emberLog" -ForegroundColor Yellow
Write-Host "To monitor: Get-Content $emberLog -Wait -Tail 30" -ForegroundColor Gray
