# Privacy

`codex-discord-presence` runs locally and reads local Codex session files from `~/.codex/sessions`.

## Data Processed

- project path/name
- git branch (derived locally)
- model id/name
- token counters
- usage windows (5h / 7d)

## Data Flow

1. Data is parsed locally from local files.
2. Presence payload is sent to local Discord desktop via IPC.
3. No built-in analytics/telemetry endpoint is used by this project.

## Data Storage

- Config: `~/.codex/discord-presence-config.json`
- Lock file: `~/.codex/codex-discord-presence.lock`

No persistent cloud storage is performed by this application.
