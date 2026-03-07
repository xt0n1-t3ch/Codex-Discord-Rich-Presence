Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptRoot
$targetRoot = Join-Path $projectRoot ".build\target"
Set-Location $projectRoot

$cargoCmd = if (Get-Command cargo -ErrorAction SilentlyContinue) {
    "cargo"
} else {
    throw "cargo executable not found in PATH."
}

Write-Host "Building release binary..."
& $cargoCmd build --release
if ($LASTEXITCODE -ne 0) {
    throw "cargo build --release failed with exit code $LASTEXITCODE"
}

$sourceBinary = Join-Path $targetRoot "release\codex-discord-presence.exe"
if (-not (Test-Path $sourceBinary)) {
    throw "Release binary not found at $sourceBinary"
}

$releaseDir = Join-Path $projectRoot "releases\windows"
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$finalBinary = Join-Path $releaseDir "codex-discord-rich-presence.exe"
Copy-Item $sourceBinary $finalBinary -Force

$iconSource = Join-Path $projectRoot "assets\branding\codex-app.png"
if (Test-Path $iconSource) {
    Copy-Item $iconSource (Join-Path $releaseDir "codex-app.png") -Force
}

Write-Host "Ready:"
Write-Host " - $finalBinary"
