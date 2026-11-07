# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Version numbers describe compatibility of the CLI and its commands, the configuration schema, the Discord presence output and the `codex-presence-core` API. A major version signals incompatible public changes, a minor version adds compatible behavior, and a patch fixes compatible behavior. The shared core has its own version; each release records core and schema compatibility when established.

## [Unreleased]

## [1.15.0] - 2026-11-07

CDRP 1.15.0 keeps its files in its own folder instead of `~/.codex`, starts no background processes, adds GPT-6.1 Terra, and moves release tooling to Rust. The shared core stays at 2.0.1 and configuration schema 13 is unchanged.

### Added

- GPT-6.1 Terra model identity, context limits and Standard/Fast pricing.
- `cargo xtask` for local checks, documentation, changelog, metadata and release tasks.
- Generated llms.txt and llms-full.txt documentation maps and CITATION.cff.

### Changed

- Align GPT-5.6 Sol, Terra and Luna rates with the published pricing table.
- Apply long-context pricing to requests above 272,000 input tokens for GPT-5.6 Sol, Terra and Luna, GPT-5.5 and GPT-5.4: 2x input and cache rates and 1.5x output rates.
- Match Pulse release notes: one summary paragraph and categorized changes without duplicated titles, asset lists, integrity sections or compare links.
- Use Rust 1.99.1, rusqlite 0.41 and ureq 3, with faster development builds.
- Replace PowerShell release scripts and their tests with Rust tooling.
- Follow Keep a Changelog 2.0.0.
- Describe every `cargo xtask` command and argument in `--help`, and document every module and the public items of the shared core.
- Keep every file the program writes in `~/.codex-discord-presence` (`CODEX_PRESENCE_HOME`). The first start moves the config and plan cache out of `~/.codex` and removes the old lock and metrics files; the Codex home is only read.
- Read the Git branch from `.git/HEAD` instead of running `git` on every poll, list WSL distros once per process, and read the launching process chain from `/proc` on Linux instead of running `ps`.
- Walk every session folder at most every 30 seconds; polls in between read today's and yesterday's folders and the recent files.
- Redraw the dashboard only when its content changes, write the plan cache only when the plan changes, and re-send an unchanged Discord activity every 30 seconds instead of every 2.
- Run without a window when started without a terminal; `CODEX_PRESENCE_OPEN_TERMINAL=1` opens one.
- Replace the author line in the dashboard footer with a clickable "Endorse at" link to this repository (OSC 8), shortened or dropped when the terminal is narrow.

### Removed

- Unused `ContextSource::Event` and `ContextSource::Catalog` aliases and duplicated OpenAI plan label helpers. Configuration files that use `event` or `catalog` still load.
- The metrics JSON and Markdown files rewritten every 10 seconds; the dashboard keeps metrics in memory.

### Fixed

- End daemon child processes with their parent on Linux.
- Check the target operating system instead of the host in build scripts.
- Repair the corrupted build-command path in the README and document `cargo xtask build-release`.
- Stop leaving `-wal` and `-shm` files next to OpenCode's database.
- Keep tests out of the user's Codex home.
- Treat a release up to 1.11 that still holds its lock in the Codex home as the running instance, so an upgrade never runs two presences at once, and keep that lock until it is released.
- Isolate the presence folder (`CODEX_PRESENCE_HOME`) as well as the Codex home in `cargo xtask smoke`, so a smoke run never touches the real configuration.
- Build the macOS ARM64 release on `macos-15`; GitHub retired the `macos-14` runner.
- Run the release contract of the checkout and the RustSec audit in `cargo xtask check`, so a release no longer stops at preflight for something the local gate could see.

### Security

- Update rustls-webpki to 0.103.16 or later and webpki-roots to 1.0.10 or later for RUSTSEC-2026-0381 and RUSTSEC-2026-0382, also proposed in PR #56 by @nekotyy.

## [1.11.3] - 2026-10-17

GPT-6.1 Sol, GPT-6 Sol and GPT-6 Luna support with bounded scanner memory and child processes. Runtime 1.11.3 keeps `codex-presence-core` 2.0.1, its public API, configuration schema 13, Discord identity rules and privacy controls. Costs remain telemetry-based estimates, and the reported GPT-6.1 Sol 500K Codex default had no OpenAI primary source in this release. Restore the previous executable to roll back; configuration and session data are not migrated or rewritten.

### Added

- Add GPT-6.1 Sol, GPT-6 Sol and GPT-6 Luna to the model catalog with API rates, Fast pricing, the 272K long-context surcharge and API context limits verified on 2026-10-16. The bundled context fallback uses Codex capacity, not the API maximum. GPT-6.1 Sol uses the reported 500K Codex default, which has no OpenAI primary source. Credit rates stay unavailable.

### Changed

- Replace the unmaintained `fs2` crate with standard-library file locks.
- Update semver-compatible dependencies, including `thiserror` 2.0.21, and update the `upload-artifact`, `download-artifact` and `rust-toolchain` action pins. Dependabot now proposes minor Cargo updates; major updates stay ignored.

