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
- reads active Codex sessions from `~/.codex/sessions`,
- computes live activity and token/limit telemetry,
- renders adaptive TUI,
- publishes Discord Rich Presence updates,
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
- active session count,
- ranked active session summary,
- token triplet and remaining limits.

### 4) Doctor

```bash
codex-discord-presence doctor
```

- exits `0` healthy, `1` when warnings/issues are found.

## Session Selection Contract

When multiple sessions exist, active-session selection is deterministic:

1. higher `pending_calls`,
2. non-idle over idle,
3. newest `last_activity`.

This ranking is used for both TUI primary card and Discord presence source.

## Discord Presence Payload Contract

When privacy mode is disabled:

- `details`: `<activity> • <project>`
  - examples:
    - `Reading src/app.rs • MCP Servers`
    - `Thinking • Property Alpha (tony/mobile1)`
- `state`: prioritized compact telemetry
  - `<model> | Last response X | Session total Y | 5h left A% | 7d left B%`
  - bounded to Discord field limits (2..128 chars).

When privacy mode is enabled:

- `details = "Using Codex"`
- `state = "In a coding session"`

Update behavior:

- deduplicates identical payloads,
- rate-limits publish bursts via minimum interval.

## Environment Variables

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`
- `CODEX_PRESENCE_TERMINAL_RELAUNCHED` (internal relaunch guard)

## Local Config Contract

- Path: `~/.codex/discord-presence-config.json`
- Schema: `3`
- Key fields:
  - `discord_client_id`
  - `discord_public_key` (metadata)
  - `privacy.*`
  - `display.large_image_key`
  - `display.small_image_key`
  - `display.activity_small_image_keys` (optional per-activity small image keys)
    - `thinking`, `reading`, `editing`, `running`, `waiting`, `idle`
  - `display.terminal_logo_mode`
  - `display.terminal_logo_path`

## Exit Codes

- `0`: success
- `1`: runtime failure or doctor-reported issues
