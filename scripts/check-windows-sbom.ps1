[CmdletBinding()]
param(
  [Parameter(Mandatory)] [string]$ArtifactPath,
  [Parameter(Mandatory)] [string]$SbomPath,
  [Parameter(Mandatory)] [string]$PackageName,
  [Parameter(Mandatory)] [string]$PackageVersion
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$artifact = (Resolve-Path -LiteralPath $ArtifactPath).Path
$sbomFile = (Resolve-Path -LiteralPath $SbomPath).Path
$sbom = Get-Content -Raw -LiteralPath $sbomFile | ConvertFrom-Json
if ($sbom.spdxVersion -ne "SPDX-2.3" -or $sbom.dataLicense -ne "CC0-1.0") {
  throw "SBOM must be an SPDX 2.3 JSON document under CC0-1.0"
}
$package = @($sbom.packages | Where-Object { $_.name -eq $PackageName -and $_.versionInfo -eq $PackageVersion })
if ($package.Count -ne 1) { throw "SBOM must contain exactly one root package $PackageName@$PackageVersion" }
$file = @($sbom.files | Where-Object { $_.fileName -ceq [IO.Path]::GetFileName($artifact) })
if ($file.Count -ne 1) { throw "SBOM does not identify the Windows artifact" }
$expected = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$actual = @($file[0].checksums | Where-Object algorithm -eq "SHA256" | ForEach-Object { ([string]$_.checksumValue).ToLowerInvariant() })
if ($actual.Count -ne 1 -or $actual[0] -ne $expected) { throw "SBOM Windows artifact checksum mismatch" }
if (@($sbom.packages).Count -lt 2) { throw "SBOM dependency inventory is empty" }
Write-Output "SPDX SBOM verified: package=$PackageName version=$PackageVersion sha256=$expected"
