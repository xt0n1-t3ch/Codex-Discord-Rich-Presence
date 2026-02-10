# Plan: TUI Layout + Idle Accuracy + Rich Presence Polish
Date: 2026-02-10
Slug: tui-layout-idle-presence-polish
Status: implemented

## 1) Discovery & Context
- Header/logo occasionally disappeared because rendered lines exceeded terminal height and scrolled buffer content.
- Footer credit was in normal flow, not anchored to bottom rows.
- Activity parser marked `Idle` too early on `function_call_output` / `custom_tool_call_output`.
- Discord presence text was noisy and not action-first.
- Rendering ran every poll tick even with unchanged frame data.

## 2) Scope & Non-Goals
### Scope
- Layout budgeting by terminal size with `Full | Compact | Minimal` modes.
- Bottom-anchored responsive author footer.
- Activity tracker with pending-call awareness and `Idle` debounce.
- Rich Presence composition prioritizing live action.
- Render/presence dedupe to cut unnecessary CPU/IPC work.

### Non-Goals
- No GUI rewrite outside terminal.
- No bundling of OpenAI brand image assets in repository.
- No changes to single-instance lock ownership model.

## 3) Architecture & Data Flow
1. Session parser streams JSONL events and updates `ActivityTracker`.
2. `ActivityTracker` stores current action, pending call count, and active timestamps.
3. Finalization applies `Idle` only after 45s inactivity and zero pending calls.
4. App loop computes frame signature and redraws only when changed, forced, or periodic refresh triggers.
5. UI renderer uses line budgets to prevent overflow and anchors footer to bottom rows.
6. Discord publisher builds action-first payload and skips redundant/rate-limited sends.

## 4) Interfaces & Schemas
### Internal Types
- `SessionActivitySnapshot` extended with:
  - `last_active_at`
  - `idle_candidate_at`
  - `pending_calls`
- New internal types in UI:
  - `UiLayoutMode`
  - `FrameBudget`

### Public Config/API
- No new required config keys.
- Existing schema remains backward-compatible (`schema_version: 3`).

## 5) Implementation Phases
1. Add `ActivityTracker` in `src/session.rs` and remove immediate `Idle` on tool output.
2. Improve read-target extraction for shell commands (`rg`, `Get-Content`, `Select-String`, `Get-ChildItem`, etc.).
3. Rewrite `src/ui.rs` renderer for height-safe layout and footer anchoring.
4. Add frame signature and app-side redraw dedupe in `src/app.rs`.
5. Redesign Discord presence lines in `src/discord.rs` (action-first details, compact state, send throttling).
6. Update docs and README to match final behavior.

## 6) Validation & Acceptance
### Automated
- `cargo fmt`
- `cargo test`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build --release`

### Added/Updated Test Scenarios
- Activity stays non-idle immediately after tool output.
- Activity becomes idle after debounce with no new events.
- Reading/editing classification for shell/apply_patch.
- Layout mode thresholds and footer budget.
- Discord details prioritize activity.

### Manual Acceptance
- Header remains visible without scroll loss in small/medium terminals.
- Footer credit remains fixed at bottom and responsive by width.
- Presence reflects live activity rather than frequent false `Idle`.

## 7) Rollout, Risks & Backout
### Rollout
- Ship as minor release with updated docs.
- Publish refreshed Windows artifact in `dist/windows/x64`.

### Risks
- Terminal image protocol support is heterogeneous.
- Heuristic parsing for read targets can still be ambiguous in edge commands.

### Mitigations
- Reliable ASCII/header fallbacks by layout mode.
- Conservative command classification and safe fallbacks to generic action labels.

### Backout
- Revert to previous parser/layout commit if regression appears.
- Force ASCII mode via config if logo-image path/protocol is unstable.
