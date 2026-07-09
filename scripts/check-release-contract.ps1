[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $Tag,

    [string] $RepositoryRoot = (Split-Path -Parent $PSScriptRoot),

    [string] $GithubOutputPath = $env:GITHUB_OUTPUT,

    [string] $RefType = $env:GITHUB_REF_TYPE
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-CargoPackageVersion {
    param(
        [Parameter(Mandatory)]
        [string] $Root
    )

    $manifestPath = Join-Path $Root "Cargo.toml"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "Cargo.toml is missing at '$manifestPath'."
    }

    $metadataJson = & cargo metadata --no-deps --format-version 1 --manifest-path $manifestPath 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "cargo metadata failed: $($metadataJson.Trim())"
    }

    $metadata = $metadataJson | ConvertFrom-Json
    $resolvedManifestPath = [System.IO.Path]::GetFullPath($manifestPath)
    $package = $metadata.packages |
        Where-Object { [System.IO.Path]::GetFullPath($_.manifest_path) -eq $resolvedManifestPath } |
        Select-Object -First 1

    if ($null -eq $package) {
        throw "Cargo metadata does not contain the root package from '$manifestPath'."
    }

    return [string] $package.version
}

function Assert-ChangelogSection {
    param(
        [Parameter(Mandatory)]
        [string] $Root,

        [Parameter(Mandatory)]
        [string] $Version
    )

    $changelogPath = Join-Path $Root "CHANGELOG.md"
    if (-not (Test-Path -LiteralPath $changelogPath -PathType Leaf)) {
        throw "CHANGELOG.md is missing at '$changelogPath'."
    }

    $escapedVersion = [regex]::Escape($Version)
    $sectionPattern = "(?ms)^##\s+\[$escapedVersion\](?:\s+-\s+\d{4}-\d{2}-\d{2})?[ \t]*\r?\n(?<body>.*?)(?=^##\s+\[|\z)"
    $section = [regex]::Match((Get-Content -Raw -LiteralPath $changelogPath), $sectionPattern)
    if (-not $section.Success -or [string]::IsNullOrWhiteSpace($section.Groups["body"].Value)) {
        throw "A non-empty CHANGELOG.md section for [$Version] is required."
    }
}

try {
    if (-not [string]::IsNullOrWhiteSpace($RefType) -and $RefType -ne "tag") {
        throw "Release validation requires a tag ref, received '$RefType'."
    }

    $numericIdentifier = "(?:0|[1-9]\d*)"
    $prereleaseIdentifier = "(?:0|[1-9]\d*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    $coreVersion = "$numericIdentifier\.$numericIdentifier\.$numericIdentifier"
    $semverPattern = "^v(?<version>$coreVersion(?<prerelease>-(?:$prereleaseIdentifier)(?:\.(?:$prereleaseIdentifier))*)?(?<build>\+(?:[0-9A-Za-z-]+)(?:\.[0-9A-Za-z-]+)*)?)$"
    $tagMatch = [regex]::Match($Tag, $semverPattern)
    if (-not $tagMatch.Success) {
        throw "Tag '$Tag' is not a valid SemVer release tag (expected vMAJOR.MINOR.PATCH with optional prerelease/build metadata)."
    }

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot)
    $version = $tagMatch.Groups["version"].Value
    $cargoVersion = Get-CargoPackageVersion -Root $root
    if ($version -ne $cargoVersion) {
        throw "Tag version '$version' does not match Cargo package version '$cargoVersion'."
    }

    Assert-ChangelogSection -Root $root -Version $version

    $isPrerelease = $tagMatch.Groups["prerelease"].Success
    $metadata = [ordered]@{
        tag_name = $Tag
        version = $version
        release_name = "Codex Discord Rich Presence v$version"
        is_prerelease = $isPrerelease
        make_latest = -not $isPrerelease
    }

    if (-not [string]::IsNullOrWhiteSpace($GithubOutputPath)) {
        @(
            "tag_name=$($metadata.tag_name)"
            "version=$($metadata.version)"
            "release_name=$($metadata.release_name)"
            "is_prerelease=$($metadata.is_prerelease.ToString().ToLowerInvariant())"
            "make_latest=$($metadata.make_latest.ToString().ToLowerInvariant())"
        ) | Add-Content -LiteralPath $GithubOutputPath -Encoding utf8NoBOM
    }

    $metadata | ConvertTo-Json -Compress
}
catch {
    Write-Error "release contract: $($_.Exception.Message)" -ErrorAction Continue
    exit 1
}
