# Run Pinggy tunnel using auth from pinggy.secret.
# Usage: .\pinggy.ps1

$ErrorActionPreference = "Stop"
$emberDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$secretFile = Join-Path $emberDir "pinggy.secret"
$pinggyExe = Join-Path $emberDir "pinggy.exe"

$auth = $env:PINGGY_AUTH
if ([string]::IsNullOrWhiteSpace($auth) -and (Test-Path $secretFile)) {
    $line = Get-Content $secretFile | Where-Object { $_ -match '\S' -and $_ -notmatch '^\s*#' } | Select-Object -First 1
    if ($line) {
        $token = $line.Trim()
        $auth = if ($token -match '@pro\.pinggy\.io$') { $token } else { "$token@pro.pinggy.io" }
    }
}

if (-not (Test-Path $pinggyExe)) {
    Write-Host "Downloading pinggy.exe..." -ForegroundColor Cyan
    $url = "https://s3.ap-south-1.amazonaws.com/public.pinggy.binaries/cli/v0.2.5/windows/amd64/pinggy.exe"
    Invoke-WebRequest -Uri $url -OutFile $pinggyExe -UseBasicParsing
}

$params = @("-p", "443", "-R0:127.0.0.1:4433")
if (-not [string]::IsNullOrWhiteSpace($auth)) {
    $params += $auth
}
& $pinggyExe @params
