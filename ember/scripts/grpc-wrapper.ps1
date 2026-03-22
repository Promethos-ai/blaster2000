# Wrapper for grpc_server: on crash, write to log and exit so NSSM restarts.
# Used by install-grpc-service.ps1 when installing EmberGrpcServer as a Windows service.

$ErrorActionPreference = "Stop"
$feb17Dir = "d:\rust\Feb17"
$logFile = "$feb17Dir\grpc_server.log"
$grpcExe = if (Test-Path "$feb17Dir\target\release\grpc_server.exe") {
    "$feb17Dir\target\release\grpc_server.exe"
} elseif (Test-Path "$feb17Dir\target\debug\grpc_server.exe") {
    "$feb17Dir\target\debug\grpc_server.exe"
} else {
    "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] CRASH grpc_server.exe not found" | Add-Content $logFile -Encoding UTF8
    exit 1
}

Set-Location $feb17Dir
& $grpcExe --port 50051 --log-file $logFile
$exitCode = $LASTEXITCODE

if ($exitCode -ne 0) {
    $msg = "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] CRASH exit code $exitCode, NSSM will restart in 5s"
    $msg | Add-Content $logFile -Encoding UTF8
}

exit $exitCode
