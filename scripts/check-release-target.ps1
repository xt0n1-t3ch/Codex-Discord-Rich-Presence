[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $Repository,

    [Parameter(Mandatory)]
    [string] $Tag,

    [Parameter(Mandatory)]
    [string] $Version,

    [Parameter(Mandatory)]
    [string] $Sha,

    [Parameter(Mandatory)]
    [ValidateSet("true", "false")]
    [string] $IsPrerelease,

    [string] $RepositoryRoot = (Split-Path -Parent $PSScriptRoot),

    [string] $MainRef = "origin/main",

    [string] $ApiFixtureDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$requiredChecks = @(
    "Lint, Test, Build (ubuntu-latest)"
    "Lint, Test, Build (windows-latest)"
    "Lint, Test, Build (macos-latest)"
)

function Invoke-GitCommand {
    param(
        [Parameter(Mandatory)]
        [string] $Root,

        [Parameter(Mandatory)]
        [string[]] $Arguments
    )

    $output = & git -C $Root @Arguments 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed: $($output.Trim())"
    }
    return $output.Trim()
}

function Get-GithubApiResponse {
    param(
        [Parameter(Mandatory)]
        [ValidateSet("immutable", "checks", "latest")]
        [string] $Kind,

        [Parameter(Mandatory)]
        [string] $Endpoint,

        [switch] $AllowNotFound
    )

    if (-not [string]::IsNullOrWhiteSpace($ApiFixtureDirectory)) {
        $fixtureName = switch ($Kind) {
            "immutable" { "immutable-releases.json" }
            "checks" { "check-runs.json" }
            "latest" { "latest-release.json" }
        }
        $fixturePath = Join-Path $ApiFixtureDirectory $fixtureName
        if ($Kind -eq "latest" -and -not (Test-Path -LiteralPath $fixturePath -PathType Leaf)) {
            $notFoundPath = Join-Path $ApiFixtureDirectory "latest-release.not-found"
            if ($AllowNotFound -and (Test-Path -LiteralPath $notFoundPath -PathType Leaf)) {
                return $null
            }
        }
        if (-not (Test-Path -LiteralPath $fixturePath -PathType Leaf)) {
            throw "GitHub API fixture '$fixturePath' is missing."
        }
        return Get-Content -Raw -LiteralPath $fixturePath | ConvertFrom-Json
    }

    $response = & gh api `
        -H "Accept: application/vnd.github+json" `
        -H "X-GitHub-Api-Version: 2026-03-10" `
        $Endpoint 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        if ($AllowNotFound -and $response -match 'HTTP 404') {
            return $null
        }
        throw "GitHub API request '$Endpoint' failed: $($response.Trim())"
    }
    return $response | ConvertFrom-Json
}

try {
    if ($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
        throw "Repository slug '$Repository' is invalid."
    }
    if ($Sha -cnotmatch '^[0-9a-f]{40}$') {
        throw "Release SHA '$Sha' must be a lowercase 40-character commit SHA."
    }
    if ($Tag -cne "v$Version") {
        throw "Tag '$Tag' does not match validated version '$Version'."
    }
    try {
        $validatedVersion = [System.Management.Automation.SemanticVersion]::new($Version)
    }
    catch {
        throw "Validated version '$Version' is not valid SemVer."
    }
    $versionIsPrerelease = -not [string]::IsNullOrWhiteSpace($validatedVersion.PreReleaseLabel)
    if (($IsPrerelease -eq "true") -ne $versionIsPrerelease) {
        throw "Prerelease flag '$IsPrerelease' does not match version '$Version'."
    }

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot)
    if ($MainRef -match '^(?<remote>[A-Za-z0-9_.-]+)/(?<branch>[A-Za-z0-9_./-]+)$') {
        $remote = $Matches["remote"]
        $branch = $Matches["branch"]
        Invoke-GitCommand -Root $root -Arguments @(
            "fetch",
            "--no-tags",
            $remote,
            "+refs/heads/${branch}:refs/remotes/${remote}/${branch}"
        ) | Out-Null
    }

    $tagCommit = Invoke-GitCommand -Root $root -Arguments @("rev-parse", "$Tag^{commit}")
    if ($tagCommit -cne $Sha) {
        throw "Release SHA '$Sha' does not match tag '$Tag' commit '$tagCommit'."
    }
    Invoke-GitCommand -Root $root -Arguments @("rev-parse", "$MainRef^{commit}") | Out-Null
    & git -C $root merge-base --is-ancestor $Sha $MainRef 2>$null
    $ancestorExitCode = $LASTEXITCODE
    if ($ancestorExitCode -eq 1) {
        throw "Tag '$Tag' commit '$Sha' is not an ancestor of $MainRef."
    }
    if ($ancestorExitCode -ne 0) {
        throw "Unable to verify '$Sha' against $MainRef (git exit $ancestorExitCode)."
    }

    $immutableState = Get-GithubApiResponse `
        -Kind immutable `
        -Endpoint "repos/$Repository/immutable-releases"
    if ($immutableState.PSObject.Properties.Name -notcontains "enabled" -or $immutableState.enabled -ne $true) {
        if ($immutableState.PSObject.Properties.Name -notcontains "enabled") {
            throw "Immutable release response must contain enabled: true."
        }
        throw "GitHub immutable releases are not enabled for '$Repository'."
    }

    $checkResponse = Get-GithubApiResponse `
        -Kind checks `
        -Endpoint "repos/$Repository/commits/$Sha/check-runs?per_page=100&filter=all"
    if ($checkResponse.PSObject.Properties.Name -notcontains "check_runs") {
        throw "GitHub check-runs response is missing check_runs."
    }
    $checkRuns = @($checkResponse.check_runs)
    foreach ($requiredCheck in $requiredChecks) {
        $successfulRuns = @($checkRuns | Where-Object {
                $_.name -ceq $requiredCheck -and
                $_.head_sha -ceq $Sha -and
                $_.status -ceq "completed" -and
                $_.conclusion -ceq "success" -and
                $_.app.slug -ceq "github-actions"
            })
        if ($successfulRuns.Count -eq 0) {
            throw "Protected check '$requiredCheck' is missing or unsuccessful for '$Sha'."
        }
    }

    $latestGuard = if ($IsPrerelease -eq "true") {
        "prerelease-not-latest"
    }
    else {
        $latestRelease = Get-GithubApiResponse `
            -Kind latest `
            -Endpoint "repos/$Repository/releases/latest" `
            -AllowNotFound
        if ($null -eq $latestRelease) {
            "first-stable-release"
        }
        else {
            if ($latestRelease.PSObject.Properties.Name -notcontains "tag_name" -or $latestRelease.tag_name -notmatch '^v(?<version>.+)$') {
                throw "Latest release response has an invalid tag_name."
            }
            try {
                $currentVersion = $validatedVersion
                $latestVersion = [System.Management.Automation.SemanticVersion]::new($Matches["version"])
            }
            catch {
                throw "Latest stable release tag '$($latestRelease.tag_name)' is not valid SemVer."
            }
            if ($currentVersion -le $latestVersion) {
                throw "Release '$Tag' is not newer than latest stable '$($latestRelease.tag_name)'."
            }
            "newer-than-$($latestRelease.tag_name)"
        }
    }

    [ordered]@{
        repository = $Repository
        tag = $Tag
        sha = $Sha
        main_ref = $MainRef
        immutable_releases = $true
        required_checks = $requiredChecks.Count
        latest_guard = $latestGuard
    } | ConvertTo-Json -Compress
}
catch {
    Write-Error "release target: $($_.Exception.Message)" -ErrorAction Continue
    exit 1
}
