# Clean everything, rebuild, and start servers.
# Run from d:\rust\ember or project root.

$ErrorActionPreference = "Stop"

Write-Host "1. Stopping processes..." -ForegroundColor Cyan
Get-Process -Name "cargo","rustc","ember-server","grpc_server","pinggy","pinggy-win" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3

Write-Host "2. Removing cargo locks..." -ForegroundColor Cyan
Get-ChildItem -Path "d:\rust" -Directory -Filter "target" -Recurse -Depth 5 -ErrorAction SilentlyContinue | ForEach-Object {
    Get-ChildItem -Path $_.FullName -Filter ".cargo-lock" -Recurse -ErrorAction SilentlyContinue -Force | Remove-Item -Force
}

Write-Host "3. Removing target directories..." -ForegroundColor Cyan
Remove-Item -Path "d:\rust\Feb17\target" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "d:\rust\ember\target" -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

Write-Host "4. Building Feb17 grpc_server..." -ForegroundColor Cyan
Push-Location "d:\rust\Feb17"
cargo build --bin grpc_server
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

Write-Host "5. Building ember-server..." -ForegroundColor Cyan
Push-Location "d:\rust\ember"
cargo build -p ember-server
if ($LASTEXITCODE -ne 0) { exit 1 }
Pop-Location

Write-Host "6. Starting servers..." -ForegroundColor Cyan
& "d:\rust\ember\start-servers.ps1"

Start-Sleep -Seconds 2
Write-Host "7. Starting pinggy..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd 'd:\rust\ember'; .\pinggy.bat"

Write-Host "`nDone. Servers running in separate windows." -ForegroundColor Green
