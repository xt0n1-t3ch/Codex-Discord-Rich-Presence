[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $ArtifactPath,

    [Parameter(Mandatory)]
    [ValidateSet("x64", "arm64")]
    [string] $Architecture
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$expectedMachine = if ($Architecture -eq "arm64") { 0xAA64 } else { 0x8664 }
$resolvedArtifact = (Resolve-Path -LiteralPath $ArtifactPath).Path
$stream = [System.IO.File]::OpenRead($resolvedArtifact)
try {
    if ($stream.Length -lt 64) {
        throw "Windows artifact '$resolvedArtifact' is too small to be a PE image."
    }
    $reader = [System.IO.BinaryReader]::new($stream)
    if ($reader.ReadUInt16() -ne 0x5A4D) {
        throw "Windows artifact '$resolvedArtifact' is missing the MZ signature."
    }
    $stream.Position = 0x3C
    $peOffset = $reader.ReadUInt32()
    if ($peOffset -gt ($stream.Length - 6)) {
        throw "Windows artifact '$resolvedArtifact' has an invalid PE header offset."
    }
    $stream.Position = $peOffset
    if ($reader.ReadUInt32() -ne 0x00004550) {
        throw "Windows artifact '$resolvedArtifact' is missing the PE signature."
    }
    $actualMachine = $reader.ReadUInt16()
    if ($actualMachine -ne $expectedMachine) {
        throw ("Windows artifact '{0}' machine is 0x{1:X4}; expected 0x{2:X4} for {3}." -f $resolvedArtifact, $actualMachine, $expectedMachine, $Architecture)
    }
}
finally {
    $stream.Dispose()
}

[ordered]@{
    artifact = $resolvedArtifact
    architecture = $Architecture
    machine = ("0x{0:X4}" -f $expectedMachine)
} | ConvertTo-Json -Compress
