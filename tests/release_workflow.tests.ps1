[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$ciPath = Join-Path $repositoryRoot ".github/workflows/ci.yml"
$releasePath = Join-Path $repositoryRoot ".github/workflows/release.yml"
$toolchainPath = Join-Path $repositoryRoot "rust-toolchain.toml"

function Assert-Matches {
    param(
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Actual,
        [Parameter(Mandatory)] [string] $Message
    )

    if ($Actual -notmatch $Pattern) {
        throw $Message
    }
}

function Assert-NotMatches {
    param(
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [string] $Actual,
        [Parameter(Mandatory)] [string] $Message
    )

    if ($Actual -match $Pattern) {
        throw $Message
    }
}

function Get-JobBlock {
    param(
        [Parameter(Mandatory)] [string] $Workflow,
        [Parameter(Mandatory)] [string] $JobName
    )

    $escapedName = [regex]::Escape($JobName)
    $match = [regex]::Match($Workflow, "(?ms)^  ${escapedName}:\r?\n(?<body>.*?)(?=^  [A-Za-z0-9_-]+:\r?$|\z)")
    if (-not $match.Success) {
        throw "Workflow job '$JobName' is missing."
    }

    return $match.Groups["body"].Value
}

function Assert-ImmutableActionPins {
    param(
        [Parameter(Mandatory)] [string] $Workflow,
        [Parameter(Mandatory)] [string] $WorkflowName
    )

    $uses = [regex]::Matches($Workflow, "(?m)^\s*uses:\s*(?<reference>\S+)")
    if ($uses.Count -eq 0) {
        throw "$WorkflowName has no Actions to validate."
    }

    foreach ($use in $uses) {
        $reference = $use.Groups["reference"].Value
        if ($reference.StartsWith("./")) {
            continue
        }
        Assert-Matches "^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_./-]+)?@[0-9a-f]{40}$" $reference "$WorkflowName uses mutable Action reference '$reference'."
    }
}

$ci = Get-Content -Raw -LiteralPath $ciPath
$release = Get-Content -Raw -LiteralPath $releasePath

if (-not (Test-Path -LiteralPath $toolchainPath -PathType Leaf)) {
    throw "rust-toolchain.toml is required."
}
$toolchain = Get-Content -Raw -LiteralPath $toolchainPath
Assert-Matches '(?m)^channel = "1\.96\.1"$' $toolchain "Rust must be pinned to 1.96.1."
Assert-Matches '(?m)^components = \["clippy", "rustfmt"\]$' $toolchain "Rust toolchain must install clippy and rustfmt."
Assert-Matches '(?m)^profile = "minimal"$' $toolchain "Rust toolchain must use the minimal profile."

Assert-ImmutableActionPins -Workflow $ci -WorkflowName "CI"
Assert-ImmutableActionPins -Workflow $release -WorkflowName "Release"

Assert-Matches '(?ms)^permissions:\r?\n  contents: read\s*$' $release "Release workflow must default to contents: read."
Assert-NotMatches '(?m)^\s*workflow_dispatch:' $release "Release workflow must remain tag-only."
Assert-Matches '(?ms)^on:\r?\n  push:\r?\n    tags:\r?\n      - "v\*\.\*\.\*"' $release "Release workflow must trigger only from SemVer-shaped tags."

$preflight = Get-JobBlock -Workflow $release -JobName "preflight"
$build = Get-JobBlock -Workflow $release -JobName "build"
$publish = Get-JobBlock -Workflow $release -JobName "publish"

foreach ($requiredCommand in @(
    'scripts/check-release-contract.ps1'
    'tests/release_contract.tests.ps1'
    'tests/release_assets.tests.ps1'
    'tests/release_workflow.tests.ps1'
    'cargo fmt --check'
    'cargo clippy --workspace --all-targets --all-features -- -D warnings'
    'cargo test --workspace --all-features --verbose'
    'cargo build --workspace --release --all-features'
)) {
    Assert-Matches ([regex]::Escape($requiredCommand)) $preflight "Preflight is missing '$requiredCommand'."
}

Assert-Matches '(?m)^    needs: preflight$' $build "Matrix builds must depend on preflight."
Assert-Matches '(?m)^    needs: \[preflight, build\]$' $publish "Publish must depend on preflight and every matrix build."
Assert-Matches '(?ms)^    permissions:\r?\n      contents: write\s*$' $publish "Only publish may receive contents: write."
Assert-Matches 'scripts/release-assets.ps1' $publish "Publish must use the checked artifact assembler."
Assert-Matches 'fail_on_unmatched_files: true' $publish "Release creation must reject an empty file glob."
Assert-Matches 'overwrite_files: false' $publish "Release creation must not replace existing assets."
Assert-Matches 'SHA256SUMS\.txt' $publish "Publish must prove the checksum manifest is present."
Assert-Matches 'draft: true' $publish "Assets must be attached while the release is a draft."
Assert-Matches 'steps\.draft-release\.outputs\.id' $publish "Draft verification and finalization must use the created release id."
Assert-Matches 'Verify draft release assets' $publish "Draft assets must be verified before publication."
Assert-Matches '\.digest' $publish "Draft verification must compare GitHub asset digests with local SHA-256 values."
Assert-Matches 'Finalize release once' $publish "The workflow must have one explicit finalization step."
Assert-Matches '\{draft: false, prerelease: \$prerelease, make_latest: \$make_latest\}' $publish "Finalization must publish the verified draft exactly once."
Assert-Matches "\.immutable.*=.*true" $publish "Finalization must confirm GitHub made the release immutable."
Assert-NotMatches 'always\s*\(' $publish "Publish must not bypass a failed dependency."

foreach ($workflow in @($ci, $release)) {
    Assert-Matches 'toolchain: 1\.96\.1' $workflow "Every Rust workflow must install toolchain 1.96.1."
    Assert-Matches 'persist-credentials: false' $workflow "Checkout credentials must not persist."
}

Write-Output "release workflow contract: pins, permissions, DAG, toolchain, and gates passed"
