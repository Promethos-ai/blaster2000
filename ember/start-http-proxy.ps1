# Start HTTP/HTTPS proxy for Flutter web app.
# Browsers cannot use QUIC; this proxy accepts HTTP(S) and forwards to ember over QUIC.
#
# Run this when testing the Flutter app in Chrome.
# Default: HTTPS on port 8443 (self-signed cert). Use --http for plain HTTP on 8080.
#
# Usage: .\start-http-proxy.ps1              # HTTPS on 8443
#        .\start-http-proxy.ps1 --http        # HTTP on 8080
#        .\start-http-proxy.ps1 --port 9443   # HTTPS on custom port

$extraArgs = @()
if ($args -contains "--http") {
    $extraArgs = @("--http")
    $port = 8080
    $scheme = "http"
} else {
    $port = 8443
    $scheme = "https"
}
if ($args -contains "--port" -and $args.Count -gt 1) {
    $idx = [array]::IndexOf($args, "--port")
    $port = $args[$idx + 1]
    $extraArgs = $extraArgs + @("--port", $port)
} elseif ($extraArgs.Count -eq 0) {
    $extraArgs = @("--port", $port)
}

$emberDir = if (Test-Path "d:\rust\ember") { "d:\rust\ember" } else { $PSScriptRoot }
Write-Host "Starting proxy ($scheme://localhost:$port)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$emberDir'; `$Host.UI.RawUI.WindowTitle='ember-http-proxy'; Write-Host 'Proxy - $scheme://localhost:$port (forwards to ember QUIC)' -ForegroundColor Green; cargo run --bin http_proxy -p ember-client --features http_proxy -- $extraArgs"
)
Write-Host "Proxy: $scheme`://localhost:$port/ask" -ForegroundColor Green
Write-Host "Web app default: $scheme`://localhost:$port" -ForegroundColor Green