### Fixed

- The session scanner checks file modification times before it opens a file. Files older than the stale and sticky cutoffs are not read. The parse cache keeps only the files that can still be included, plus the four newest files for account limits. A file that becomes active again is parsed from the start.
- Parse `~/.codex/models_cache.json` once for each modification instead of on every snapshot rebuild.
- Discard session lines longer than 16 MiB instead of buffering them, and stop copying each line.
- Expire tool calls that never complete after two hours, and keep at most 256 open calls per session.
- Resolve Git branches only for included sessions. Each Git probe has a two-second timeout; on expiry the probe is killed and reaped.
- In wrapper mode, kill and reap the `codex` child on every exit path, including an early error and Ctrl+C. On Windows, a kill-on-close job object also ends its process tree.
- Single-instance takeover checks that the PID in the instance file still runs the recorded executable before it terminates that process. On Windows, process checks and termination use native APIs instead of `tasklist` and PowerShell.

## [1.11.2] - 2026-09-19

Per-event session costing and reliable currency display, including partial known subtotals. Runtime 1.11.2 and `codex-presence-core` 2.0.1 preserve the public core API, configuration schema 13, Discord identity rules and privacy controls. Costs remain estimates from observed telemetry and selected rates; snapshots retain coverage and provenance. Restore the previous executable to roll back; configuration and session data are not migrated or rewritten.

### Fixed

- Accumulate session cost per usage event with model, speed and cache-write telemetry. Reject repeated events and preserve known multimodel subtotals.
- Apply supported long-context prices to observed request input, not accumulated session input.
- Publish enabled known monetary amounts as currency only, including partial subtotals, while keeping unavailable amounts absent and preserving coverage in snapshots.
- Reserve space for enabled costs in the shared Discord compositor when model labels are long.

### Security

- Update `rustls` to 0.23.45 to address RUSTSEC-2026-0285, which affects TLS 1.3 handshake validation.

## [1.11.1] - 2026-09-05

GPT-6 Astra support and Windows Efficiency mode, with warning-denying Linux builds restored. The v1.11.0 candidate did not publish release assets; this release includes its Astra, plan-precedence, monetary-display and Windows efficiency changes.

### Added

- Publish GPT-6 Astra identity, observed effort, context limits and Standard/Fast pricing with honest incomplete-cost handling.
- Publish process-local Windows Efficiency mode with an environment opt-out and read-only policy verifier.

### Fixed

- Make Windows Efficiency mode initialization an explicit no-op on other operating systems, so warning-denying Linux builds pass without suppressing lints. Windows EcoQoS behavior is unchanged.

## [1.11.0] - 2026-09-05

Unshipped candidate for GPT-6 Astra, plan precedence, monetary display and Windows Efficiency mode. The shared core remains 2.0.0, and Codex identity policy and configuration schema 13 are unchanged. Its changes ship in 1.11.1 instead.

### Added

- GPT-6 Astra identity, observed reasoning effort, a 1,050,000-token total context window, 922,000-token input limit and 128,000-token output limit.
- Astra Standard/Fast pricing resolution with explicit incomplete coverage and no invented subscription-credit rates.
- Windows Efficiency mode through process-local EcoQoS and Idle priority, with an environment opt-out and read-only process verifier.

### Changed

- README and release documentation explain the standalone terminal interface, source boundaries, Windows process policy and immutable artifact checks.
- The shared core remains version 2.0.0. Codex identity policy and config schema 13 are unchanged.

### Fixed

- Manual plan overrides resolve before automatic cache updates. Protocol pro maps to Pro 20x and prolite to Pro 5x; Edu and explicit plan aliases are supported.
- Unknown plan labels are omitted from public Discord model text when no reliable identification exists.
- Monetary display rounds to two decimal places while retaining full calculation precision and cost-completeness rules.

## [1.10.3] - 2026-08-25

Desktop sessions identified by `codex_work_desktop` retain the selected Codex App or ChatGPT App identity, alongside compatible dependency updates.

### Changed

- Refreshed `thiserror`, `serde`, and `ctrlc` to their compatible patch
  releases.

### Fixed

- Sessions whose originator is `codex_work_desktop` now retain the selected
  Codex App or ChatGPT App desktop identity even when their source is VS Code.

## [1.10.2] - 2026-08-12

Daybreak Blue and Cyber Red model support and Windows ARM64 packaging, with release-preflight and publication checks corrected. Model contracts, displays, formatting, lint, workspace tests, release builds, Windows PE files, SBOMs and checksums were covered by local validation. Native Windows x64 and ARM64 version and doctor smoke checks remain mandatory in the manual release workflow.

### Added

- Native Daybreak Blue and Cyber Red model identities, canonical aliases, six reasoning efforts, runtime-first Blue context, and published Red context/pricing without invented Fast or credit rates.
- Windows ARM64 packaging with `codex-discord-rich-presence-windows-arm64.exe`, an architecture-specific SPDX SBOM, PE `0xAA64` validation, and native `windows-11-arm` release smoke.

