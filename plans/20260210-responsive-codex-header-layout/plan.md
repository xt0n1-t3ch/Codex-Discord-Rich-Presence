# Plan: Responsive CODEX Header + Adaptive TUI Layout
Date: 2026-02-10
Slug: responsive-codex-header-layout
Status: implemented

## 1) Discovery & Context
- The full ASCII banner (OpenAI + CODEX) was tied to `UiLayoutMode::Full`.
- Medium terminals downgraded too early to text-only banner variants.
- Fixed blank spacer rows consumed vertical budget in constrained windows.
- `Recent Sessions` reservation had no extreme-height exception and could compete with header visibility.

## 2) Scope & Non-Goals
### Scope
- Keep CODEX ASCII branding visible across medium and small windows when space permits.
- Introduce deterministic banner fallback ladder with no partial ASCII rendering.
- Make section spacing adaptive by layout mode.
- Allow `Recent Sessions` reservation to drop to zero in extreme-height terminals.
- Update UI docs for the new responsive behavior.

### Non-Goals
- No Discord payload/contract changes.
- No config schema or CLI flag changes.
- No parser/session behavior changes outside rendering layout.

## 3) Architecture & Data Flow
1. Keep `select_layout_mode` for data density (`Full`, `Compact`, `Minimal`).
2. Add independent banner variant selection by width, available rows, and image eligibility:
   - `Image`
   - `ASCII Dual` (OpenAI + CODEX, full mode only)
   - `ASCII CODEX` (medium/small preferred fallback)
   - `Compact text`
   - `Minimal text`
3. Render only complete banner blocks; fall back before partial clipping.
4. Preserve breathing space in `Full` mode while removing fixed gaps in `Compact`/`Minimal`.
5. Reserve `Recent Sessions` rows adaptively, including `0` rows at extreme heights (`<= 12` body rows).

## 4) Interfaces & Schemas
### Internal Interfaces
- Added internal `BannerVariant` enum in `src/ui.rs`.
- Added helper functions for:
  - banner variant selection,
  - ASCII width computation,
  - dual/CODEX ASCII rendering,
  - logo image size/row estimation,
  - conditional section spacing.

### Public Interfaces
- No public API/config/schema changes.

## 5) Implementation Phases
1. Add plan artifact under `/plans/20260210-responsive-codex-header-layout/plan.md`.
2. Refactor banner rendering in `src/ui.rs` to use deterministic fallback selection.
3. Add CODEX-only ASCII intermediate banner for medium windows.
4. Replace fixed inter-section blank lines with full-mode-only spacing helper.
5. Update recent-section reservation policy for extreme-height windows.
6. Extend `src/ui.rs` tests for:
   - banner variant selection across target sizes,
   - constrained fallback behavior,
   - extreme-height recent-row policy.
7. Update UI documentation in `docs/ui/UI_SITEMAP.md`.

## 6) Validation & Acceptance
### Automated
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

### Target Scenarios
- `80x20`: CODEX ASCII should be selected.
- `100x28`: CODEX ASCII should remain visible.
- `120x34`: full banner path should allow dual ASCII (or image if eligible).
- very small windows (`<= 60x16`): compact/minimal fallback without partial ASCII clipping.

## 7) Rollout, Risks & Backout
### Rollout
- Ship as non-breaking TUI visual improvement.
- Keep behavior deterministic across terminal resizes.

### Risks
- Very small windows may hide `Recent Sessions` temporarily.
- Terminal image protocol support remains terminal-dependent.

### Mitigations
- Deterministic fallback ladder with text-safe endpoint.
- Explicit extreme-height reservation policy.
- Unit tests for target size classes and constrained fallbacks.

### Backout
- Revert `src/ui.rs` banner-selection and reservation changes.
- Restore prior fixed-spacing and reservation behavior if regressions appear.
