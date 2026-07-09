[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$assetScript = Join-Path $repositoryRoot "scripts/release-assets.ps1"
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-release-assets-" + [guid]::NewGuid())

function Assert-Equal {
    param(
        [Parameter(Mandatory)] $Expected,
        [Parameter(Mandatory)] $Actual,
        [Parameter(Mandatory)] [string] $Message
    )

    if ($Expected -ne $Actual) {
        throw "$Message Expected '$Expected', received '$Actual'."
    }
}

function Assert-True {
    param(
        [Parameter(Mandatory)] [bool] $Condition,
        [Parameter(Mandatory)] [string] $Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Invoke-AssetBuild {
    param(
        [Parameter(Mandatory)] [string] $ArtifactRoot,
        [Parameter(Mandatory)] [string] $OutputDirectory
    )

    $output = & pwsh -NoProfile -File $assetScript -ArtifactRoot $ArtifactRoot -OutputDirectory $OutputDirectory 2>&1 | Out-String
    return [pscustomobject]@{
        ExitCode = $LASTEXITCODE
        Output = $output.Trim()
    }
}

function Add-FixtureFile {
    param(
        [Parameter(Mandatory)] [string] $Root,
        [Parameter(Mandatory)] [string] $RelativePath,
        [Parameter(Mandatory)] [string] $Content
    )

    $path = Join-Path $Root $RelativePath
    New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
    [System.IO.File]::WriteAllText($path, $Content, [System.Text.UTF8Encoding]::new($false))
}

try {
    New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null

    $emptyRoot = Join-Path $temporaryRoot "empty"
    $emptyOutput = Join-Path $temporaryRoot "empty-output"
    New-Item -ItemType Directory -Path $emptyRoot -Force | Out-Null
    $empty = Invoke-AssetBuild -ArtifactRoot $emptyRoot -OutputDirectory $emptyOutput
    Assert-Equal 1 $empty.ExitCode "An empty artifact set must fail."
    Assert-True ($empty.Output -match "No release artifacts") "Empty-artifact failure is unclear."
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $emptyOutput "SHA256SUMS.txt"))) "An empty artifact set produced a checksum manifest."

    $artifactRoot = Join-Path $temporaryRoot "downloaded"
    $outputDirectory = Join-Path $temporaryRoot "release-assets"
    Add-FixtureFile -Root $artifactRoot -RelativePath "windows-x64/releases/windows/codex-discord-rich-presence.exe" -Content "windows-binary"
    Add-FixtureFile -Root $artifactRoot -RelativePath "linux-x64/releases/linux/codex-discord-rich-presence" -Content "linux-binary"
    Add-FixtureFile -Root $artifactRoot -RelativePath "macos-x64/releases/macos/codex-discord-rich-presence-x64" -Content "macos-x64-binary"
    Add-FixtureFile -Root $artifactRoot -RelativePath "macos-arm64/releases/macos/codex-discord-rich-presence-arm64" -Content "macos-arm64-binary"
    Add-FixtureFile -Root $artifactRoot -RelativePath "windows-x64/releases/windows/codex-app.png" -Content "logo"

    $complete = Invoke-AssetBuild -ArtifactRoot $artifactRoot -OutputDirectory $outputDirectory
    Assert-Equal 0 $complete.ExitCode "A complete artifact set must pass."

    $expectedNames = @(
        "Codex Discord Rich Presence - Windows x64.exe"
        "Codex Discord Rich Presence - Linux x64"
        "Codex Discord Rich Presence - macOS x64"
        "Codex Discord Rich Presence - macOS arm64"
        "Codex Discord Rich Presence - Codex App Logo.png"
        "SHA256SUMS.txt"
    )
    $actualNames = @(Get-ChildItem -LiteralPath $outputDirectory -File | Sort-Object Name | ForEach-Object Name)
    Assert-Equal (($expectedNames | Sort-Object) -join "|") ($actualNames -join "|") "Published asset names are incomplete."

    $manifestLines = @(Get-Content -LiteralPath (Join-Path $outputDirectory "SHA256SUMS.txt"))
    Assert-Equal 5 $manifestLines.Count "Checksum manifest must cover each published payload."
    foreach ($name in $expectedNames | Where-Object { $_ -ne "SHA256SUMS.txt" }) {
        $path = Join-Path $outputDirectory $name
        $expectedHash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        Assert-True ($manifestLines -contains "$expectedHash  $name") "Checksum manifest is missing '$name'."
    }

    Write-Output "release asset contract: empty and complete scenarios passed"
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