### Changed

- Release assembly now publishes exactly ten assets: Windows x64 and ARM64 executables/SBOMs, Linux and macOS binaries, two logos, and one checksum manifest.
- Locked dependencies now resolve `anyhow` 1.0.104, `chrono` 0.4.45, and `thiserror` 2.0.19; transitive `lru` is updated to 0.18.2 to clear RUSTSEC-2026-0253.

### Fixed

- Release preflight clears the expected native-command exit status left by negative contract probes, so a fully passing suite exits successfully on Linux PowerShell.
- Publication validates the six release jobs that actually run, preflight plus every Linux, macOS, and Windows architecture, instead of requiring retired per-push CI contexts.

### Security

- Both Windows binaries receive architecture-specific SPDX 2.3 SBOMs, PE-machine validation, and SHA-256 verification before an immutable release is finalized.

## [1.10.1] - 2026-08-12

Tagged Daybreak Blue, Cyber Red and Windows ARM64 update with the release-preflight exit status corrected. Local validation covered model contracts and displays, formatting, lint, workspace tests, release builds, Windows PE files, SBOMs and checksums. Native Windows x64 and ARM64 smoke checks remain required by the release workflow.

### Added

- Native Daybreak Blue and Cyber Red model identities, canonical aliases, six reasoning efforts, runtime-first Blue context, and published Red context/pricing without invented Fast or credit rates.
- Windows ARM64 packaging with `codex-discord-rich-presence-windows-arm64.exe`, an architecture-specific SPDX SBOM, PE `0xAA64` validation, and native `windows-11-arm` release smoke.

### Changed

- Release assembly now publishes exactly ten assets: Windows x64 and ARM64 executables/SBOMs, Linux and macOS binaries, two logos, and one checksum manifest.
- Locked dependencies now resolve `anyhow` 1.0.104, `chrono` 0.4.45, and `thiserror` 2.0.19; transitive `lru` is updated to 0.18.2 to clear RUSTSEC-2026-0253.

### Fixed

- Release preflight clears the expected native-command exit status left by negative contract probes, so a fully passing suite exits successfully on Linux PowerShell.

### Security

- Both Windows binaries receive architecture-specific SPDX 2.3 SBOMs, PE-machine validation, and SHA-256 verification before an immutable release is finalized.

## [1.10.0] - 2026-08-12

Tagged Daybreak Blue, Cyber Red and Windows ARM64 update. Local validation covered model contracts and displays, formatting, lint, workspace tests, release builds, Windows PE files, SBOMs and checksums. Native Windows x64 and ARM64 smoke checks remain required by the release workflow.

### Added

- Native Daybreak Blue and Cyber Red model identities, canonical aliases, six reasoning efforts, runtime-first Blue context, and published Red context/pricing without invented Fast or credit rates.
- Windows ARM64 packaging with `codex-discord-rich-presence-windows-arm64.exe`, an architecture-specific SPDX SBOM, PE `0xAA64` validation, and native `windows-11-arm` release smoke.

### Changed

- Release assembly now publishes exactly ten assets: Windows x64 and ARM64 executables/SBOMs, Linux and macOS binaries, two logos, and one checksum manifest.
- Locked dependencies now resolve `anyhow` 1.0.104, `chrono` 0.4.45, and `thiserror` 2.0.19; transitive `lru` is updated to 0.18.2 to clear RUSTSEC-2026-0253.

### Security

- Both Windows binaries receive architecture-specific SPDX 2.3 SBOMs, PE-machine validation, and SHA-256 verification before an immutable release is finalized.

## [1.9.0] - 2026-08-04

Provider-neutral usage lanes in `codex-presence-core` 2.0.0 and configuration schema 13, with adaptive quotas, Credits and deterministic Discord composition. Recorded validation covered Windows release contracts, locked workspace gates, RustSec, schema migration, privacy, provider lanes and real Discord SET/CLEAR acknowledgements. The release notes also required Linux, macOS and Windows compile/test gates.

### Added

- `codex-presence-core` 2.0.0 as the UI-free owner of adaptive provider-neutral usage lanes, stream isolation, semantic quota scopes/windows, credits, service tier, session selection, and deterministic two-line Discord composition.
- Canonical parsing for authenticated `account/rateLimits/read` responses, preserving `rateLimitResetCredits` count/details separately from quota windows and keeping protocol `null` versus empty detail states intact.
- Explicit provider usage signals and `UsageSnapshotCollection` preserve Codex, OpenAI API, Claude, and Anthropic streams without cross-lane or cross-account merging; the upstream runtime remains Codex-local.
- Schema 13 field layout for project, branch, model, activity, tokens, cost, quotas, credits, context, and systems, including visibility, zone, order, and compact/descriptive presentation.
- Credits support with explicit zero, unlimited, absent, and malformed input behavior; Credits is enabled in Standard and Full presets and remains individually private.
- Operational TUI usage and privacy surfaces for semantic quota windows, scoped limits, Credits, and `⚡ Fast` presentation.

