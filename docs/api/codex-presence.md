# codex-discord-presence CLI Contract

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
- token/cost breakdown and remaining limits.

### 4) Doctor

```bash
codex-discord-presence doctor
```

- validates discovered session roots, Discord client id, and command availability.
- exits `0` healthy, `1` when warnings/issues are found.

## Build/Release Artifact Layout Contract

All generated build and release outputs are written under `releases/`:

- Cargo target cache/output root: `releases/.cargo-target`
- Windows executable output: `releases/windows/x64/executables/codex-discord-presence.exe`
- Linux executable output: `releases/linux/distros/x64/executables/codex-discord-presence`
- macOS executable outputs:
  - `releases/macos/x64/executables/codex-discord-presence`
  - `releases/macos/arm64/executables/codex-discord-presence`
- Packaged archives/checksums are emitted into sibling `archives/` directories for each OS/arch path.

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

## Activity Interpretation Contract

- `response_item.message` with `role = assistant` and `phase = commentary` is treated as a live-progress signal.
  - it does not overwrite an existing working activity.
  - it reactivates `Waiting for input` / `Idle` to `Thinking`.
- `response_item.message` with `role = assistant` and `phase = final_answer` maps to `Waiting for input`.
- assistant messages without a known phase fall back to `Waiting for input`.
- `event_msg.agent_message` is treated as commentary progress (not immediate waiting).
- `response_item.web_search_call` / `response_item.web_search_result` are treated as working activity signals.
- file read/edit targets are sanitized to basename (for example `src/session.rs` -> `session.rs`).
- Sticky visibility extends sessions within `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` for:
  - working activity kinds
  - `Waiting for input`.

## Discord Presence Payload Contract

When privacy mode is disabled:

- `details`: `<activity> • <project>`
  - examples:
    - `Reading app.rs • MCP Servers`
    - `Thinking • Property Alpha (tony/mobile1)`
- `state`: prioritized compact telemetry
  - `<model> | I X C Y O Z | Last response N | Session total M | Cost $K | 5h left A% | 7d left B%`
  - bounded to Discord field limits (2..128 chars).

When privacy mode is enabled:

- `details = "Using Codex"`
- `state = "In a coding session"`

Update behavior:

- deduplicates identical payloads,
- rate-limits publish bursts via minimum interval.
- sends heartbeat re-publish every `30s` even when payload is unchanged.
- reconnects IPC with exponential backoff (`5s` to `60s`) when Discord is unavailable.
- keeps an explicit idle card (`Codex CLI` / `Waiting for session`) instead of clearing activity.
- validates configured image keys against Discord app assets when available.
- skips invalid image keys and falls back to safe icon payload (avoids `?` placeholder on Discord mobile).

## Environment Variables

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`
- `CODEX_PRESENCE_TERMINAL_RELAUNCHED` (internal relaunch guard)

## Local Config Contract

- Path: `~/.codex/discord-presence-config.json`
- Schema: `4`
- Key fields:
  - `discord_client_id`
  - `discord_public_key` (metadata)
  - `privacy.*`
    - includes `show_cost`
  - `display.large_image_key`
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

## Metrics Persistence Contract

- JSON snapshot: `~/.codex/discord-presence-metrics.json`
- Markdown report: `~/.codex/discord-presence-metrics.md`
- Persist cadence: every `10s` (atomic tmp-file rename).
- Snapshot includes totals, cost breakdown, per-model aggregation, uptime, and active session count.

## Exit Codes

- `0`: success
- `1`: runtime failure or doctor-reported issues
