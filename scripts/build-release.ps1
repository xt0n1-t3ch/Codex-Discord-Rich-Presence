[CmdletBinding()]
param(
    [ValidateSet("all", "x64", "arm64")]
    [string] $Architecture = "all"
)

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

function Import-MsvcEnvironment {
    param([Parameter(Mandatory)] [string] $TargetArchitecture)

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "Visual Studio Installer vswhere.exe is required for Windows release builds."
    }
    $installation = (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1).Trim()
    if ([string]::IsNullOrWhiteSpace($installation)) {
        throw "Visual Studio C++ Build Tools are not installed."
    }
    $toolset = Get-ChildItem -LiteralPath (Join-Path $installation "VC\Tools\MSVC") -Directory |
        Sort-Object Name -Descending |
        Select-Object -First 1
    if ($null -eq $toolset) {
        throw "No MSVC toolset was found under '$installation'."
    }
    $toolDirectory = Join-Path $toolset.FullName "bin\Hostx64\$TargetArchitecture"
    $libraryDirectory = Join-Path $toolset.FullName "lib\$TargetArchitecture"
    foreach ($tool in @("cl.exe", "lib.exe", "link.exe")) {
        if (-not (Test-Path -LiteralPath (Join-Path $toolDirectory $tool) -PathType Leaf)) {
            $component = if ($TargetArchitecture -eq "arm64") { "Microsoft.VisualStudio.Component.VC.Tools.ARM64" } else { "Microsoft.VisualStudio.Component.VC.Tools.x86.x64" }
            throw "MSVC $TargetArchitecture tool '$tool' is missing. Install Visual Studio component '$component'."
        }
    }
    if (-not (Test-Path -LiteralPath $libraryDirectory -PathType Container)) {
        throw "MSVC $TargetArchitecture libraries are missing at '$libraryDirectory'."
    }

    $devCmd = Join-Path $installation "Common7\Tools\VsDevCmd.bat"
    $environmentLines = & $env:ComSpec /d /s /c "`"$devCmd`" -no_logo -arch=$TargetArchitecture -host_arch=x64 && set"
    if ($LASTEXITCODE -ne 0) {
        throw "VsDevCmd failed for $TargetArchitecture with exit code $LASTEXITCODE."
    }
    foreach ($line in $environmentLines) {
        $separator = $line.IndexOf('=')
        if ($separator -le 0) { continue }
        [Environment]::SetEnvironmentVariable(
            $line.Substring(0, $separator),
            $line.Substring($separator + 1),
            [EnvironmentVariableTarget]::Process
        )
    }
}

$targetSpecs = @(
    [pscustomobject]@{
        Architecture = "x64"
        Target = "x86_64-pc-windows-msvc"
        Artifact = "codex-discord-rich-presence-windows-x64.exe"
        Sbom = "codex-discord-rich-presence-windows-x64.spdx.json"
    },
    [pscustomobject]@{
        Architecture = "arm64"
        Target = "aarch64-pc-windows-msvc"
        Artifact = "codex-discord-rich-presence-windows-arm64.exe"
        Sbom = "codex-discord-rich-presence-windows-arm64.spdx.json"
    }
)
if ($Architecture -ne "all") {
    $targetSpecs = @($targetSpecs | Where-Object Architecture -eq $Architecture)
}

$releaseDir = Join-Path $projectRoot "releases\windows"
if (Test-Path -LiteralPath $releaseDir) {
    $existing = @(Get-ChildItem -LiteralPath $releaseDir -Force)
    if ($existing.Count -ne 0) {
        throw "Release directory '$releaseDir' must be empty."
    }
}
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$version = (& $cargoCmd metadata --locked --no-deps --format-version 1 | ConvertFrom-Json).packages |
    Where-Object name -eq "codex-discord-presence" |
    Select-Object -ExpandProperty version -First 1
$payloadNames = [System.Collections.Generic.List[string]]::new()

foreach ($spec in $targetSpecs) {
    Import-MsvcEnvironment -TargetArchitecture $spec.Architecture
    Write-Host "Building release binary for $($spec.Target)..."
    & $cargoCmd build --locked --workspace --release --all-features --target $spec.Target
    if ($LASTEXITCODE -ne 0) {
        throw "locked Cargo release build for $($spec.Target) failed with exit code $LASTEXITCODE"
    }

    $sourceBinary = Join-Path $targetRoot "$($spec.Target)\release\codex-discord-presence.exe"
    if (-not (Test-Path -LiteralPath $sourceBinary -PathType Leaf)) {
        throw "Release binary not found at $sourceBinary"
    }
    $windowsArtifact = Join-Path $releaseDir $spec.Artifact
    Copy-Item -LiteralPath $sourceBinary -Destination $windowsArtifact
    & (Join-Path $scriptRoot "check-windows-pe.ps1") -ArtifactPath $windowsArtifact -Architecture $spec.Architecture | Out-Host
    & (Join-Path $scriptRoot "new-windows-sbom.ps1") -ArtifactPath $windowsArtifact -OutputPath (Join-Path $releaseDir $spec.Sbom) -PackageName codex-discord-presence -PackageVersion $version -Architecture $spec.Architecture
    & (Join-Path $scriptRoot "check-windows-sbom.ps1") -ArtifactPath $windowsArtifact -SbomPath (Join-Path $releaseDir $spec.Sbom) -PackageName codex-discord-presence -PackageVersion $version
    $payloadNames.Add($spec.Artifact)
    $payloadNames.Add($spec.Sbom)
}

$logos = [ordered]@{
    "codex-app-logo.png" = Join-Path $projectRoot "assets\branding\codex-app.png"
    "chatgpt-app-logo.jpg" = Join-Path $projectRoot "assets\branding\chatgpt-app.jpg"
}
foreach ($entry in $logos.GetEnumerator()) {
    if (-not (Test-Path -LiteralPath $entry.Value -PathType Leaf)) {
        throw "Required release payload '$($entry.Value)' is missing."
    }
    Copy-Item -LiteralPath $entry.Value -Destination (Join-Path $releaseDir $entry.Key)
    $payloadNames.Add($entry.Key)
}

$checksumLines = foreach ($name in $payloadNames | Sort-Object) {
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
foreach ($name in $payloadNames) {
    Write-Host " - $(Join-Path $releaseDir $name)"
}
Write-Host " - $checksumPath"
