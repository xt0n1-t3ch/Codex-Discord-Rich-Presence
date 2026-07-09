[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $Repository,

    [Parameter(Mandatory)]
    [string] $Sha,

    [string] $VariableName = "RELEASE_APPROVED_SHA",

    [string] $ApiFixturePath,

    [switch] $SkipVariableWrite
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

try {
    if ($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
        throw "Repository slug '$Repository' is invalid."
    }
    if ($Sha -cnotmatch '^[0-9a-f]{40}$') {
        throw "Release SHA '$Sha' must be a lowercase 40-character commit SHA."
    }
    if ($VariableName -cnotmatch '^[A-Z][A-Z0-9_]*$') {
        throw "Repository variable '$VariableName' is invalid."
    }

    $immutableState = if ([string]::IsNullOrWhiteSpace($ApiFixturePath)) {
        $response = & gh api `
            -H "Accept: application/vnd.github+json" `
            -H "X-GitHub-Api-Version: 2026-03-10" `
            "repos/$Repository/immutable-releases" 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "Unable to verify immutable releases: $($response.Trim())"
        }
        $response | ConvertFrom-Json
    }
    else {
        if (-not (Test-Path -LiteralPath $ApiFixturePath -PathType Leaf)) {
            throw "Immutable release fixture '$ApiFixturePath' is missing."
        }
        Get-Content -Raw -LiteralPath $ApiFixturePath | ConvertFrom-Json
    }

    if ($immutableState.PSObject.Properties.Name -notcontains "enabled" -or $immutableState.enabled -ne $true) {
        throw "GitHub immutable releases are not enabled for '$Repository'."
    }

    if (-not $SkipVariableWrite) {
        & gh variable set $VariableName --repo $Repository --body $Sha
        if ($LASTEXITCODE -ne 0) {
            throw "Unable to set repository variable '$VariableName'."
        }
        $storedSha = (& gh variable get $VariableName --repo $Repository 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or $storedSha -cne $Sha) {
            throw "Repository variable '$VariableName' did not preserve the approved SHA."
        }
    }

    [ordered]@{
        repository = $Repository
        approved_sha = $Sha
        immutable_releases = $true
        variable = $VariableName
        variable_written = -not $SkipVariableWrite
    } | ConvertTo-Json -Compress
}
catch {
    Write-Error "release approval: $($_.Exception.Message)" -ErrorAction Continue
    exit 1
}
