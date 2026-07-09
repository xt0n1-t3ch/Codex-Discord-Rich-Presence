[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$approvalScript = Join-Path $repositoryRoot "scripts/approve-release.ps1"
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-release-approval-" + [guid]::NewGuid())

function Invoke-Approval {
    param(
        [Parameter(Mandatory)] [string] $Fixture,
        [Parameter(Mandatory)] [string] $Sha
    )

    $output = & pwsh -NoProfile -File $approvalScript `
        -Repository "xt0n1-t3ch/Codex-Discord-Rich-Presence" `
        -Sha $Sha `
        -ApiFixturePath $Fixture `
        -SkipVariableWrite 2>&1 | Out-String
    [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = $output.Trim() }
}

try {
    New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
    $enabled = Join-Path $temporaryRoot "enabled.json"
    '{"enabled":true}' | Set-Content -LiteralPath $enabled -Encoding utf8NoBOM
    $sha = "a" * 40
    $approved = Invoke-Approval -Fixture $enabled -Sha $sha
    if ($approved.ExitCode -ne 0) {
        throw "Enabled immutable releases should approve the SHA. Output: $($approved.Output)"
    }
    $result = $approved.Output | ConvertFrom-Json
    if ($result.approved_sha -cne $sha -or $result.immutable_releases -ne $true) {
        throw "Approval output does not preserve the exact SHA and immutable state."
    }

    $disabled = Join-Path $temporaryRoot "disabled.json"
    '{"enabled":false}' | Set-Content -LiteralPath $disabled -Encoding utf8NoBOM
    $rejected = Invoke-Approval -Fixture $disabled -Sha $sha
    if ($rejected.ExitCode -ne 1 -or $rejected.Output -notmatch "not enabled") {
        throw "Disabled immutable releases must fail closed. Output: $($rejected.Output)"
    }

    Write-Output "release approval contract: enabled and disabled states passed"
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
