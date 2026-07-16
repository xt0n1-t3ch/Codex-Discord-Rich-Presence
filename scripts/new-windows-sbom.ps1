[CmdletBinding()]
param(
  [Parameter(Mandatory)] [string]$ArtifactPath,
  [Parameter(Mandatory)] [string]$OutputPath,
  [Parameter(Mandatory)] [string]$PackageName,
  [Parameter(Mandatory)] [string]$PackageVersion,
  [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot)
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

$artifact = (Resolve-Path -LiteralPath $ArtifactPath).Path
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$manifest = Join-Path $root "Cargo.toml"
$metadataText = & cargo --locked metadata --format-version 1 --manifest-path $manifest | Out-String
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed: $($metadataText.Trim())" }
$metadata = $metadataText | ConvertFrom-Json
$artifactHash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$rootId = "SPDXRef-Package-$($PackageName -replace '[^A-Za-z0-9.-]', '-')"

$packages = @(
  [ordered]@{
    SPDXID = $rootId
    name = $PackageName
    versionInfo = $PackageVersion
    downloadLocation = "NOASSERTION"
    filesAnalyzed = $true
    licenseConcluded = "NOASSERTION"
    licenseDeclared = "NOASSERTION"
    copyrightText = "NOASSERTION"
    checksums = @([ordered]@{ algorithm = "SHA256"; checksumValue = $artifactHash })
  }
)
$relationships = @(
  [ordered]@{ spdxElementId = "SPDXRef-DOCUMENT"; relationshipType = "DESCRIBES"; relatedSpdxElement = $rootId }
)

$index = 0
foreach ($dependency in @($metadata.packages | Sort-Object name, version, source)) {
  if ([string]$dependency.name -eq $PackageName -and [string]$dependency.version -eq $PackageVersion) { continue }
  $index += 1
  $dependencyId = "SPDXRef-Package-$index-$(([string]$dependency.name) -replace '[^A-Za-z0-9.-]', '-')"
  $license = if ([string]::IsNullOrWhiteSpace([string]$dependency.license)) { "NOASSERTION" } else { [string]$dependency.license }
  $packages += [ordered]@{
    SPDXID = $dependencyId
    name = [string]$dependency.name
    versionInfo = [string]$dependency.version
    downloadLocation = if ([string]::IsNullOrWhiteSpace([string]$dependency.source)) { "NOASSERTION" } else { [string]$dependency.source }
    filesAnalyzed = $false
    licenseConcluded = "NOASSERTION"
    licenseDeclared = $license
    copyrightText = "NOASSERTION"
    externalRefs = @([ordered]@{
      referenceCategory = "PACKAGE-MANAGER"
      referenceType = "purl"
      referenceLocator = "pkg:cargo/$([uri]::EscapeDataString([string]$dependency.name))@$([uri]::EscapeDataString([string]$dependency.version))"
    })
  }
  $relationships += [ordered]@{ spdxElementId = $rootId; relationshipType = "DEPENDS_ON"; relatedSpdxElement = $dependencyId }
}

$document = [ordered]@{
  spdxVersion = "SPDX-2.3"
  dataLicense = "CC0-1.0"
  SPDXID = "SPDXRef-DOCUMENT"
  name = "$PackageName-$PackageVersion-windows-x64"
  documentNamespace = "https://github.com/xt0n1-t3ch/$PackageName/sbom/$PackageVersion/$artifactHash"
  creationInfo = [ordered]@{
    created = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
    creators = @("Tool: Pulse release-contract SPDX generator/1")
    licenseListVersion = "3.26"
  }
  packages = $packages
  files = @([ordered]@{
    SPDXID = "SPDXRef-File-WindowsArtifact"
    fileName = [IO.Path]::GetFileName($artifact)
    checksums = @([ordered]@{ algorithm = "SHA256"; checksumValue = $artifactHash })
    licenseConcluded = "NOASSERTION"
    copyrightText = "NOASSERTION"
  })
  relationships = $relationships + @(
    [ordered]@{ spdxElementId = $rootId; relationshipType = "CONTAINS"; relatedSpdxElement = "SPDXRef-File-WindowsArtifact" }
  )
}

$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
$parent = Split-Path -Parent $resolvedOutput
if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
[IO.File]::WriteAllText($resolvedOutput, (($document | ConvertTo-Json -Depth 10) + "`n"), $utf8NoBom)
Write-Output "SPDX SBOM generated: $resolvedOutput packages=$($packages.Count) sha256=$artifactHash"
