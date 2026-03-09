# Detect default server address for the app (local IP or ngrok).
# Run before building Android. Writes to android/app/src/main/res/values/server_defaults.xml

$ErrorActionPreference = "SilentlyContinue"
$rootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$outFile = Join-Path $rootDir "android\app\src\main\res\values\server_defaults.xml"

# 1. Try ngrok API (when ngrok is running with web interface)
$ngrokUrl = $null
try {
    $tunnels = Invoke-RestMethod -Uri "http://127.0.0.1:4040/api/tunnels" -TimeoutSec 2
    if ($tunnels.tunnels) {
        $tcp = $tunnels.tunnels | Where-Object { $_.proto -eq "tcp" } | Select-Object -First 1
        if ($tcp) {
            $ngrokUrl = $tcp.public_url -replace "^tcp://", ""
            $ngrokUrl = $ngrokUrl -replace "^https?://", ""
        }
    }
} catch {}

# 2. Fallback: get local IPv4 (first non-loopback)
$localIp = $null
$addrs = Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notmatch "Loopback" -and $_.IPAddress -notmatch "^127\." }
$lan = $addrs | Where-Object { $_.IPAddress -match "^192\.168\.|^10\.|^172\.(1[6-9]|2[0-9]|3[0-1])\." } | Select-Object -First 1
if ($lan) { $localIp = $lan.IPAddress }
elseif ($addrs) { $localIp = $addrs[0].IPAddress }

# 3. Choose address
$addr = if ($ngrokUrl) { $ngrokUrl } else { "${localIp}:4433" }
if (-not $addr -or $addr -eq ":4433") { $addr = "192.168.1.100:4433" }

# 4. Write resource file
$dir = Split-Path $outFile
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

$xml = @"
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="default_server_address">$addr</string>
</resources>
"@
Set-Content -Path $outFile -Value $xml -Encoding UTF8

Write-Host "Default server address: $addr" -ForegroundColor Cyan
if ($ngrokUrl) { Write-Host "  (from ngrok)" -ForegroundColor Gray }
else { Write-Host "  (local IP)" -ForegroundColor Gray }
