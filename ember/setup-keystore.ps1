# Create android/keystore.properties for Gradle signing.
# Run once; you'll be prompted for your keystore password.
# Requires: ember.keystore (create it with .\sign-apk.ps1 first if needed)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$keystore = Join-Path $rootDir "ember.keystore"
$propsFile = Join-Path $rootDir "android\keystore.properties"

if (-not (Test-Path $keystore)) {
    Write-Host "ember.keystore not found. Run .\sign-apk.ps1 first to create it." -ForegroundColor Red
    exit 1
}

$pass = Read-Host "Keystore password" -AsSecureString
$passPlain = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($pass))

$relPath = "../ember.keystore"
$content = @"
storeFile=$relPath
storePassword=$passPlain
keyAlias=ember
keyPassword=$passPlain
"@
Set-Content -Path $propsFile -Value $content -NoNewline
Write-Host "Created $propsFile" -ForegroundColor Green
Write-Host "Rebuild with .\build-android.ps1 to get a signed APK." -ForegroundColor Cyan
