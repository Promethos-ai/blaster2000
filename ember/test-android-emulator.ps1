# Complete Android emulator test for Ember app.
# Prerequisites: grpc_server and ember-server running, emulator connected.
# Usage: .\test-android-emulator.ps1

$ErrorActionPreference = "Stop"
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
$device = "emulator-5554"

# Coordinates from UI hierarchy (1080x2400 screen)
$serverFieldY = 240
$promptFieldY = 1800
$askBtnY = 2232
$centerX = 540

Write-Host "=== Ember Android Emulator Test ===" -ForegroundColor Cyan
Write-Host ""

# Check servers
$grpc = netstat -ano | Select-String ":50051.*LISTENING"
$ember = netstat -ano -p UDP | Select-String "8443"
if (-not $grpc) { Write-Host "WARNING: grpc_server not listening on 50051" -ForegroundColor Yellow }
if (-not $ember) { Write-Host "WARNING: ember-server not listening on UDP 8443" -ForegroundColor Yellow }
if ($grpc -and $ember) { Write-Host "[OK] Both servers running" -ForegroundColor Green }

# Check device
$devList = & $adb devices 2>&1 | Out-String
if ($devList -notmatch "emulator-\d+\s+device") { throw "No emulator connected. Run: emulator -avd <name>" }
Write-Host "[OK] Emulator connected" -ForegroundColor Green
Write-Host ""

# Clear logcat
& $adb -s $device logcat -c 2>$null
Write-Host "1. Launching app..." -ForegroundColor Cyan
& $adb -s $device shell am start -n com.ember.android/.SplashActivity
Start-Sleep -Seconds 3

Write-Host "2. Setting server to 10.0.2.2:8443..." -ForegroundColor Cyan
& $adb -s $device shell input tap $centerX $serverFieldY
Start-Sleep -Milliseconds 600
# Move to start, then delete forward to clear (more reliable than backspace)
& $adb -s $device shell input keyevent 122  # KEYCODE_MOVE_HOME
1..100 | ForEach-Object { & $adb -s $device shell input keyevent 112 }  # KEYCODE_FORWARD_DEL
& $adb -s $device shell input text "10.0.2.2:8443"
Start-Sleep -Milliseconds 400

Write-Host "3. Tapping prompt field and typing question..." -ForegroundColor Cyan
& $adb -s $device shell input tap $centerX $promptFieldY
Start-Sleep -Milliseconds 500
& $adb -s $device shell input text "What%sis%stwo%splus%stwo"
Start-Sleep -Milliseconds 400

Write-Host "4. Tapping Ask..." -ForegroundColor Cyan
& $adb -s $device shell input tap $centerX $askBtnY

Write-Host "5. Waiting 30s for response..." -ForegroundColor Cyan
Start-Sleep -Seconds 30

Write-Host ""
Write-Host "=== Checking result ===" -ForegroundColor Cyan
& $adb -s $device shell uiautomator dump 2>$null | Out-Null
$dump = & $adb -s $device shell cat /sdcard/window_dump.xml 2>$null
if ($dump -match 'resource-id="com\.ember\.android:id/server_address"[^>]*text="([^"]*)"') {
    Write-Host "Server field: $($Matches[1])" -ForegroundColor Gray
}
if ($dump -match 'resource-id="com\.ember\.android:id/prompt_input"[^>]*text="([^"]*)"') {
    Write-Host "Prompt field: $($Matches[1])" -ForegroundColor Gray
}

# Check for errors in logcat
$logs = & $adb -s $device logcat -d 2>&1
$errors = $logs | Select-String -Pattern "MainActivity.*Error|askStreaming failed|Could not resolve|FATAL"
if ($errors) {
    Write-Host ""
    Write-Host "Errors in log:" -ForegroundColor Red
    $errors | ForEach-Object { Write-Host $_.Line -ForegroundColor Red }
} else {
    Write-Host "[OK] No errors in log" -ForegroundColor Green
}

Write-Host ""
Write-Host "Test complete. Check the emulator screen for the AI response." -ForegroundColor Yellow
