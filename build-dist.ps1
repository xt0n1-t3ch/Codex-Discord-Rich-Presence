Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

$releaseRoot = Join-Path $projectRoot "releases"
$cargoTargetRoot = Join-Path $releaseRoot ".cargo-target"
$env:CARGO_TARGET_DIR = $cargoTargetRoot

Write-Host "Building release binary (output root: $releaseRoot)..."
cargo build --release

$binaryName = "codex-discord-presence.exe"
$releaseCandidates = @(
    (Join-Path $cargoTargetRoot "release\$binaryName"),
    (Join-Path $cargoTargetRoot "x86_64-pc-windows-msvc\release\$binaryName")
)
$releaseBinary = $releaseCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $releaseBinary) {
    throw "Release binary not found under $cargoTargetRoot"
}

$windowsRoot = Join-Path $releaseRoot "windows"
$windowsArchRoot = Join-Path $windowsRoot "x64"
$executablesDir = Join-Path $windowsArchRoot "executables"
$archivesDir = Join-Path $windowsArchRoot "archives"
New-Item -ItemType Directory -Force -Path $executablesDir | Out-Null
New-Item -ItemType Directory -Force -Path $archivesDir | Out-Null

$rootBinary = Join-Path $windowsRoot $binaryName
$archBinary = Join-Path $executablesDir $binaryName
$archNextBinary = Join-Path $executablesDir "codex-discord-presence.next.exe"

Copy-Item $releaseBinary $rootBinary -Force
try {
    Copy-Item $releaseBinary $archBinary -Force
}
catch {
    Write-Warning "$archBinary is in use; writing $archNextBinary instead."
    Copy-Item $releaseBinary $archNextBinary -Force
}

$archiveName = "codex-discord-presence-windows-x64.zip"
$archivePath = Join-Path $archivesDir $archiveName
Compress-Archive -Path (Join-Path $executablesDir $binaryName) -DestinationPath $archivePath -Force

$shaPath = "$archivePath.sha256"
$hash = (Get-FileHash $archivePath -Algorithm SHA256).Hash.ToLower()
"$hash  $archiveName" | Out-File $shaPath -Encoding ascii

Write-Host "Ready (releases-only layout):"
Write-Host " - $rootBinary"
Write-Host " - $archBinary"
Write-Host " - $archNextBinary (fallback when locked)"
Write-Host " - $archivePath"
Write-Host " - $shaPath"
