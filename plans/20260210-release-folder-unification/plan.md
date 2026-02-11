# Plan: Release Folder Unification (`releases/` only)

## 1) Discovery & Context

- The repo currently mixes generated outputs across `dist/`, `.build/`, and sometimes `target/`.
- Build scripts and GitHub workflows still reference legacy output roots.
- User requirement: use one output root only (`releases/`) with clear OS-based folder grouping.

## 2) Scope & Non-Goals

### Scope

- Route Cargo target output to `releases/.cargo-target`.
- Update local Windows build script to emit only `releases/windows/...`.
- Update CI and Release workflows to build/package/upload from `releases/**`.
- Update docs and changelog to reflect the new artifact contract.
- Remove existing legacy generated directories (`dist`, `target`, `.build`) from workspace.

### Non-Goals

- No runtime behavior changes for Discord/session parsing.
- No CLI command interface changes.
- No change to config schema.

## 3) Architecture & Data Flow

- Single generated-output root: `releases/`.
- Local builds:
  - Cargo compilation artifacts: `releases/.cargo-target`.
  - Windows convenience outputs: `releases/windows/x64/executables` + `archives`.
- Release CI builds per target, then packages into:
  - `releases/windows/x64/{executables,archives}`
  - `releases/linux/distros/x64/{executables,archives}`
  - `releases/macos/<arch>/{executables,archives}`

## 4) Interfaces & Schemas

- Public build/distribution contract updates (docs):
  - artifact directory layout under `releases/`.
- No API payload/schema changes.
- No config schema changes.

## 5) Implementation Phases

1. Update `.cargo/config.toml` target-dir.
2. Update `build-dist.ps1` to output only under `releases/`.
3. Update `.github/workflows/ci.yml` and `.github/workflows/release.yml` paths.
4. Update `README.md` and `docs/api/codex-presence.md`.
5. Update `CHANGELOG.md`.
6. Remove legacy generated directories from workspace.

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo test`
- `cargo build --release`
- `./build-dist.ps1` (Windows local check)

Acceptance:

- No new generated artifacts appear under `dist/`, `.build/`, or root `target/`.
- Generated artifacts are placed under `releases/...` with OS/arch grouping.
- Workflows reference `releases/**` for artifacts.

## 7) Rollout, Risks & Backout

### Rollout

- Land script/workflow/docs updates together.
- Regenerate local binary with `build-dist.ps1` and verify path layout.

### Risks

- Any external scripts hardcoded to old `dist/...` paths will need updates.
- Existing local automation expecting `.build/target` may fail until migrated.

### Backout

- Revert workflow/script path changes as one unit.
- Restore previous `target-dir` in `.cargo/config.toml` if compatibility is required.
