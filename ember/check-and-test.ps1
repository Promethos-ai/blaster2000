# Check all components and test connection to pinggy
# Run from d:\rust\ember

$ErrorActionPreference = "Continue"

Write-Host "`n=== CHECK: All Components ===" -ForegroundColor Cyan

# 1. grpc_server (TCP 50051)
$grpc = netstat -ano | Select-String ":50051.*LISTENING"
if ($grpc) {
    Write-Host "[OK] grpc_server (inference) - TCP 50051 listening" -ForegroundColor Green
} else {
    Write-Host "[MISSING] grpc_server - NOT running. Start: d:\rust\Feb17\target\release\grpc_server.exe" -ForegroundColor Red
}

# 2. ember-server (UDP 4433)
$ember = netstat -ano | Select-String ":4433.*UDP"
if ($ember) {
    Write-Host "[OK] ember-server (QUIC bridge) - UDP 4433 listening" -ForegroundColor Green
} else {
    Write-Host "[MISSING] ember-server - NOT running. Start: cargo run -p ember-server" -ForegroundColor Red
}

# 3. Pinggy hostname
Write-Host "`n=== CHECK: Pinggy URL ===" -ForegroundColor Cyan
$default = (Get-Content "android\app\src\main\res\values\server_defaults.xml" | Select-String "default_server_address").ToString() -replace '<[^>]+>',''
Write-Host "App default: $default" -ForegroundColor Gray
Write-Host "Pinggy shows URL in its window - use that host:port (e.g. jkazjsnynw.a.pinggy.link:10822)" -ForegroundColor Gray

# 4. Resolve pinggy
$hostPart = ($default -split ":")[0]
Write-Host "`nResolving $hostPart..." -ForegroundColor Gray
try {
    $dns = Resolve-DnsName $hostPart -ErrorAction SilentlyContinue
    if ($dns) { Write-Host "[OK] DNS resolves: $($dns[0].IPAddress)" -ForegroundColor Green }
    else { Write-Host "[WARN] DNS lookup failed" -ForegroundColor Yellow }
} catch {
    Write-Host "[WARN] DNS: $($_.Exception.Message)" -ForegroundColor Yellow
}

# 5. Test connection (if ember-client exists)
Write-Host "`n=== TEST: Connection ===" -ForegroundColor Cyan
$client = $null
if (Test-Path "target\debug\ember-client.exe") { $client = "target\debug\ember-client.exe" }
elseif (Test-Path "..\target\debug\ember-client.exe") { $client = "..\target\debug\ember-client.exe" }

if ($client) {
    Write-Host "Running: $client `"$default`" `"What is 2+2?`"" -ForegroundColor Gray
    $result = & $client $default "What is 2+2?" 2>&1
    Write-Host $result
    if ($result -match "Error|connection") {
        Write-Host "[FAIL] Connection test failed" -ForegroundColor Red
    } else {
        Write-Host "[OK] Connection test - got response" -ForegroundColor Green
    }
} else {
    Write-Host "ember-client not built. Run: cargo build -p ember-client" -ForegroundColor Yellow
    Write-Host "Then test manually: cargo run -p ember-client -- $default `"What is 2+2?`"" -ForegroundColor Gray
}

Write-Host "`n=== Summary ===" -ForegroundColor Cyan
Write-Host "1. Start grpc_server (if not running): d:\rust\Feb17\target\release\grpc_server.exe"
Write-Host "2. Start ember-server: cargo run -p ember-server"
Write-Host "3. Start pinggy: .\pinggy.bat"
Write-Host "4. Android app: Enter the URL from pinggy window (host:port)"
Write-Host ""
