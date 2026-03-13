# Start grpc_server and ember-server with file logging for monitoring.
# Logs: d:\rust\Feb17\grpc-server.log, d:\rust\ember\ember-connections.log
#
# Usage: .\start-with-logging.ps1

$feb17Dir = "d:\rust\Feb17"
$emberDir = "d:\rust\ember"
$grpcLog = "$feb17Dir\grpc-server.log"
$emberLog = "$emberDir\ember-connections.log"

Write-Host "Starting grpc_server with log: $grpcLog" -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$feb17Dir'; `$exe = if (Test-Path 'target\release\grpc_server.exe') { 'target\release\grpc_server.exe' } else { 'target\debug\grpc_server.exe' }; Write-Host 'grpc_server - logging to grpc-server.log' -ForegroundColor Green; & `$exe --quic --host localhost --log-file grpc-server.log"
)

Start-Sleep -Seconds 3

Write-Host "Starting ember-server with log: $emberLog" -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$emberDir'; Write-Host 'ember-server (QUIC bridge) - logging to ember-connections.log' -ForegroundColor Green; cargo run -p ember-server"
)

Write-Host "`nTo monitor logs: .\monitor-logs.ps1" -ForegroundColor Yellow
Write-Host "Then start pinggy: .\pinggy.bat" -ForegroundColor Yellow
