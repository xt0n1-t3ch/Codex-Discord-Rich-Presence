# Release Procedure

Releases are tag-only, immutable, and bound to a protected `main` commit. The
operator approval step uses the local authenticated GitHub CLI so an
administration token is never stored in Actions.

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
   and SHA-256 digests before it publishes the draft once.
5. Verify the public release and then clear the one-use approval:

   ```powershell
   gh variable delete RELEASE_APPROVED_SHA `
     --repo xt0n1-t3ch/Codex-Discord-Rich-Presence
   ```

Never move a published tag or replace a published asset. Publish a new patch
version when a correction is required.
