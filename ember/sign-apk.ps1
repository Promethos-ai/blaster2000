# Sign the Ember APK for distribution.
# Requires: Java (keytool, jarsigner) - from Android Studio JBR or JDK
#
# Usage: .\sign-apk.ps1
#        .\sign-apk.ps1 -KeystorePath "path\to\my.keystore" -Alias "myalias"
#
# First run: Creates ember.keystore and prompts for keystore password and alias details.

param(
    [string]$KeystorePath = "",
    [string]$Alias = "ember",
    [string]$ApkPath = "",
    [string]$KeystorePassword = ""  # Or set env KEYSTORE_PASSWORD for non-interactive
)

$ErrorActionPreference = "Stop"
$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Find APK
if (-not $ApkPath) {
    $ApkPath = Join-Path $rootDir "android\app\build\outputs\apk\release\app-release-unsigned.apk"
}
if (-not (Test-Path $ApkPath)) {
    Write-Host "APK not found: $ApkPath" -ForegroundColor Red
    Write-Host "Build first: .\build-android.ps1" -ForegroundColor Yellow
    exit 1
}

# Find Java (keytool, jarsigner)
$javaBin = $null
if ($env:JAVA_HOME) {
    $javaBin = Join-Path $env:JAVA_HOME "bin"
}
if (-not $javaBin -or -not (Test-Path (Join-Path $javaBin "keytool.exe"))) {
    $jbrPaths = @(
        "C:\Program Files\Android\Android Studio\jbr",
        "$env:LOCALAPPDATA\Programs\Android Studio\jbr"
    )
    foreach ($p in $jbrPaths) {
        if (Test-Path (Join-Path $p "bin\keytool.exe")) {
            $env:JAVA_HOME = $p
            $javaBin = Join-Path $p "bin"
            break
        }
    }
}
if (-not $javaBin -or -not (Test-Path (Join-Path $javaBin "keytool.exe"))) {
    Write-Host "Java not found. Set JAVA_HOME to your JDK or Android Studio JBR." -ForegroundColor Red
    exit 1
}

$keytool = Join-Path $javaBin "keytool.exe"
$jarsigner = Join-Path $javaBin "jarsigner.exe"

# Keystore path
if (-not $KeystorePath) {
    $KeystorePath = Join-Path $rootDir "ember.keystore"
}

# Create keystore if it doesn't exist
if (-not (Test-Path $KeystorePath)) {
    Write-Host "Creating new keystore: $KeystorePath" -ForegroundColor Cyan
    Write-Host "You will be prompted for:"
    Write-Host "  - Keystore password (remember this!)"
    Write-Host "  - Name, organization, etc."
    Write-Host ""
    & $keytool -genkey -v -keystore $KeystorePath -alias $Alias -keyalg RSA -keysize 2048 -validity 10000
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

# Find apksigner (required: v1+v2+v3; jarsigner is v1-only and causes "package appears to be invalid")
if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA "Android\Sdk"
}
$buildTools = Join-Path $env:ANDROID_HOME "build-tools"
$apksigner = $null
if (Test-Path $buildTools) {
    $latest = Get-ChildItem $buildTools -Directory | Sort-Object Name -Descending | Select-Object -First 1
    if ($latest) {
        $apksignerBat = Join-Path $latest.FullName "apksigner.bat"
        if (Test-Path $apksignerBat) { $apksigner = $apksignerBat }
    }
}
if (-not $apksigner) {
    Write-Host "apksigner not found. Set ANDROID_HOME to Android SDK (e.g. $env:LOCALAPPDATA\Android\Sdk)." -ForegroundColor Red
    Write-Host "jarsigner produces v1-only APKs that fail with 'package appears to be invalid' on Android 7+." -ForegroundColor Yellow
    exit 1
}

$signedApk = $ApkPath -replace "unsigned\.apk$", "signed.apk"
if ($signedApk -eq $ApkPath) {
    $signedApk = Join-Path (Split-Path $ApkPath) "app-release-signed.apk"
}

$ksPass = $KeystorePassword
if (-not $ksPass) { $ksPass = $env:KEYSTORE_PASSWORD }

Write-Host "Signing APK (v1+v2+v3)..." -ForegroundColor Cyan
Copy-Item $ApkPath $signedApk -Force
if ($ksPass) {
    & $apksigner sign --ks $KeystorePath --ks-key-alias $Alias --ks-pass "pass:$ksPass" --key-pass "pass:$ksPass" --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true $signedApk
} else {
    & $apksigner sign --ks $KeystorePath --ks-key-alias $Alias --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true $signedApk
}
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Host "Verifying signature..." -ForegroundColor Cyan
& $apksigner verify --verbose $signedApk
if ($LASTEXITCODE -ne 0) {
    Write-Host "Signature verification failed." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Signed APK: $signedApk" -ForegroundColor Green
Write-Host "Install on device or upload to release." -ForegroundColor Gray
