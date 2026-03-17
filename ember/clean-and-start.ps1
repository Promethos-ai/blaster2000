# Clean everything, rebuild, and start servers.
# Run from d:\rust\ember or project root.

$ErrorActionPreference = "Stop"

# 1. Remove all file locks (kills cargo/rustc/servers, clears .cargo-lock, Gradle)
if (Test-Path "d:\rust\remove-all-locks.ps1") {
    Write-Host "1. Removing file locks..." -ForegroundColor Cyan
    & "d:\rust\remove-all-locks.ps1"
    Start-Sleep -Seconds 3
}

Write-Host "2. Removing target directories..." -ForegroundColor Cyan
Remove-Item -Path "d:\rust\Feb17\target" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "d:\rust\ember\target" -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

Write-Host "3. Building Feb17 grpc_server..." -ForegroundColor Cyan
Push-Location "d:\rust\Feb17"
cargo build --bin grpc_server
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

Write-Host "4. Building ember-server..." -ForegroundColor Cyan
Push-Location "d:\rust\ember"
cargo build -p ember-server
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

Write-Host "5. Starting servers..." -ForegroundColor Cyan
& "d:\rust\ember\start-servers.ps1"

Start-Sleep -Seconds 2
Write-Host "6. Starting pinggy..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd 'd:\rust\ember'; .\pinggy.bat"

Write-Host "`nDone. Servers running in separate windows." -ForegroundColor Green
