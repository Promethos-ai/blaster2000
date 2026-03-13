# Monitor grpc_server and ember-server logs for pinggy requests.
# Run this while sending prompts from the Android app.
#
# Usage: .\monitor-logs.ps1
# Or: .\monitor-logs.ps1 -GrpcLog "d:\rust\Feb17\grpc-server.log"

param(
    [string]$GrpcLog = "d:\rust\Feb17\grpc-server.log",
    [string]$EmberLog = "d:\rust\ember\ember-connections.log"
)

$ErrorActionPreference = "Continue"

Write-Host "=== Log Monitor (pinggy requests) ===" -ForegroundColor Cyan
Write-Host "Watching: $GrpcLog" -ForegroundColor Gray
Write-Host "Watching: $EmberLog" -ForegroundColor Gray
Write-Host "Send a prompt from the Android app to see activity." -ForegroundColor Yellow
Write-Host ""

# Create grpc log if it doesn't exist (so we can tail it)
if (-not (Test-Path $GrpcLog)) {
    "" | Out-File $GrpcLog -Encoding utf8
}

$lastGrpcSize = 0
$lastEmberSize = 0

while ($true) {
    Start-Sleep -Seconds 2

    # grpc_server log
    if (Test-Path $GrpcLog) {
        $grpc = Get-Item $GrpcLog -ErrorAction SilentlyContinue
        if ($grpc.Length -gt $lastGrpcSize) {
            $content = Get-Content $GrpcLog -Tail 20 -ErrorAction SilentlyContinue
            $newLines = $content | Select-Object -Skip ([Math]::Max(0, $content.Count - 5))
            foreach ($line in $newLines) {
                if ($line -match "RECV|SEND|CompleteStream|Complete request") {
                    Write-Host "[grpc] $line" -ForegroundColor Green
                }
            }
            $lastGrpcSize = $grpc.Length
        }
    }

    # ember-connections.log
    if (Test-Path $EmberLog) {
        $ember = Get-Item $EmberLog -ErrorAction SilentlyContinue
        if ($ember.Length -gt $lastEmberSize) {
            $content = Get-Content $EmberLog -Tail 20 -ErrorAction SilentlyContinue
            $newLines = $content | Select-Object -Skip ([Math]::Max(0, $content.Count - 5))
            foreach ($line in $newLines) {
                Write-Host "[ember] $line" -ForegroundColor Cyan
            }
            $lastEmberSize = $ember.Length
        }
    }
}
