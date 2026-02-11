# Plan: Direct Dist Build Output + Ready-to-Run Windows EXE
Date: 2026-02-10
Slug: direct-dist-exe-output
Status: implemented

## 1) Discovery & Context
- Local Cargo output was configured to `.build/target`.
- Users expecting an executable directly under `/dist` could see that folder as empty at root level.
- Existing release artifacts are grouped under `dist/<os>/<arch>`.
- GitHub release workflow expects build output in `.build/target/...` paths.

## 2) Scope & Non-Goals
### Scope
- Move default local Cargo target directory under `dist/`.
- Provide a deterministic one-command Windows build that leaves `dist/codex-discord-presence.exe` ready to open.
- Preserve CI and release workflow behavior to avoid packaging regressions.
- Update documentation to reflect new local-output behavior.

### Non-Goals
- No changes to runtime behavior of the application.
- No changes to Discord/session logic or TUI rendering behavior.
- No changes to release archive naming/layout.

## 3) Architecture & Data Flow
1. Update Cargo config so local compilations emit into `dist/target`.
2. Add a PowerShell build helper:
   - run `cargo build --release`,
   - copy release binary to `dist/codex-discord-presence.exe`,
   - refresh `dist/windows/x64/codex-discord-presence.exe`.
3. Pin CI/release `CARGO_TARGET_DIR=.build/target` so pipelines keep existing artifact behavior.
4. Update README build instructions and output paths.

## 4) Interfaces & Schemas
### Internal/Developer Interfaces
- `.cargo/config.toml`
  - `build.target-dir` changed to `dist/target`.
- New script:
  - `build-dist.ps1`

### Public Interfaces
- No API/config schema changes.
- No CLI flag changes.

## 5) Implementation Phases
1. Change local Cargo target dir to `dist/target`.
2. Add `build-dist.ps1` convenience script for Windows.
3. Keep workflow compatibility by setting `CARGO_TARGET_DIR` in CI/release jobs.
4. Update README with new local build output and script usage.
5. Build locally and verify EXE exists at direct `dist/` path.

## 6) Validation & Acceptance
### Automated
- `cargo build --release`

### Acceptance
- `dist/target/release/codex-discord-presence.exe` exists after build.
- `build-dist.ps1` produces:
  - `dist/codex-discord-presence.exe`
  - `dist/windows/x64/codex-discord-presence.exe`
- CI/release workflow files remain aligned with `.build/target` packaging behavior.

## 7) Rollout, Risks & Backout
### Rollout
- Adopt `build-dist.ps1` as default local Windows packaging command.
- Keep release automation stable through workflow-level `CARGO_TARGET_DIR`.

### Risks
- Developers may assume release CI uses `dist/target` if workflow overrides are unnoticed.
- Mixed historical artifacts may coexist under `dist/`.

### Mitigations
- README explicitly documents local vs release output behavior.
- Workflows explicitly define `CARGO_TARGET_DIR`.

### Backout
- Revert `.cargo/config.toml` target-dir to `.build/target`.
- Remove `build-dist.ps1` and restore README build-output section.