### Changed

- Usage windows are dynamic and named from each `window_minutes`; arbitrary and weekly-only windows render truthfully without positional `primary`/`secondary` assumptions or fabricated `5h` windows.
- Global-account Credits take precedence within a stream; ambiguous/equal-rank envelopes and unknown lanes fail closed.
- Service-tier authority is active-session evidence first, `config.toml` fallback second, legacy global state third, then explicit unknown/standard behavior.
- Rich Presence composition preserves all observed global/model quota envelopes, omits unavailable values, and enforces Discord's 128-character line boundary deterministically.
- Repository governance now uses YAML issue forms, Conventional Commits, runtime-proof PRs, synchronized SemVer/core/schema release gates, Dependabot, and a Windows SPDX SBOM contract.

### Fixed

- Parser tests no longer depend on a developer's real `models_cache.json`.
- Explicit Standard sessions no longer inherit stale global Fast state.
- Active OpenCode sessions no longer inherit Codex subscription quota or plan evidence from an unrelated session lane.
- Usage-window deserialization rejects missing percentages and non-positive durations instead of fabricating `100% remaining`; provider-reported remaining percentages and individual-account scope are preserved.
- `discord-proof` correlates both SET and CLEAR acknowledgements by nonce, redacts local session identity and hidden costs, and prevents shell setup statements or command arguments from entering the public activity.

### Security

- Release artifacts retain immutable tag/SHA approval and checksum verification; the Windows binary additionally receives a validated SPDX 2.3 SBOM.

## [1.8.0] - 2026-07-16

Shared `codex-presence-core` 1.0.0, schema 13 and semantic quotas, Credits and presence layout. The published notes retained candidate wording and listed Windows workspace gates, Linux/macOS compile and test gates, and live Fast, quota, Credits, TUI and Discord proof as pending before promotion; they did not record completed validation.

### Added

- `codex-presence-core` 1.0.0 as the UI-free owner of semantic quota scopes/windows, credits, service tier, session selection, and deterministic two-line Discord composition.
- Schema 13 field layout for project, branch, model, activity, tokens, cost, quotas, credits, context, and systems, including visibility, zone, order, and compact/descriptive presentation.
- Credits support with explicit zero, unlimited, absent, and malformed input behavior; Credits is enabled in Standard and Full presets and remains individually private.
- Operational TUI usage and privacy surfaces for semantic quota windows, scoped limits, Credits, and `⚡ Fast` presentation.

### Changed

- Usage windows are named from `window_minutes`; a 10,080-minute weekly window renders as `7d` and never fabricates a `5h` window when none exists.
- Service-tier authority is active-session evidence first, `config.toml` fallback second, legacy global state third, then explicit unknown/standard behavior.
- Rich Presence composition preserves all observed global/model quota envelopes, omits unavailable values, and enforces Discord's 128-character line boundary deterministically.
- Repository governance now uses YAML issue forms, Conventional Commits, runtime-proof PRs, synchronized SemVer/core/schema release gates, Dependabot, and a Windows SPDX SBOM contract.

### Fixed

- Parser tests no longer depend on a developer's real `models_cache.json`.
- Explicit Standard sessions no longer inherit stale global Fast state.

### Security

- Release artifacts retain immutable tag/SHA approval and checksum verification; the Windows binary additionally receives a validated SPDX 2.3 SBOM.

## [1.7.6] - 2026-07-10

Silent Windows background probes prevent console flashes without changing interactive children. Recorded validation covered the silent-launcher source contract, a Windows child-process trace, formatting, lint, workspace tests, release build and RustSec audit.

### Fixed

- Windows background Git branch probes now use the shared `CREATE_NO_WINDOW` launcher, preventing console flashes during session polling.
- Process-lineage, takeover, task-list, relaunch-wrapper, and command-availability probes use the same silent launcher without changing interactive Codex or terminal children.

## [1.7.5] - 2026-07-10

Persistent pause/resume control in schema 12 and live configuration reload across foreground, headless and wrapper modes. Schema-11 installations default to enabled. Recorded validation covered migration, last-good reload, all runtime loops, pause/resume transitions, terminal controls and the formatting, lint, test, release-build and audit gates.

### Added

- Schema 12 adds the durable `presence_enabled` master switch, defaulting to enabled for every existing schema-11 installation.
- Ratatui exposes `M` as a direct pause/resume shortcut and shows both the persisted switch state and truthful Discord `Paused` status.

### Fixed

- Foreground TUI, headless, and Codex-wrapper loops reload the shared persisted config on every poll, so Pulse changes to design and all nine privacy fields apply without restarting or terminating either process.
- Disabling presence clears Discord activity once per transition while local session monitoring continues; re-enabling invalidates the prior payload and republishes current state.
- Invalid, incomplete, missing, or transiently replaced config reads now log the failure and preserve the last valid runtime config instead of crashing a running loop.

