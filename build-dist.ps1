Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

$releaseRoot = Join-Path $projectRoot "releases"
$cargoTargetRoot = Join-Path $releaseRoot "_build-cache"
$env:CARGO_TARGET_DIR = $cargoTargetRoot

Write-Host "Building release binary (output root: $releaseRoot)..."
$cargoCandidates = @(
    (Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"),
    "cargo"
)
$cargoCmd = $cargoCandidates | Where-Object {
    ($_ -eq "cargo") -or (Test-Path $_)
} | Select-Object -First 1
if (-not $cargoCmd) {
    throw "cargo executable not found. Install Rust toolchain or add cargo to PATH."
}

& $cargoCmd build --release
if ((Get-Variable LASTEXITCODE -ErrorAction SilentlyContinue) -and $LASTEXITCODE -ne 0) {
    throw "cargo build --release failed with exit code $LASTEXITCODE"
}

$binaryName = "codex-discord-presence.exe"
$releaseCandidates = @(
    (Join-Path $cargoTargetRoot "release\$binaryName"),
    (Join-Path $cargoTargetRoot "x86_64-pc-windows-msvc\release\$binaryName"),
    (Join-Path $releaseRoot ".cargo-target\release\$binaryName"),
    (Join-Path $releaseRoot ".cargo-target\x86_64-pc-windows-msvc\release\$binaryName")
)
$releaseBinary = $releaseCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $releaseBinary) {
    throw "Release binary not found under $cargoTargetRoot"
}

$windowsRoot = Join-Path $releaseRoot "windows"
New-Item -ItemType Directory -Force -Path $windowsRoot | Out-Null

$rootBinary = Join-Path $windowsRoot $binaryName
$nextBinary = Join-Path $windowsRoot "codex-discord-presence.next.exe"

try {
    Copy-Item $releaseBinary $rootBinary -Force
    if (Test-Path $nextBinary) {
        Remove-Item $nextBinary -Force -ErrorAction SilentlyContinue
    }
}
catch {
    Write-Warning "$rootBinary is in use; writing $nextBinary instead."
    Copy-Item $releaseBinary $nextBinary -Force
}

$legacyPaths = @(
    (Join-Path $projectRoot "dist"),
    (Join-Path $releaseRoot ".cargo-target"),
    (Join-Path $releaseRoot "windows\x64"),
    (Join-Path $releaseRoot "linux"),
    (Join-Path $releaseRoot "macos"),
    $cargoTargetRoot
)
foreach ($path in $legacyPaths | Select-Object -Unique) {
    if (Test-Path $path) {
        try {
            Remove-Item $path -Recurse -Force
        }
        catch {
            Write-Warning "Could not remove $path (possibly locked)."
        }
    }
}

Write-Host "Ready (simple releases layout):"
Write-Host " - $rootBinary"
Write-Host " - $nextBinary (fallback when locked)"
