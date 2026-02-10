# codex-discord-presence CLI Contract

## Binary

- `codex-discord-presence`

## Commands

### 1) Smart foreground mode

```bash
codex-discord-presence
```

Behavior:

- acquires single-instance lock (with automatic takeover),
- reads active Codex sessions,
- computes real token metrics (`This update | Last response | Session total`),
- computes limits using **remaining** semantics (`100 - used`),
- computes live activity with pending-call tracking and idle debounce,
- keeps non-idle sessions visible with sticky activity window fallback,
- updates Discord Rich Presence,
- runs interactive TUI until `q` or `Ctrl+C`.

### 2) Codex wrapper mode

```bash
codex-discord-presence codex [args...]
```

Behavior:

- launches `codex` child process,
- keeps presence alive while child is running,
- clears/disconnects on child exit or signal.

### 3) Status

```bash
codex-discord-presence status
```

Output contract:

- `running: true|false`,
- optional `pid`,
- paths (`config`, `sessions_dir`),
- `active_sessions` count,
- active session summary:
  - activity summary (`Thinking`, `Reading`, `Editing`, etc.) when enabled,
  - token summary (`This update | Last response | Session total`),
  - remaining limits (`5h`, `7d`),
  - optional `limits_source_session`.

### 4) Doctor

```bash
codex-discord-presence doctor
```

- exits `0` when healthy, `1` when issues/warnings are found.

## Environment Variables

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`
- `CODEX_PRESENCE_TERMINAL_RELAUNCHED` (internal guard)

## Local Config Contract

- Path: `~/.codex/discord-presence-config.json`
- Schema: `3`
- Includes:
  - `discord_client_id`
  - `discord_public_key` (metadata)
  - `privacy`
    - `show_activity`
    - `show_activity_target`
  - `display`
    - `terminal_logo_mode`
    - `terminal_logo_path`

## Discord Presence Payload Contract

When privacy mode is disabled:

- `details`: action-first line
  - format: `<activity> • <project>`
  - examples:
    - `Thinking • Property Alpha (tony/mobile1)`
    - `Editing src/ui.rs • codex-discord-presence`
- `state`: compact telemetry line
  - format: `<model> | Last response X | Session total Y | 5h left A% | 7d left B%`
  - sections are included based on privacy flags and truncated to Discord limits.

When privacy mode is enabled:

- `details = "Using Codex"`
- `state = "In a coding session"`

Update behavior:

- deduplicates identical `(details, state)` payloads,
- applies a minimum IPC publish interval to avoid bursty updates.

## Exit Codes

- `0`: success
- `1`: runtime failure or doctor issues