## [1.7.4] - 2026-07-10

A persistent terminal privacy editor and shared public Discord presentation contract. Recorded validation covered the final-payload privacy matrix, editor rendering and keyboard behavior, formatting, lint, workspace tests, release build and RustSec audit.

### Added

- Ratatui now has a persistent `V` privacy editor for project name, Git branch, model, activity, token count, cost, session limits, context usage, and system signals. Each option updates the live Discord payload immediately.
- `discord::active_presence_presentation` and `idle_presence_presentation` expose one public presentation contract for the daemon and immutable downstream consumers such as Pulse.

### Fixed

- Context visibility is independent from token visibility, and disabling Systems removes the Discord small activity icon and tooltip instead of leaving a hidden data path active.
- Discord activity construction now consumes the same public presentation object used by previews, preventing app title, branch, model/reasoning, asset, and privacy drift.

## [1.7.3] - 2026-07-09

Selected Discord activity titles and separate model, reasoning and Fast segments now stay consistent across surfaces. Recorded validation covered formatting, lint, workspace tests, release build, RustSec audit, downloaded assets and their checksum manifest.

### Fixed

- Discord activities now send the selected surface label through the RPC `name` field, so the `ChatGPT App` design overrides the underlying application title instead of changing only the logo. The same owner supplies `Codex App`, `Codex CLI`, and `Codex VS Code Extension` titles.
- GPT model presentation now keeps the exact Codex App label in the factual catalog while rendering the human-facing line as `GPT-5.6 Sol · Max | Pro 20x ($200/month)`. Reasoning and Fast are separate ` · ` segments in Discord, Ratatui, status output, and OpenCode views.
- Presence payload equality includes the activity title, forcing an immediate republish when the selected design changes.

## [1.7.2] - 2026-07-09

GPT-5.6 catalog support, persistent desktop design selection and stricter telemetry, context and privacy handling. Recorded validation covered formatting, lint, workspace tests, release build, RustSec audit and the release contract, approval, target, asset and workflow suites.

### Added

- A machine-readable GPT-5.6 catalog for Sol, Terra, and Luna with App labels, aliases, supported efforts, Fast capability, 372K raw / 353.4K usable context, verified API rates, Codex credit rates, prompt-cache policy, and dated source metadata.
- Persistent schema-10 desktop design selection. Press `D` in Ratatui to switch between `Codex App` and `ChatGPT App`; the runtime reconnects through the corresponding Discord application id.
- `assets/branding/chatgpt-app.jpg`, copied byte-for-byte from the selected ChatGPT-style source asset for downstream Pulse previews.

### Changed

- Discord and Ratatui now show the chosen reasoning level and independent speed, including `5.6 Sol Max` and `5.6 Sol Max · Fast`.
- CLI, VS Code Extension, desktop, and OpenCode surfaces use exact labels. Active-session metadata wins; fallback detection now requires an extension-host or explicit environment marker instead of inferring VS Code/OpenCode from generic process names, terminal variables, or `PATH` entries.
- Cost output carries known subtotal, cache-write component, provenance, reconciliation, attribution, and `exact` / `partial` / `unavailable` status. Partial subtotals remain diagnostic and are omitted from public presence; unknown models never inherit an older model's price.
- Context resolution is `observed JSONL > local models_cache.json > bundled catalog`; snapshots retain usable and raw windows plus the source of each, and UI copy no longer substitutes blanket 400K/1.05M assumptions for GPT-5.6.
- The carried-forward App inventory now matches Codex 0.144.0: 5.5/5.4/5.4 Mini use 272K raw at 95%, Spark uses 128K raw at 95%, and Fast is unavailable for 5.4 Mini and Spark. GPT-5.4 Mini uses current `$0.75 / $0.075 / $4.50` API rates; Spark remains unpriced while its credit rates are marked research preview.

### Fixed

- Session-scoped Standard/Fast state survives later turn-context records and never inherits a global speed. Mixed models or speeds are attributed explicitly.
- Cached input is bounded by input tokens, hostile totals saturate safely, local model-cache reads are size bounded through one file handle, and activity text no longer leaks command arguments, search queries, credentials, URLs, or host-specific path prefixes.
- Provider-reported OpenCode totals keep their own provenance and hide component breakdowns unless those components reconcile.
- Cumulative session input above a published long-context threshold is marked partial because session totals cannot prove whether a single prompt crossed the billing boundary.

### Security

- Removed the unused `viuer` dependency and its vulnerable/unmaintained AVIF stack, updated `anyhow` to 1.0.103, and made pinned `cargo-audit 0.22.2 --deny warnings` a required CI and release-preflight gate.
- CI and immutable release publication now use Rust 1.96.1, locked dependencies, pinned Actions, least-privilege permissions, annotated-tag and exact-SHA approval, latest protected-check validation, portable asset names, digest verification, and `SHA256SUMS.txt` across Windows, macOS, and Linux.

