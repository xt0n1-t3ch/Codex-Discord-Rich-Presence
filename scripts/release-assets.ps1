[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $ArtifactRoot,

    [Parameter(Mandatory)]
    [string] $OutputDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Find-RequiredArtifact {
    param(
        [Parameter(Mandatory)]
        [System.IO.FileInfo[]] $Files,

        [Parameter(Mandatory)]
        [string] $PathSuffix
    )

    $normalizedSuffix = "/" + $PathSuffix.Replace("\", "/").TrimStart("/")
    $matches = @($Files | Where-Object { $_.FullName.Replace("\", "/").EndsWith($normalizedSuffix, [System.StringComparison]::Ordinal) })
    if ($matches.Count -ne 1) {
        throw "Expected exactly one artifact ending in '$PathSuffix', found $($matches.Count)."
    }

    return $matches[0]
}

try {
    $resolvedArtifactRoot = [System.IO.Path]::GetFullPath($ArtifactRoot)
    if (-not (Test-Path -LiteralPath $resolvedArtifactRoot -PathType Container)) {
        throw "Artifact root '$resolvedArtifactRoot' does not exist."
    }

    $files = @(Get-ChildItem -LiteralPath $resolvedArtifactRoot -Recurse -File | Sort-Object FullName)
    if ($files.Count -eq 0) {
        throw "No release artifacts were downloaded."
    }

    $sources = [ordered]@{
        "codex-discord-rich-presence-windows-x64.exe" = Find-RequiredArtifact -Files $files -PathSuffix "windows/codex-discord-rich-presence-windows-x64.exe"
        "codex-discord-rich-presence-linux-x64" = Find-RequiredArtifact -Files $files -PathSuffix "linux/codex-discord-rich-presence-linux-x64"
        "codex-discord-rich-presence-macos-x64" = Find-RequiredArtifact -Files $files -PathSuffix "macos/codex-discord-rich-presence-macos-x64"
        "codex-discord-rich-presence-macos-arm64" = Find-RequiredArtifact -Files $files -PathSuffix "macos/codex-discord-rich-presence-macos-arm64"
    }

    $logos = @($files | Where-Object { $_.Name -eq "codex-app-logo.png" })
    if ($logos.Count -eq 0) {
        throw "Expected at least one codex-app-logo.png artifact, found none."
    }
    $sources["codex-app-logo.png"] = $logos[0]

    $resolvedOutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
    if (Test-Path -LiteralPath $resolvedOutputDirectory) {
        $existingFiles = @(Get-ChildItem -LiteralPath $resolvedOutputDirectory -Force)
        if ($existingFiles.Count -ne 0) {
            throw "Output directory '$resolvedOutputDirectory' must be empty."
        }
    }
    else {
        New-Item -ItemType Directory -Path $resolvedOutputDirectory -Force | Out-Null
    }

    foreach ($entry in $sources.GetEnumerator()) {
        Copy-Item -LiteralPath $entry.Value.FullName -Destination (Join-Path $resolvedOutputDirectory $entry.Key)
    }

    $checksumLines = foreach ($name in $sources.Keys | Sort-Object) {
        $path = Join-Path $resolvedOutputDirectory $name
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $name"
    }
    $checksumPath = Join-Path $resolvedOutputDirectory "SHA256SUMS.txt"
    [System.IO.File]::WriteAllText(
        $checksumPath,
        (($checksumLines -join "`n") + "`n"),
        [System.Text.UTF8Encoding]::new($false)
    )

    [ordered]@{
        payload_count = $sources.Count
        checksum_manifest = $checksumPath
    } | ConvertTo-Json -Compress
}
catch {
    Write-Error "release assets: $($_.Exception.Message)" -ErrorAction Continue
    exit 1
}
