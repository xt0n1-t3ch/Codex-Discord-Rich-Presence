#requires -Version 5.1
<#
.SYNOPSIS
  Local verification for Codex Discord Rich Presence.

.DESCRIPTION
  Runs the format, lint, and test checks that previously ran in GitHub Actions,
  locally, so Actions minutes are reserved for essentials. Run this before
  pushing. Fails fast on the first failing step.

  Requires: a Rust toolchain with rustfmt + clippy.
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

function Invoke-Step {
  param([string]$Name, [scriptblock]$Command)
  Write-Host "== $Name ==" -ForegroundColor Cyan
  & $Command
  if ($LASTEXITCODE -ne 0) { throw "$Name failed (exit $LASTEXITCODE)" }
}

Push-Location $root
try {
  Invoke-Step "cargo fmt --check" { cargo fmt --all -- --check }
  Invoke-Step "cargo clippy -D warnings" { cargo clippy --workspace --all-targets --all-features -- -D warnings }
  Invoke-Step "cargo test" { cargo test --workspace --all-features }
  Write-Host "All local checks passed." -ForegroundColor Green
}
finally {
  Pop-Location
}
