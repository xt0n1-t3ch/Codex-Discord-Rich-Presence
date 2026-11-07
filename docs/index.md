[Codex Discord Rich Presence](../README.md) / Documentation

# Codex Discord Rich Presence documentation

Start with the task you have, then follow the runtime, data or release contract it touches.

**State on 2026-11-07:** runtime 1.15.0, `codex-presence-core` 2.0.1 and configuration schema 13. Windows, Linux and macOS binaries are published; current cross-platform validation results belong in the test map, not inferred from artifact availability.

[Start here](#start-here) · [How it works](#how-it-works) · [Maintain it](#maintain-it) · [Rules](#rules)

## Start here

| I want to... | Read |
| --- | --- |
| Install on Windows, macOS or Linux | [README: Installation](../README.md#installation) |
| Use the dashboard or configure privacy | [README: Features](../README.md#features) · [Terminal UI](ui/UI_SITEMAP.md) |
| Understand missing cost or context values | [Runtime contract](api/codex-presence.md) |
| Update or remove the runtime | [Updating](../README.md#updating) · [Removal](../README.md#removal) |
| Run all local checks | [Test map](../tests/index.md) |

## How it works

| I want to... | Read |
| --- | --- |
| Understand GPT-6.1 Terra and other GPT-6 identities, pricing and context | [GPT-6 family contract](api/codex-presence.md#gpt-6-family-contract) · [Canonical model catalog](../src/model_catalog.json) |
| Understand schema 13, shared control and snapshot provenance | [Local schema](database/schema.md) |
| Understand layouts, keys and paused state | [Terminal UI](ui/UI_SITEMAP.md) |
| Inspect Windows EcoQoS and Idle priority | [Windows Efficiency mode](windows-efficiency.md) |
| Find Codex/OpenAI visual assets | [Branding guide](../assets/branding/README.md) |

## Maintain it

| I want to... | Read |
| --- | --- |
| Build and contribute | [Development](../README.md#development) · [Contributing](../CONTRIBUTING.md) |
| Check regressions and their validation status | [Test map](../tests/index.md) |
| Prepare an approved, immutable release | [Release procedure](releasing.md) |
| Review compatibility and version history | [Changelog](../CHANGELOG.md) |
| Find the documentation map for language models | [llms.txt](../llms.txt) · [Generator manifest](llms.toml) |
| Cite this project | [CITATION.cff](../CITATION.cff) |

## Rules

| I want to... | Read |
| --- | --- |
| Know what leaves the machine | [Privacy policy](../PRIVACY.md) |
| Report a vulnerability privately | [Security policy](../SECURITY.md) |
| Understand participation expectations | [Code of conduct](../CODE_OF_CONDUCT.md) |
| Check license terms | [MIT license](../LICENSE) |

Runtime facts must be backed by source or tests. External model facts need a source URL and verification date. Release instructions must match the manual release workflow. Keep plans, local agent state, credentials and private session evidence out of public documentation.
