[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $Tag,

    [string] $RepositoryRoot = (Split-Path -Parent $PSScriptRoot),

    [string] $GithubOutputPath = $env:GITHUB_OUTPUT,

    [string] $RefType = $env:GITHUB_REF_TYPE,

    [string] $ReleaseNotesPath,

    [string] $RepositorySlug = $env:GITHUB_REPOSITORY,

    [string] $PreviousTag
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

    $metadataJson = & cargo --locked metadata --no-deps --format-version 1 --manifest-path $manifestPath 2>&1 | Out-String
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

function Get-ChangelogSection {
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

    return $section.Groups["body"].Value.Trim()
}

function Assert-ReleaseVersionSurfaces {
    param(
        [Parameter(Mandatory)]
        [string] $Root,

        [Parameter(Mandatory)]
        [string] $Version
    )

    $readmePath = Join-Path $Root "README.md"
    if (-not (Test-Path -LiteralPath $readmePath -PathType Leaf)) {
        throw "README.md is missing at '$readmePath'."
    }
    $readme = Get-Content -Raw -LiteralPath $readmePath
    $escapedVersion = [regex]::Escape($Version)
    if ($readme -notmatch "Release v$escapedVersion") {
        throw "README.md release badge does not match version '$Version'."
    }
    if ($readme -notmatch "What's New in v$escapedVersion") {
        throw "README.md What's New heading does not match version '$Version'."
    }
}

function Write-ReleaseNotes {
    param(
        [Parameter(Mandatory)]
        [string] $Path,

        [Parameter(Mandatory)]
        [string] $Root,

        [Parameter(Mandatory)]
        [string] $Repository,

        [Parameter(Mandatory)]
        [string] $TagName,

        [Parameter(Mandatory)]
        [string] $Version,

        [Parameter(Mandatory)]
        [string] $ChangelogSection,

        [Parameter(Mandatory)]
        [bool] $Prerelease,

        [string] $PriorTag
    )

    if ($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
        throw "Repository slug '$Repository' is invalid."
    }

    $encodedTag = [uri]::EscapeDataString($TagName)
    if ([string]::IsNullOrWhiteSpace($PriorTag)) {
        $fullChangelogUrl = "https://github.com/$Repository/commits/$encodedTag"
    }
    else {
        $encodedPreviousTag = [uri]::EscapeDataString($PriorTag)
        $fullChangelogUrl = "https://github.com/$Repository/compare/$encodedPreviousTag...$encodedTag"
    }

    $releaseSummary = if ($Prerelease) { "Prerelease" } else { "Stable release" }
    $notes = @(
        "# Codex Discord Rich Presence $Version"
        ""
        "$releaseSummary for Codex CLI, Codex VS Code Extension, Codex App, and OpenCode-hosted Codex sessions."
        ""
        "## What Changed"
        ""
        $ChangelogSection
        ""
        "## Release Assets"
        ""
        "- codex-discord-rich-presence-windows-x64.exe"
        "- codex-discord-rich-presence-linux-x64"
        "- codex-discord-rich-presence-macos-x64"
        "- codex-discord-rich-presence-macos-arm64"
        "- codex-app-logo.png"
        "- chatgpt-app-logo.jpg"
        "- SHA256SUMS.txt"
        ""
        "## Integrity"
        ""
        "Verify every downloaded payload against SHA256SUMS.txt."
        ""
        "## Full Changelog"
        ""
        $fullChangelogUrl
        ""
    ) -join "`n"

    $resolvedPath = if ([System.IO.Path]::IsPathRooted($Path)) {
        [System.IO.Path]::GetFullPath($Path)
    }
    else {
        [System.IO.Path]::GetFullPath((Join-Path $Root $Path))
    }
    [System.IO.File]::WriteAllText($resolvedPath, $notes, [System.Text.UTF8Encoding]::new($false))
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
    Assert-ReleaseVersionSurfaces -Root $root -Version $version

    $changelogSection = Get-ChangelogSection -Root $root -Version $version

    $isPrerelease = $tagMatch.Groups["prerelease"].Success
    $metadata = [ordered]@{
        tag_name = $Tag
        version = $version
        release_name = "Codex Discord Rich Presence v$version"
        is_prerelease = $isPrerelease
        make_latest = -not $isPrerelease
    }

    if (-not [string]::IsNullOrWhiteSpace($ReleaseNotesPath)) {
        $priorTag = if ($PSBoundParameters.ContainsKey("PreviousTag")) {
            $PreviousTag
        }
        else {
            $candidate = & git -C $root describe --tags --abbrev=0 "$Tag^" 2>$null | Out-String
            if ($LASTEXITCODE -eq 0) { $candidate.Trim() } else { $null }
        }
        Write-ReleaseNotes `
            -Path $ReleaseNotesPath `
            -Root $root `
            -Repository $RepositorySlug `
            -TagName $Tag `
            -Version $version `
            -ChangelogSection $changelogSection `
            -Prerelease $isPrerelease `
            -PriorTag $priorTag
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