## [1.7.1] - 2026-07-06

A monochrome terminal dashboard, opt-in WSL discovery and distinct Pro plan tiers. Recorded validation covered WSL safety, plan behavior and terminal UI tests.

### Changed

- The Ratatui foreground dashboard now uses a centered monochrome Codex wordmark and black/white panel treatment instead of the previous neon accent palette.
- The README now follows the Pulse-style launch structure with What's New, About, Screenshots, Features, Usage, Roadmap, Security, and brand-true Shields/Simple Icons badges.

### Fixed

- Windows WSL session discovery is now explicit opt-in via `CODEX_PRESENCE_INCLUDE_WSL=1` or `CC_PRESENCE_INCLUDE_WSL=1`; by default the runtime does not invoke `wsl.exe`, and the opt-in path uses the hidden subprocess helper to avoid visible console windows.
- The terminal plan selector now distinguishes `Pro 5x ($100/month)` from `Pro 20x ($200/month)`, with legacy `pro` config/cache values mapped to `Pro 20x`.

## [1.7.0] - 2026-07-05

Codex Rich Presence now has the full Codex App-quality runtime: sticky desktop identity, accurate GPT-5.4/GPT-5.5 context and Fast-mode economics, a Ratatui terminal dashboard, and polished local-first docs/assets. No public configuration key was removed.

### Added

- Ratatui dashboard with Codex dark styling, responsive layouts, animated status widgets, cache/context/cost panels, recent-session cards, and optional Codex App logo rendering.
- Unified Codex usage snapshot for input, cached input, output, context-window utilization, cache hit ratio, cached-input savings, and service-tier adjusted cost.
- GPT-5.4 and GPT-5.5 metadata for OAuth and API contexts: 400K visible OAuth cap, API-only 1,050,000-token metadata, 272K input threshold, 128K output reserve, and Fast multipliers of 2x / 2.5x.
- Codex App process detection for the official desktop runtime, plus README hero/screenshot assets that match the Codex visual language.

### Changed

- Discord payload construction now uses one surface/branding policy across active and idle states instead of recomputing generic CLI defaults.
- Cost, cache, context, model, surface, and terminal display strings now flow from centralized owners instead of duplicated local fallbacks.
- Documentation and README were refreshed around install, privacy, model economics, identity, UI, schema, and test coverage.

### Fixed

- Idle no longer drops a Codex App session back to the generic Codex CLI / VS Code identity. If the last detected surface was Codex App, Discord keeps the `Codex App` app identity and shows `Idling...`.
- Desktop/OpenCode collection keeps recent Codex activity visible for the intended sticky window instead of aging to idle after a near-instant gap.
- Cache savings and context display now use the same accounting summary in terminal, Discord, status, and tests.

## [1.6.0] - 2026-06-08

Codex App parity for OpenCode is here. The runtime now reads OpenCode's local SQLite state, publishes it through the official `Codex App` Discord identity, shows GPT-5.5 Fast labels correctly, and reports the active context window from OpenCode's latest step instead of the lifetime session total.

### Added

- OpenCode session collection from `~/.local/share/opencode/opencode*.db`, filtered to the current workspace and mapped into the same model, token, cost, activity, and context fields used by Codex JSONL sessions.
- Official `Codex App` Discord identity support for desktop/OpenCode sessions with client id `1478395304624652345` and the `codex-app` asset.
- Generic `gpt-*-fast` display support, so models like `gpt-5.5-fast` render as `⚡ GPT-5.5` while pricing and context-window lookup use the base model.
- Priority presence republishing every two seconds, keeping Codex above browser/PreMiD Discord activities while preserving the original session timer.
- Regression coverage for OpenCode activity parsing, OpenCode context-window parsing, Codex App surface fallback, GPT-5.5 Fast labels, and Codex-only identity migration.
- Project maps for docs and tests through `docs/index.md` and `tests/index.md`, plus a Taskfile mirror for the core quality commands.

### Changed

- Discord context copy now shows used context (`Ctx 19% used`) instead of remaining context, matching the way OpenCode surfaces its context bar.
- `status` now reports the runtime surface and prints context as `used / window` with the used percentage.
- README and API docs now describe CLI, VS Code, Codex App, and OpenCode host behavior under the Codex-only identity policy.

### Fixed

- Long OpenCode sessions no longer show impossible context values such as multi-million tokens over a 400K window; the parser uses the latest `step-finish.tokens` snapshot and hides context when no reliable active-window value exists.
- Persisted non-Codex Discord client IDs and assets are rewritten back to the approved Codex identities before publish.
- GPT-5.5 and GPT-5.5 Pro pricing, context-window, and Fast-mode labels resolve consistently across standard and Fast variants.
- Transitive `rand` and `rustls-webpki` lockfile entries are updated to patched versions, resolving the two stale Dependabot PRs without leaving red checks behind.

## [1.5.0] - 2026-05-21

