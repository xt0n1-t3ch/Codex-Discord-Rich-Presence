# Docs

| Doc | Contract |
|:---|:---|
| [api/codex-presence.md](api/codex-presence.md) | CLI commands, Discord payloads, priority republish, surface detection, context windows, release artifacts |
| [database/schema.md](database/schema.md) | Local config, Codex JSONL, OpenCode SQLite, global state, plan cache, metrics snapshots |
| [ui/UI_SITEMAP.md](ui/UI_SITEMAP.md) | Terminal layout, plan picker, status cards, footer behavior |

Root references: [README.md](../README.md), [CHANGELOG.md](../CHANGELOG.md), [tests/index.md](../tests/index.md).

## Rules

| Rule | Standard |
|:---|:---|
| Runtime facts | Backed by code or tests |
| External facts | Include source URL and access date |
| Release facts | Match `.github/workflows/release.yml` |
| Scope | Durable contracts only; no plans or handoffs |
