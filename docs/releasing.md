# Release procedure

Releases are immutable and bound to a protected `main` commit. Quality checks run locally, not in GitHub Actions: there is no per-push or per-PR CI, and the Release workflow is manual-only (`workflow_dispatch`), so pushing a tag does not trigger it. Run the local gate with `cargo xtask check` and build locally, or dispatch the workflow by hand. The operator approval step uses the local authenticated GitHub CLI, so an administration token is never stored in Actions. Local implementation does not imply promotion.

Every check and release step is a Rust command in the `xtask` crate. PowerShell is not needed on any platform.

## Commands

| Task | Command |
| --- | --- |
| Full local gate (format, clippy, nextest, changelog, llms) | `cargo xtask check` |
| Release-script tests | `cargo nextest run -p xtask` |
| Validate the changelog format | `cargo xtask changelog check` |
| Print one changelog section | `cargo xtask changelog notes <version>` |
| Regenerate or verify `llms.txt` and `llms-full.txt` | `cargo xtask llms` / `cargo xtask llms --check` |
| Compare or apply the GitHub description and topics | `cargo xtask meta --check` / `cargo xtask meta --apply` |
| Validate tag, Cargo version, README badge, contract and changelog | `cargo xtask release-contract --tag vX.Y.Z` |
| Print the release notes (body), or the title | `cargo xtask release-notes <version>` / `cargo xtask release-notes <version> --title` |
| Build, package, verify and checksum the host target | `cargo xtask build-release` |
| Same for chosen targets (one operating system per run) | `cargo xtask build-release --target x86_64-pc-windows-msvc --target aarch64-pc-windows-msvc` |
| Run `--version` and `doctor` on a built executable | `cargo xtask smoke --artifact <path>` |
| Assemble the ten published assets from downloaded artifacts | `cargo xtask release-assets --artifact-root <dir> --output-directory <dir>` |
| Approve the exact commit SHA | `cargo xtask approve-release --sha <40-character-main-sha>` |
| Check the approved target, protected checks and latest ordering | `cargo xtask release-target --tag vX.Y.Z --version X.Y.Z --sha <sha> --approved-sha <sha> --is-prerelease false` |
| Check a Windows executable's PE machine | `cargo xtask check-pe --artifact <exe> --architecture x64` |
| Generate and verify a Windows SPDX SBOM | `cargo xtask sbom ...` / `cargo xtask check-sbom ...` |
| Read the efficiency policy of a running process (Windows) | `cargo xtask windows-efficiency --pid <pid>` |

`approve-release` and `release-target` read the repository slug from `.github/repository.toml`. Pass `--repository owner/name` to override it. Every command prints one JSON line (or a short text line) on success and exits with code 1 and a message on failure.

## Version surfaces

`scripts/release-contract.json` is the machine-readable owner for the candidate product version, core version, config schema, checksum manifest, and both Windows target, artifact, SBOM and PE-machine contracts. For v1.15.0:

- binary and workspace: 1.15.0;
- `codex-presence-core`: 2.0.1;
- config schema: 13.

The tag version, Cargo metadata, README release badge (`Release v<version>`), changelog section and release contract must agree before preflight succeeds.

## Release notes

The GitHub release has the title `CDRP <version>`, short enough for the Releases list, and a body that is the matching `CHANGELOG.md` section, unchanged: the summary paragraph and the Added, Changed, Deprecated, Removed, Fixed and Security sections. The body has no `#` or `##` heading, no asset list, no integrity section and no compare link, because GitHub shows the title, the assets and the checksums next to the notes.

Only a section longer than 150 lines also gets three closing sections: Release assets, Integrity and Full changelog.

Preview the notes before tagging:

```bash
cargo xtask release-notes 1.11.3
```

GitHub lets maintainers edit the title and notes of a published immutable release. Only the tag and the assets are locked. To correct a published release, run `gh release edit vX.Y.Z --title "CDRP X.Y.Z" --notes-file <file>`. See [Immutable releases](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases).

## Required proof

1. Run `cargo xtask check`. It runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo nextest run --workspace --all-features`, the changelog check and `llms --check`. The `xtask` tests cover the metadata, approval, target, asset, PE, SBOM, notes and workflow contracts.
2. Run the release build and `cargo audit --deny warnings`.
3. Prove schema 12 to 13 migration and parser fixtures independent of the user profile.
4. On Windows, exercise `status`, `doctor`, TUI persistence, Fast, semantic quota windows, Credits, and real Discord publication.
5. Keep Linux and macOS compile and test gates green.
6. Install Rust targets `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` plus the MSVC ARM64 toolset, then run `cargo xtask build-release --target x86_64-pc-windows-msvc --target aarch64-pc-windows-msvc`. The output lands in `releases/windows/`, which must be empty before the run.
7. The build validates PE machines `0x8664` and `0xAA64`, generates architecture-specific SPDX SBOMs, and includes all nine payloads in `SHA256SUMS.txt` (ten published assets including the manifest).
8. Before closing Windows ARM64 support, run `cargo xtask smoke` on the native `windows-11-arm` release job. A local x64-hosted cross-build does not substitute for that proof.

## Publish

1. Confirm the intended `main` commit has all required platform checks green.
2. Approve that exact commit:

   ```bash
   cargo xtask approve-release --sha <40-character-main-sha>
   ```

3. Create and push an annotated tag pointing to the approved commit:

   ```bash
   git tag -a vX.Y.Z <40-character-main-sha> -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

4. Publish the release. The Release workflow is manual-only, so pushing the tag does not start it. Either build and publish locally (`cargo xtask build-release`, then `gh release create vX.Y.Z <assets> --title "$(cargo xtask release-notes X.Y.Z --title)" --notes-file <(cargo xtask release-notes X.Y.Z)`), or dispatch the workflow by hand from the Actions tab for the tagged commit. On dispatch it rechecks tag ancestry, the approved SHA, version ordering, artifacts, both Windows SPDX SBOMs and PE machines, and SHA-256 digests before publishing once.
5. Verify the public release and then clear the one-use approval:

   ```bash
   gh variable delete RELEASE_APPROVED_SHA --repo xt0n1-t3ch/Codex-Discord-Rich-Presence
   ```

Never move a published tag or replace a published asset. Publish a new patch version when a correction to the code or the binaries is required. Corrections to the title or the notes are edits, as described above.