GPT-5.5 and GPT-5.5 Pro pricing and Fast labels, centralized pricing constants and refreshed README visuals. The recorded test suite had 114 passing tests.

### Added

- Official pricing catalog entries for `gpt-5.5` ($5 / $0.50 / $30 per 1M) and `gpt-5.5-pro` ($30 / $3 / $180 per 1M), so Codex sessions on GPT-5.5 resolve to the correct OpenAI pricing the moment the model is selected.
- Lightning `⚡` Fast-mode prefix coverage is now confirmed for the entire GPT-5.5 family (inherited from the existing service-tier resolver; no new wiring required).
- Unit tests covering `gpt-5.5`, `gpt-5.5-pro`, case-and-trim normalization on the new keys, and context-window resolution for the GPT-5.5 family. Test suite total now sits at 114 passing.
- README highlight banner calling out GPT-5.5 + GPT-5.5 Pro support, including pricing chips, rendered identically on github.com and in local Markdown previews.

### Changed

- Pricing rows in `src/cost.rs` are now centralized as named `ModelPricing` constants (`GPT_5_5`, `GPT_5_5_PRO`, `GPT_5_4`, `GPT_5_2_FAMILY`, `GPT_5_1_FAMILY`, `GPT_5_MINI_FAMILY`, `GPT_5_NANO`, `CODEX_MINI_LATEST`); the Codex session context window is centralized as `CODEX_CONTEXT_WINDOW: u64 = 400_000`.
- README badge row upgraded to `for-the-badge` style with real brand logos (GitHub Actions, semver, Rust, Discord, OpenAI), with the same visual language across github.com and Markdown previews.
- README model-label examples and Fast-mode visibility examples refer to GPT-5.5.

### Fixed

- `clippy::collapsible_match` warning in `src/session/activity.rs` (surfaced on Rust 1.95.0 stable) is collapsed into a single match guard so `cargo clippy -- -D warnings` stays green on the current toolchain.
- `.gitignore` now excludes injected local assistant boilerplate that previously surfaced as untracked.

## [1.1.0] - 2026-03-07

A full-screen plan selector with Fast mode, reasoning effort and GPT-5.4 pricing support, plus more reliable session selection and incremental parsing.

### Added

- Full-screen TUI plan selector with instant save support for `Auto Detect`, `Free`, `Go`, `Plus`, `Pro`, `Business`, and `Enterprise`.
- Fast-mode visibility derived from Codex global state, including the lightning-prefixed model label.
- Reasoning-effort visibility across TUI, status output, and Discord presence.
- Official pricing catalog entries for `gpt-5.4`, `gpt-5.4-2026-03-05`, `gpt-5.3-codex`, and `gpt-5.3-codex-latest`.

### Changed

- Release outputs are standardized under `releases/<platform>/`, with Cargo cache kept under `.build/target`.
- Active-session selection is recency-first and uses deterministic tie-breakers based on pending calls, activity priority, and session ID.
- Discord and TUI model labels now compose model, Fast mode, reasoning effort, and resolved account plan more cleanly.
- Session parsing internals are now split into focused activity and parser modules for easier maintenance.
- README, schema docs, API contract, and UI docs were refreshed to reflect plan selection, Fast mode, reasoning visibility, and current build/release layout.

### Fixed

- Idle transitions no longer fabricate recency or prematurely hide active sessions.
- Commentary and tool-call activity tracking now preserves working-state visibility more accurately.
- Incremental JSONL parsing safely retains partial lines across updates.
- Legacy `gpt-5.3-codex -> gpt-5.2-codex` pricing aliasing is migrated away; Spark variants now map to `gpt-5.3-codex`.

## [1.0.0] - 2026-03-04

README and branding refresh with semantic-version release tags and clearer cross-platform assets and changelog deltas.

### Changed

- Redesign README visuals and branding assets.
- Switch releases to semantic-version tags.
- Clarify cross-platform release assets and changelog deltas.

## [0.2.2] - 2026-02-10

A focused packaging release with archive-only distribution and checksum files for easier automated verification.

### Changed

- Release publishing now includes only packaged archives (`.tar.gz` / `.zip`) and `.sha256` checksum files.

## [0.2.1] - 2026-02-10

A compatibility release for CI and release stability, moving the macOS Intel runner to `macos-15-intel`.

### Changed

- Release workflow macOS x64 runner updated to `macos-15-intel` for current GitHub Actions compatibility.

## [0.2.0] - 2026-02-10

A major UX and runtime foundation update with incremental session parsing, live activity, clearer telemetry and configurable terminal logos.

### Added

- Incremental session parsing cache:
  - per-file cursor tracking
  - appended-line parsing only
  - cached snapshot reuse for unchanged files
- New spec-kit plan:
  - `plans/20260210-ux-intuitive-realtime-github-bootstrap/plan.md`
- Branding assets:
  - `assets/branding/project-mark.svg`
  - `assets/branding/social-card.svg`
  - `assets/branding/README.md`
- Release note categorization config:
  - `.github/release.yml`
