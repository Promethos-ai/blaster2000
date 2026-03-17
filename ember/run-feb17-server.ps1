# Run Feb17 http_server from ember workspace. Removes all locks first.
$ErrorActionPreference = "Stop"
$feb17 = "d:\rust\Feb17"

if (Test-Path "d:\rust\remove-all-locks.ps1") {
    & "d:\rust\remove-all-locks.ps1"
}

Set-Location $feb17
cargo run --bin http_server --features "cuda,integrations" -- @args
