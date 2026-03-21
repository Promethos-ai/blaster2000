# Monitor ember-server.log for "PUSH from file queued" (reminder deliveries).
# Run from ember dir. Press Ctrl+C to stop.
#
# Usage: .\monitor-push-queue.ps1 [LogPath]
#   LogPath defaults to .\ember-server.log

param([string]$LogPath = (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "ember-server.log"))

if (-not (Test-Path $LogPath)) {
    Write-Host "Log file not found: $LogPath" -ForegroundColor Yellow
    Write-Host "Start ember server first (or run as service). Log path may vary." -ForegroundColor Gray
    exit 1
}

Write-Host "Monitoring $LogPath for 'PUSH from file queued' (Ctrl+C to stop)" -ForegroundColor Cyan
Get-Content -Path $LogPath -Wait -Tail 50 | ForEach-Object {
    if ($_ -match "PUSH from file queued") {
        Write-Host $_ -ForegroundColor Green
    }
}
