# Codex Discord Rich Presence CLI Contract

## Binary

- `codex-discord-presence`

## Commands

### 1) Smart foreground mode

```bash
codex-discord-presence
```

Behavior:

- acquires single-instance lock (automatic takeover supported),
- reads active Codex sessions from discovered session roots,
  - primary root: `CODEX_HOME/sessions` (default `~/.codex/sessions`),
  - Windows build also probes WSL roots (`\\\\wsl.localhost\\<distro>\\...\\.codex\\sessions`),
  - WSL runtime also probes Windows root (`/mnt/c/Users/<user>/.codex/sessions`),
- computes live activity plus token/cost/limit telemetry,
- renders adaptive TUI,
- publishes Discord Rich Presence updates,
- persists metrics snapshots to JSON/Markdown under `~/.codex/`,
- exits on `q` or `Ctrl+C`.

### 2) Codex wrapper mode

```bash
codex-discord-presence codex [args...]
```

- spawns `codex` child process,
- keeps presence active while child runs,
- clears presence on exit/signal.

### 3) Status

```bash
codex-discord-presence status
```

Outputs:

- running instance state (+ optional pid),
- discovered session roots,
- active session count,
- ranked active session summary,
- selected limits source details (`limit_id`, scope, freshness),
- token/cost breakdown and remaining limits.

### 4) Doctor

```bash
codex-discord-presence doctor
```

- validates discovered session roots, Discord client id, and command availability.
- exits `0` healthy, `1` when warnings/issues are found.

## Build/Release Artifact Layout Contract

Published release artifacts are written under `releases/`:

- Windows executable output: `releases/windows/codex-discord-rich-presence.exe`
- Linux executable output: `releases/linux/codex-discord-rich-presence`
- macOS executable output: `releases/macos/codex-discord-rich-presence`
- Internal Cargo build cache is routed to `.build/target` (gitignored) so root-level `target/` stays out of the workspace.
- Release packaging scripts:
  - `scripts/build-release.ps1` (Windows)
  - `scripts/build-release.sh` (Linux/macOS)

## Session Selection Contract

When multiple sessions exist, active-session selection is deterministic:

1. newest `last_activity`,
2. higher `pending_calls`,
3. activity class priority:
   - working (`Thinking`, `Reading`, `Editing`, `Running command`)
   - `Waiting for input`
   - `Idle`
4. stable `session_id` tiebreak.

This ranking is used for both TUI primary card and Discord presence source.

## Limits Selection Contract

- Effective 5h/7d selection is `global codex-first`:
  1. Prefer newest valid envelope with `rate_limits.limit_id = "codex"`.
  2. Fallback to newest valid non-`codex` envelope (`codex_*` or other) only if global is missing.
- Parser retains multiple envelopes per session to avoid losing global data when events alternate (`codex` + `codex_*`).
- Status/TUI expose selected source as `<limit_id> (<scope>)` + freshness (`Updated: <age>`).

## Activity Interpretation Contract

- `response_item.message` with `role = assistant` and `phase = commentary` is treated as a live-progress signal.
  - it does not overwrite an existing working activity.
  - it reactivates `Waiting for input` / `Idle` to `Thinking`.
- `response_item.message` with `role = assistant` and `phase = final_answer` maps to `Waiting for input`.
- assistant messages without a known phase fall back to `Waiting for input`.
- `event_msg.agent_message` is treated as commentary progress (not immediate waiting).
- `response_item.web_search_call` / `response_item.web_search_result` are treated as working activity signals.
- `response_item.function_call` supports `shell_command` and `exec_command`.
  - command text is read from JSON argument key `command` or `cmd`.
  - running commands are summarized for presence readability (for example `rg --files`, `cargo test`, `sed -n`).
- file read/edit targets are sanitized to basename (for example `src/session.rs` -> `session.rs`).
- Sticky visibility extends sessions within `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` for:
  - working activity kinds
  - `Waiting for input`.

## Surface Detection Contract

- Runtime surface is auto-detected from `session_meta`:
  - `originator` containing `desktop` => Desktop surface.
  - fallback: string `source` containing `desktop` => Desktop surface.
  - otherwise => default Codex CLI / Codex VS Code Extension surface.
- Non-string `source` payloads (for example subagent metadata objects) are ignored for surface detection.
- Idle surface sticks to the latest detected active surface, so the idle card remains consistent.

## Discord Presence Payload Contract

When privacy mode is disabled:

- `details`: `<activity> - <project>`
  - examples:
    - `Reading app.rs - MCP Servers`
    - `Thinking - Property Alpha (tony/mobile1)`
