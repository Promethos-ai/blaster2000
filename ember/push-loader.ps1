# Push Ember Loader to repo and add to release.
# Run from ember directory. Close other git processes first if index.lock exists.
#
# 1. Add and commit loader
# 2. Push to repo
# 3. Upload loader APK to v0.1.22 release

$ErrorActionPreference = "Stop"
$root = if (Test-Path "d:\rust\ember") { "d:\rust" } else { (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) }

if (Test-Path "$root\.git\index.lock") {
    Write-Host "Remove index.lock first: Remove-Item '$root\.git\index.lock' -Force" -ForegroundColor Yellow
    exit 1
}

Write-Host "Adding loader..." -ForegroundColor Cyan
Set-Location $root
git add ember/.gitignore ember/README.md ember/android/settings.gradle.kts ember/android/loader/
git commit -m "ember: add Ember Loader 0.0.1 - fetches releases, downloads and installs APK"
git push origin master

$loaderApk = "d:\rust\ember\android\loader\build\outputs\apk\release\loader-release-unsigned.apk"
if (Test-Path $loaderApk) {
    Copy-Item $loaderApk "d:\rust\ember\ember-loader-0.0.1.apk" -Force
    Write-Host "Uploading loader to release..." -ForegroundColor Cyan
    gh release upload v0.1.22 "d:\rust\ember\ember-loader-0.0.1.apk" --repo Promethos-ai/blaster2000
    Write-Host "Done." -ForegroundColor Green
} else {
    Write-Host "Build loader first: cd android; .\gradlew.bat :loader:assembleRelease" -ForegroundColor Yellow
}
