# Docs Index

This folder tracks the runtime contracts and UI/data behavior for `codex-discord-presence`.

## Structure

- `api/codex-presence.md`
  - CLI/runtime contract, Discord payload contract, activity/context interpretation rules.
- `database/schema.md`
  - Local persisted files and derived runtime snapshot semantics.
- `ui/UI_SITEMAP.md`
  - TUI information architecture, responsive behavior, and activity rendering rules.

## Reading Order

1. `api/codex-presence.md` for executable behavior and payload contracts.
2. `database/schema.md` for telemetry/config/schema semantics.
3. `ui/UI_SITEMAP.md` for render order and display constraints.

## Scope Notes

- This project has no relational database; `database/schema.md` documents local file contracts.
- Build/release artifact layout is defined in API contract under `Build/Release Artifact Layout Contract`.
