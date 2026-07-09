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
& $cargoCmd build --locked --workspace --release --all-features
if ($LASTEXITCODE -ne 0) {
    throw "locked Cargo release build failed with exit code $LASTEXITCODE"
}

$sourceBinary = Join-Path $targetRoot "release\codex-discord-presence.exe"
if (-not (Test-Path $sourceBinary)) {
    throw "Release binary not found at $sourceBinary"
}

$releaseDir = Join-Path $projectRoot "releases\windows"
if (Test-Path -LiteralPath $releaseDir) {
    $existing = @(Get-ChildItem -LiteralPath $releaseDir -Force)
    if ($existing.Count -ne 0) {
        throw "Release directory '$releaseDir' must be empty."
    }
}
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$payloads = [ordered]@{
    "codex-discord-rich-presence-windows-x64.exe" = $sourceBinary
    "codex-app-logo.png" = Join-Path $projectRoot "assets\branding\codex-app.png"
    "chatgpt-app-logo.jpg" = Join-Path $projectRoot "assets\branding\chatgpt-app.jpg"
}
foreach ($entry in $payloads.GetEnumerator()) {
    if (-not (Test-Path -LiteralPath $entry.Value -PathType Leaf)) {
        throw "Required release payload '$($entry.Value)' is missing."
    }
    Copy-Item -LiteralPath $entry.Value -Destination (Join-Path $releaseDir $entry.Key)
}

$checksumLines = foreach ($name in $payloads.Keys | Sort-Object) {
    $path = Join-Path $releaseDir $name
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name"
}
$checksumPath = Join-Path $releaseDir "SHA256SUMS.txt"
[System.IO.File]::WriteAllText(
    $checksumPath,
    (($checksumLines -join "`n") + "`n"),
    [System.Text.UTF8Encoding]::new($false)
)

Write-Host "Ready:"
foreach ($name in $payloads.Keys) {
    Write-Host " - $(Join-Path $releaseDir $name)"
}
Write-Host " - $checksumPath"
