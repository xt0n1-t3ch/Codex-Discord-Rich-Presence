[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$checkScript = Join-Path $repositoryRoot "scripts/check-windows-pe.ps1"
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-release-pe-" + [guid]::NewGuid())

function New-PeFixture {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [int] $Machine
    )
    $bytes = [byte[]]::new(128)
    $bytes[0] = 0x4D
    $bytes[1] = 0x5A
    [BitConverter]::GetBytes([uint32]64).CopyTo($bytes, 0x3C)
    $bytes[64] = 0x50
    $bytes[65] = 0x45
    [BitConverter]::GetBytes([uint16]$Machine).CopyTo($bytes, 68)
    [System.IO.File]::WriteAllBytes($Path, $bytes)
}

function Invoke-PeCheck {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Architecture
    )
    $output = & pwsh -NoProfile -File $checkScript -ArtifactPath $Path -Architecture $Architecture 2>&1 | Out-String
    [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = $output.Trim() }
}

try {
    New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
    $x64 = Join-Path $temporaryRoot "x64.exe"
    $arm64 = Join-Path $temporaryRoot "arm64.exe"
    New-PeFixture -Path $x64 -Machine 0x8664
    New-PeFixture -Path $arm64 -Machine 0xAA64

    $x64Result = Invoke-PeCheck -Path $x64 -Architecture x64
    if ($x64Result.ExitCode -ne 0 -or $x64Result.Output -notmatch '"machine":"0x8664"') {
        throw "x64 PE validation failed: $($x64Result.Output)"
    }
    $arm64Result = Invoke-PeCheck -Path $arm64 -Architecture arm64
    if ($arm64Result.ExitCode -ne 0 -or $arm64Result.Output -notmatch '"machine":"0xAA64"') {
        throw "ARM64 PE validation failed: $($arm64Result.Output)"
    }
    $mismatch = Invoke-PeCheck -Path $x64 -Architecture arm64
    if ($mismatch.ExitCode -eq 0 -or $mismatch.Output -notmatch 'expected 0xAA64') {
        throw "PE architecture mismatch did not fail closed: $($mismatch.Output)"
    }

    Write-Output "Windows PE contract: x64=0x8664 arm64=0xAA64 mismatch rejected"
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