- Config schema v3:
  - `privacy.show_activity`
  - `privacy.show_activity_target`
  - `display.terminal_logo_mode`
  - `display.terminal_logo_path`
- Session activity extraction and snapshot model:
  - `SessionActivityKind`
  - `SessionActivitySnapshot`
  - `CodexSessionSnapshot.activity`
- Terminal footer author credit:
  - `By XT0N1.T3CH | Discord @XT0N1.T3CH | ID 211189703641268224`

### Changed

- Discord details now prioritize live activity (`Thinking`, `Reading`, `Editing`, etc.).
- Discord state copy now uses natural labels:
  - `Last response`
  - `Session total`
  - `This update`
  - `5h left`
  - `7d left`
- TUI/status token copy now uses human-readable `Tokens: ...` format.
- Smart foreground TUI now shows:
  - live activity line,
  - semantic colorized remaining-limit bars (green/yellow/red thresholds),
  - hybrid official-logo rendering with ASCII fallback.
- `status` output now includes activity summary when enabled.
- Default polling interval updated to `2s` for balanced real-time behavior.
- CI workflow now includes Rust dependency caching for faster runs.
- Release workflow now uses `.build/target` to match Cargo target-dir config.

## [0.1.2] - 2026-02-09

Session token metrics, remaining-limit semantics and an error-aware Discord update pipeline, with a redesigned terminal layout.

### Added

- Session token metrics from Codex JSONL:
  - `session_delta_tokens`
  - `last_turn_tokens`
  - `session_total_tokens`
  - `last_token_event_at`
- Global limits-source selection based on most recent active session token event.
- `.cargo/config.toml` to route build output to `.build/target`.
- New spec-kit plan:
  - `plans/20260209-codex-discord-presence-v1-2-usage-parity-ui/plan.md`

### Changed

- 5h/7d semantics now match Codex CLI display:
  - show **remaining** percent, not used percent.
- Discord state formatting now prioritizes model + real token triplet + remaining limits with deterministic truncation.
- Discord update pipeline is now error-aware (no silent update failures).
- TUI layout redesigned with cleaner runtime/session sections and remaining-limit bars.
- `status` command now reports token triplet and remaining-limit snapshot.
- Release workflow now produces grouped artifact structure by platform/arch under `dist/<os>/<arch>`.

## [0.1.1] - 2026-02-09

Configuration schema 2, project branding and authoritative single-instance takeover, with terminal relaunch before headless fallback.

### Added

- Config schema v2 with project defaults:
  - `discord_client_id = 1470480085453770854`
  - `discord_public_key` metadata
  - default asset keys `codex-logo` / `openai`
- ASCII hero banner in TUI (OpenAI-style icon + `CODEX` wordmark).
- Instance metadata file `~/.codex/codex-discord-presence.instance.json`.

### Changed

- Smart mode and `codex` wrapper now use automatic takeover:
  - new launch terminates older running instance and acquires lock.
- `status` running detection now reports authoritative lock state.
- No-TTY startup now attempts terminal relaunch before headless fallback.
- Discord missing-client-id status text standardized to `Missing CODEX_DISCORD_CLIENT_ID`.

## [0.1.0] - 2026-02-09

The initial single-binary Rust runtime with smart foreground, Codex wrapper, status and doctor commands, local session parsing and cross-platform Discord IPC.

### Added

- Initial Rust implementation with single-binary UX.
- Commands:
  - default smart foreground mode
  - `codex` child wrapper mode
  - `status`
  - `doctor`
- Cross-platform Discord IPC integration.
- Codex session JSONL parser with tolerance for missing fields.
- Live usage windows (5h/7d) and token display.
- Single-instance lock handling.
- Open source docs and CI/release workflows.

[Unreleased]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.15.0...HEAD
[1.15.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.11.3...v1.15.0
[1.11.3]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.11.2...v1.11.3
[1.11.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.11.1...v1.11.2
[1.11.1]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.11.0...v1.11.1
[1.11.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.10.3...v1.11.0
[1.10.3]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.10.2...v1.10.3
[1.10.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.10.1...v1.10.2
[1.10.1]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.10.0...v1.10.1
[1.10.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.9.0...v1.10.0
[1.9.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.8.0...v1.9.0
[1.8.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.6...v1.8.0
[1.7.6]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.5...v1.7.6
[1.7.5]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.4...v1.7.5
[1.7.4]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.3...v1.7.4
[1.7.3]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.2...v1.7.3
[1.7.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.1...v1.7.2
[1.7.1]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.1.0...v1.5.0
[1.1.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v0.2.2...v1.0.0
[0.2.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/tag/v0.2.0
[0.1.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/commit/a4c6d9f6206a9dcbe3a941d27ce6aa08beeaf9bb
[0.1.1]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/commit/a4c6d9f6206a9dcbe3a941d27ce6aa08beeaf9bb
[0.1.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/commit/a4c6d9f6206a9dcbe3a941d27ce6aa08beeaf9bb
