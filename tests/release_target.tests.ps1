[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$targetScript = Join-Path $repositoryRoot "scripts/check-release-target.ps1"
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-release-target-" + [guid]::NewGuid())
$requiredChecks = @(
    "Lint, Test, Build (ubuntu-latest)"
    "Lint, Test, Build (windows-latest)"
    "Lint, Test, Build (macos-latest)"
)

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

function Assert-Matches {
    param(
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Actual,
        [Parameter(Mandatory)] [string] $Message
    )

    if ($Actual -notmatch $Pattern) {
        throw "$Message Output: $Actual"
    }
}

function Invoke-Git {
    param(
        [Parameter(Mandatory)] [string] $WorkingDirectory,
        [Parameter(Mandatory)] [string[]] $Arguments
    )

    $output = & git -C $WorkingDirectory @Arguments 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed: $($output.Trim())"
    }
    return $output.Trim()
}

function New-GitFixture {
    $path = Join-Path $temporaryRoot "repository"
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    Invoke-Git -WorkingDirectory $path -Arguments @("init", "--initial-branch=main") | Out-Null
    Invoke-Git -WorkingDirectory $path -Arguments @("config", "user.name", "Release Contract") | Out-Null
    Invoke-Git -WorkingDirectory $path -Arguments @("config", "user.email", "release-contract@example.invalid") | Out-Null
    "main" | Set-Content -LiteralPath (Join-Path $path "state.txt") -Encoding utf8NoBOM
    Invoke-Git -WorkingDirectory $path -Arguments @("add", "state.txt") | Out-Null
    Invoke-Git -WorkingDirectory $path -Arguments @("commit", "-m", "test: seed release target") | Out-Null
    $sha = Invoke-Git -WorkingDirectory $path -Arguments @("rev-parse", "HEAD")
    Invoke-Git -WorkingDirectory $path -Arguments @("tag", "-a", "v1.7.2", "-m", "v1.7.2") | Out-Null
    return [pscustomobject]@{ Path = $path; Sha = $sha }
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] $Value
    )

    $Value | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Path -Encoding utf8NoBOM
}

function New-ApiFixture {
    param(
        [Parameter(Mandatory)] [string] $Name,
        [Parameter(Mandatory)] [string] $Sha,
        [string] $FailedCheck,
        [string] $LatestTag,
        [switch] $IncludeStaleSuccess
    )

    $path = Join-Path $temporaryRoot $Name
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    $checkRuns = foreach ($check in $requiredChecks) {
        $passed = $check -ne $FailedCheck
        [ordered]@{
            id = 200
            name = $check
            status = "completed"
            conclusion = if ($passed) { "success" } else { "failure" }
            head_sha = $Sha
            app = @{ slug = "github-actions" }
        }
    }
    if ($IncludeStaleSuccess -and -not [string]::IsNullOrWhiteSpace($FailedCheck)) {
        $checkRuns += [ordered]@{
            id = 100
            name = $FailedCheck
            status = "completed"
            conclusion = "success"
            head_sha = $Sha
            app = @{ slug = "github-actions" }
        }
    }
    Write-JsonFile -Path (Join-Path $path "check-runs.json") -Value @{ total_count = $checkRuns.Count; check_runs = @($checkRuns) }

    if ([string]::IsNullOrWhiteSpace($LatestTag)) {
        New-Item -ItemType File -Path (Join-Path $path "latest-release.not-found") -Force | Out-Null
    }
    else {
        Write-JsonFile -Path (Join-Path $path "latest-release.json") -Value @{ tag_name = $LatestTag; draft = $false; prerelease = $false }
    }
    return $path
}

