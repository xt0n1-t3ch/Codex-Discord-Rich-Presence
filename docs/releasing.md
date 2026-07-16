# Release Procedure

Releases are tag-only, immutable, and bound to a protected `main` commit. The
operator approval step uses the local authenticated GitHub CLI so an
administration token is never stored in Actions. Local implementation does not imply promotion.

## Version surfaces

`scripts/release-contract.json` is the machine-readable owner for the candidate product version, core version, config schema, checksum manifest, and Windows SBOM name. For v1.8.0:

- binary/workspace: 1.8.0;
- `codex-presence-core`: 1.0.0;
- config schema: 13.

The tag version, Cargo metadata, README release copy, changelog section, and release contract must agree before preflight succeeds.

## Required proof

1. Run all five PowerShell release-contract suites.
2. Run locked fmt, clippy, tests, release build, and `cargo audit --deny warnings`.
3. Prove schema 12 to 13 migration and parser fixtures independent of the user profile.
4. On Windows, exercise `status`, `doctor`, TUI persistence, Fast, semantic quota windows, Credits, and real Discord publication.
5. Keep Linux/macOS compile and test gates green.
6. Generate `codex-discord-rich-presence-windows-x64.spdx.json`, validate its binary SHA-256, and include it in `SHA256SUMS.txt`.

## Publish

1. Confirm the intended `main` commit has all required platform checks green.
2. Approve that exact commit:

   ```powershell
   ./scripts/approve-release.ps1 `
     -Repository xt0n1-t3ch/Codex-Discord-Rich-Presence `
     -Sha <40-character-main-sha>
   ```

3. Create and push an annotated tag pointing to the approved commit:

   ```powershell
   git tag -a vX.Y.Z <40-character-main-sha> -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

4. Watch the Release workflow. It rechecks tag ancestry, the approved SHA,
   the latest attempt of every protected check, version ordering, artifacts,
   the Windows SPDX SBOM, and SHA-256 digests before it publishes the draft once.
5. Verify the public release and then clear the one-use approval:

   ```powershell
   gh variable delete RELEASE_APPROVED_SHA `
     --repo xt0n1-t3ch/Codex-Discord-Rich-Presence
   ```

Never move a published tag or replace a published asset. Publish a new patch
version when a correction is required.