- `state`: prioritized compact telemetry
  - `<model> | <plan> • $cost • N tok • Ctx L% • 5h A% • 7d B%`
  - model labels can include:
    - Fast prefix: `⚡ GPT-5.4`
    - effort suffix: `GPT-5.4 (Extra High)`
  - plan label is resolved from either:
    - telemetry/cache (`openai_plan.mode = "auto"`), or
    - config override (`openai_plan.mode = "manual"`).
  - truncation policy drops lower-priority tail fields first.
  - model + plan and cost remain pinned whenever present.
  - bounded to Discord field limits (2..128 chars).

When privacy mode is enabled:

- `details = "Using Codex"`
- `state = "In a coding session"`

Update behavior:

- deduplicates identical payloads,
- rate-limits publish bursts via minimum interval.
- sends heartbeat re-publish every `30s` even when payload is unchanged.
- reconnects IPC with exponential backoff (`5s` to `60s`) when Discord is unavailable.
- keeps an explicit idle card (`Codex CLI / Codex VS Code Extension` or `Codex App` / `Waiting for session`) instead of clearing activity.
- can switch Discord application/client dynamically when surface changes (Codex CLI / Codex VS Code Extension <-> Desktop).
- validates configured image keys against Discord app assets when available.
- skips invalid image keys and falls back to safe icon payload (avoids `?` placeholder on Discord mobile).

## Environment Variables

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_DISCORD_CLIENT_ID_DESKTOP`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`
- `CODEX_PRESENCE_TERMINAL_RELAUNCHED` (internal relaunch guard)

## Local Config Contract

- Path: `~/.codex/discord-presence-config.json`
- Schema: `8`
- Key fields:
  - `discord_client_id`
  - `discord_client_id_desktop`
  - `discord_public_key` (metadata)
  - `privacy.*`
    - includes `show_cost`
  - `display.large_image_key`
  - `display.desktop_large_image_key`
  - `display.desktop_large_text`
  - `display.small_image_key`
  - `display.activity_small_image_keys` (optional per-activity small image keys)
    - `thinking`, `reading`, `editing`, `running`, `waiting`, `idle`
  - image keys accept either:
    - uploaded Discord asset keys, or
    - `https://...` external image URLs
  - `display.terminal_logo_mode`
  - `display.terminal_logo_path`
  - `pricing.aliases` (model-id alias map)
  - `pricing.overrides` (per-model `input_per_million`, `cached_input_per_million`, `output_per_million`)
  - `openai_plan.mode` (`auto` or `manual`)
  - `openai_plan.tier` (displayed plan when `openai_plan.mode = "manual"`)
  - `openai_plan.show_price` (applies to the resolved plan label)

## Plan Detection Contract

- Runtime plan source priority:
  1. `openai_plan.mode = "manual"` => use configured `openai_plan.tier`,
  2. latest non-null `rate_limits.plan_type` (prefer global `limit_id=codex` signal),
  3. in-memory last known telemetry value,
  4. persisted cache `~/.codex/discord-presence-plan-cache.json`,
  5. fallback `Unknown`.
- Supported mapped tiers:
  - `free`, `go`, `plus`, `business`, `enterprise`, `pro`, `unknown`.
- Spark policy:
  - `gpt-5.3-codex-spark` is treated as Pro-only for diagnostics.
  - non-Pro + Spark is flagged as telemetry anomaly in TUI (no crash; Discord remains compact).

## Fast Mode + Effort Contract

- Fast mode is resolved from `~/.codex/.codex-global-state.json`.
  - source JSON path: `electron-persisted-atom-state.default-service-tier`
  - `fast` => Fast mode on
  - any other or missing value => Fast mode off
- Reasoning effort is resolved from the active session turn context:
  1. `turn_context.payload.effort`
  2. fallback `turn_context.payload.collaboration_mode.settings.reasoning_effort`
- Supported effort labels:
  - `minimal`, `low`, `medium`, `high`, `xhigh`
  - `xhigh` is displayed as `Extra High`

## Context Window Contract

- Preferred window source: `event_msg.token_count.info.model_context_window`.
- Fallback window source: model catalog context window (for known families, e.g. GPT-5/Codex `400_000`).
- `used_tokens` is taken from active-turn usage (`info.last_token_usage.total_tokens`) when available.
- Fallback to session total is allowed only when the session total does not exceed the window.
- This prevents false values like multi-million cumulative totals being shown as current context usage.

## Metrics Persistence Contract

- JSON snapshot: `~/.codex/discord-presence-metrics.json`
- Markdown report: `~/.codex/discord-presence-metrics.md`
- Persist cadence: every `10s` (atomic tmp-file rename).
- Snapshot includes totals, cost breakdown, per-model aggregation, uptime, and active session count.

## Exit Codes

- `0`: success
- `1`: runtime failure or doctor-reported issues