function Invoke-TargetCheck {
    param(
        [Parameter(Mandatory)] [string] $GitRoot,
        [Parameter(Mandatory)] [string] $ApiFixture,
        [Parameter(Mandatory)] [string] $Tag,
        [Parameter(Mandatory)] [string] $Version,
        [Parameter(Mandatory)] [string] $Sha,
        [string] $ApprovedSha = $Sha,
        [ValidateSet("true", "false")]
        [string] $IsPrerelease = "false"
    )

    $output = & pwsh -NoProfile -File $targetScript `
        -Repository "xt0n1-t3ch/Codex-Discord-Rich-Presence" `
        -Tag $Tag `
        -Version $Version `
        -Sha $Sha `
        -ApprovedSha $ApprovedSha `
        -IsPrerelease $IsPrerelease `
        -RepositoryRoot $GitRoot `
        -MainRef main `
        -ApiFixtureDirectory $ApiFixture 2>&1 | Out-String
    return [pscustomobject]@{
        ExitCode = $LASTEXITCODE
        Output = $output.Trim()
    }
}

try {
    New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
    $gitFixture = New-GitFixture

    $validApi = New-ApiFixture -Name "api-valid" -Sha $gitFixture.Sha
    $valid = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $validApi -Tag "v1.7.2" -Version "1.7.2" -Sha $gitFixture.Sha
    Assert-Equal 0 $valid.ExitCode "A protected main commit with immutable releases must pass. Output: $($valid.Output)"
    $validResult = $valid.Output | ConvertFrom-Json
    Assert-Equal 3 $validResult.required_checks "The gate must validate all three protected platform contexts."
    Assert-Equal $true $validResult.release_approval "The gate did not confirm exact-SHA operator approval."

    $unapproved = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $validApi -Tag "v1.7.2" -Version "1.7.2" -Sha $gitFixture.Sha -ApprovedSha ("0" * 40)
    Assert-Equal 1 $unapproved.ExitCode "A release SHA without exact operator approval must fail closed."
    Assert-Matches "approved release SHA" $unapproved.Output "Approval mismatch failure is unclear."

    $failedCheckName = "Lint, Test, Build (windows-latest)"
    $failedCheckApi = New-ApiFixture -Name "api-failed-check" -Sha $gitFixture.Sha -FailedCheck $failedCheckName
    $failedCheck = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $failedCheckApi -Tag "v1.7.2" -Version "1.7.2" -Sha $gitFixture.Sha
    Assert-Equal 1 $failedCheck.ExitCode "A failed protected context must block release creation."
    Assert-Matches ([regex]::Escape($failedCheckName)) $failedCheck.Output "Failed-check error does not name the protected context."

    $staleSuccessApi = New-ApiFixture -Name "api-stale-success" -Sha $gitFixture.Sha -FailedCheck $failedCheckName -IncludeStaleSuccess
    $staleSuccess = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $staleSuccessApi -Tag "v1.7.2" -Version "1.7.2" -Sha $gitFixture.Sha
    Assert-Equal 1 $staleSuccess.ExitCode "A stale successful attempt must not hide a newer failed check."
    Assert-Matches ([regex]::Escape($failedCheckName)) $staleSuccess.Output "Latest failed check error is unclear."

    $mismatchedSha = "0" * 40
    $mismatch = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $validApi -Tag "v1.7.2" -Version "1.7.2" -Sha $mismatchedSha
    Assert-Equal 1 $mismatch.ExitCode "A tag/GITHUB_SHA mismatch must fail."
    Assert-Matches "does not match tag" $mismatch.Output "Tag-SHA failure is unclear."

    Invoke-Git -WorkingDirectory $gitFixture.Path -Arguments @("switch", "-c", "release-side") | Out-Null
    "side" | Set-Content -LiteralPath (Join-Path $gitFixture.Path "state.txt") -Encoding utf8NoBOM
    Invoke-Git -WorkingDirectory $gitFixture.Path -Arguments @("add", "state.txt") | Out-Null
    Invoke-Git -WorkingDirectory $gitFixture.Path -Arguments @("commit", "-m", "test: create non-main release commit") | Out-Null
    $sideSha = Invoke-Git -WorkingDirectory $gitFixture.Path -Arguments @("rev-parse", "HEAD")
    Invoke-Git -WorkingDirectory $gitFixture.Path -Arguments @("tag", "-a", "v1.7.3", "-m", "v1.7.3") | Out-Null
    $sideApi = New-ApiFixture -Name "api-side" -Sha $sideSha
    $side = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $sideApi -Tag "v1.7.3" -Version "1.7.3" -Sha $sideSha
    Assert-Equal 1 $side.ExitCode "A tag outside main must fail."
    Assert-Matches "not an ancestor of main" $side.Output "Main-ancestry failure is unclear."

    $newerLatestApi = New-ApiFixture -Name "api-newer-latest" -Sha $gitFixture.Sha -LatestTag "v1.8.0"
    $newerLatest = Invoke-TargetCheck -GitRoot $gitFixture.Path -ApiFixture $newerLatestApi -Tag "v1.7.2" -Version "1.7.2" -Sha $gitFixture.Sha
    Assert-Equal 1 $newerLatest.ExitCode "An older stable release must not replace a newer latest release."
    Assert-Matches "not newer than latest stable" $newerLatest.Output "Latest-release guard failure is unclear."

    Write-Output "release target contract: 9 scenarios passed"
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
