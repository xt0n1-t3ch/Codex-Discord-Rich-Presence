# Changelog

All notable changes to this project are documented in this file.

## [1.7.6] - 2026-07-10

### Fixed

- Windows background Git branch probes now use the shared `CREATE_NO_WINDOW` launcher, preventing console flashes during session polling.
- Process-lineage, takeover, task-list, relaunch-wrapper, and command-availability probes use the same silent launcher without changing interactive Codex or terminal children.

### Validated

- Source contract rejects raw background `git`, `powershell`, and `tasklist` launchers
- Windows runtime child-process trace across repeated polling cycles
- `cargo --locked fmt --check`
- `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo --locked test --workspace --all-features`
- `cargo --locked build --workspace --release --all-features`
- `cargo audit --deny warnings`

## [1.7.5] - 2026-07-10

### Added

- Schema 12 adds the durable `presence_enabled` master switch, defaulting to enabled for every existing schema-11 installation.
- Ratatui exposes `M` as a direct pause/resume shortcut and shows both the persisted switch state and truthful Discord `Paused` status.

### Fixed

- Foreground TUI, headless, and Codex-wrapper loops reload the shared persisted config on every poll, so Pulse changes to design and all nine privacy fields apply without restarting or terminating either process.
- Disabling presence clears Discord activity once per transition while local session monitoring continues; re-enabling invalidates the prior payload and republishes current state.
- Invalid, incomplete, missing, or transiently replaced config reads now log the failure and preserve the last valid runtime config instead of crashing a running loop.

### Validated

- Schema 11 to 12 migration and persisted enabled default
- External config reload plus invalid-replacement last-good behavior
- TUI, headless, and wrapper shared reload boundary
- Idempotent pause and resume-to-fresh-publish state transition
- Ratatui master shortcut, paused copy, and responsive footer
- `cargo --locked fmt --check`
- `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo --locked test --workspace --all-features`
- `cargo --locked build --workspace --release --all-features`
- `cargo audit --deny warnings`

## [1.7.4] - 2026-07-10

### Added

- Ratatui now has a persistent `V` privacy editor for project name, Git branch, model, activity, token count, cost, session limits, context usage, and system signals. Each option updates the live Discord payload immediately.
- `discord::active_presence_presentation` and `idle_presence_presentation` expose one public presentation contract for the daemon and immutable downstream consumers such as Pulse.

### Fixed

- Context visibility is independent from token visibility, and disabling Systems removes the Discord small activity icon and tooltip instead of leaving a hidden data path active.
- Discord activity construction now consumes the same public presentation object used by previews, preventing app title, branch, model/reasoning, asset, and privacy drift.

### Validated

- Privacy-field matrix against the final public payload
- Ratatui privacy editor render and keyboard contract
- `cargo --locked fmt --check`
- `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo --locked test --workspace --all-features`
- `cargo --locked build --workspace --release --all-features`
- `cargo audit --deny warnings`

## [1.7.3] - 2026-07-09

### Fixed

- Discord activities now send the selected surface label through the RPC `name` field, so the `ChatGPT App` design overrides the underlying application title instead of changing only the logo. The same owner supplies `Codex App`, `Codex CLI`, and `Codex VS Code Extension` titles.
- GPT model presentation now keeps the exact Codex App label in the factual catalog while rendering the human-facing line as `GPT-5.6 Sol · Max | Pro 20x ($200/month)`. Reasoning and Fast are separate ` · ` segments in Discord, Ratatui, status output, and OpenCode views.
- Presence payload equality includes the activity title, forcing an immediate republish when the selected design changes.

### Validated

- `cargo --locked fmt --check`
- `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo --locked test --workspace --all-features`
- `cargo --locked build --workspace --release --all-features`
- `cargo audit --deny warnings`
- Downloaded release assets and `SHA256SUMS.txt`

## [1.7.2] - 2026-07-09

### Added

- A machine-readable GPT-5.6 catalog for Sol, Terra, and Luna with App labels, aliases, supported efforts, Fast capability, 372K raw / 353.4K usable context, verified API rates, Codex credit rates, prompt-cache policy, and dated source metadata.
- Persistent schema-10 desktop design selection. Press `D` in Ratatui to switch between `Codex App` and `ChatGPT App`; the runtime reconnects through the corresponding Discord application id.
- `assets/branding/chatgpt-app.jpg`, copied byte-for-byte from the selected ChatGPT-style source asset for downstream Pulse previews.

### Changed

- Discord and Ratatui now show the chosen reasoning level and independent speed, including `5.6 Sol Max` and `5.6 Sol Max · Fast`.
- CLI, VS Code Extension, desktop, and OpenCode surfaces use exact labels. Active-session metadata wins; fallback detection now requires an extension-host or explicit environment marker instead of inferring VS Code/OpenCode from generic process names, terminal variables, or `PATH` entries.
- Cost output carries known subtotal, cache-write component, provenance, reconciliation, attribution, and `exact` / `partial` / `unavailable` status. Unknown models never inherit an older model's price.
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

### Validated

- `cargo --locked fmt --check`
- `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo --locked test --workspace --all-features`
- `cargo --locked build --workspace --release --all-features`
- `cargo audit --deny warnings`
- `pwsh -NoProfile -File tests/release_contract.tests.ps1`
- `pwsh -NoProfile -File tests/release_approval.tests.ps1`
- `pwsh -NoProfile -File tests/release_target.tests.ps1`
- `pwsh -NoProfile -File tests/release_assets.tests.ps1`
- `pwsh -NoProfile -File tests/release_workflow.tests.ps1`

## [1.7.1] - 2026-07-06

### Changed

- The Ratatui foreground dashboard now uses a centered monochrome Codex wordmark and black/white panel treatment instead of the previous neon accent palette.
- The README now follows the Pulse-style launch structure with What's New, About, Screenshots, Features, Usage, Roadmap, Security, and brand-true Shields/Simple Icons badges.

### Fixed

- Windows WSL session discovery is now explicit opt-in via `CODEX_PRESENCE_INCLUDE_WSL=1` or `CC_PRESENCE_INCLUDE_WSL=1`; by default the runtime does not invoke `wsl.exe`, and the opt-in path uses the hidden subprocess helper to avoid visible console windows.
- The terminal plan selector now distinguishes `Pro 5x ($100/month)` from `Pro 20x ($200/month)`, with legacy `pro` config/cache values mapped to `Pro 20x`.

### Validated

- `cargo test windows_wsl`
- `cargo test plan --lib`
- `cargo test ui::tests --lib`

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

### Added

- Official pricing catalog entries for `gpt-5.5` ($5 / $0.50 / $30 per 1M) and `gpt-5.5-pro` ($30 / $3 / $180 per 1M), so Codex sessions on GPT-5.5 resolve to the correct OpenAI pricing the moment the model is selected.
- Lightning `⚡` Fast-mode prefix coverage is now confirmed for the entire GPT-5.5 family (inherited from the existing service-tier resolver — no new wiring required).
- Unit tests covering `gpt-5.5`, `gpt-5.5-pro`, case-and-trim normalization on the new keys, and context-window resolution for the GPT-5.5 family. Test suite total now sits at 114 passing.
- README highlight banner calling out GPT-5.5 + GPT-5.5 Pro support, including pricing chips, rendered identically on github.com and in local Markdown previews.

### Changed

- Pricing rows in `src/cost.rs` are now centralized as named `ModelPricing` constants (`GPT_5_5`, `GPT_5_5_PRO`, `GPT_5_4`, `GPT_5_2_FAMILY`, `GPT_5_1_FAMILY`, `GPT_5_MINI_FAMILY`, `GPT_5_NANO`, `CODEX_MINI_LATEST`); the Codex session context window is centralized as `CODEX_CONTEXT_WINDOW: u64 = 400_000`.
- README badge row upgraded to `for-the-badge` style with real brand logos (GitHub Actions, semver, Rust, Discord, OpenAI) — same visual language across github.com and Markdown previews.
- README model-label examples and Fast-mode visibility examples refer to GPT-5.5.

### Fixed

- `clippy::collapsible_match` warning in `src/session/activity.rs` (surfaced on Rust 1.95.0 stable) is collapsed into a single match guard so `cargo clippy -- -D warnings` stays green on the current toolchain.
- `.gitignore` now excludes injected local assistant boilerplate that previously surfaced as untracked.

## [1.1.0] - 2026-03-07

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

## [0.2.2] - 2026-02-10

### Changed

- Release publishing now includes only packaged archives (`.tar.gz` / `.zip`) and `.sha256` checksum files.

## [0.2.1] - 2026-02-10

### Changed

- Release workflow macOS x64 runner updated to `macos-15-intel` for current GitHub Actions compatibility.

## [0.2.0] - 2026-02-10

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

[1.7.6]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.5...v1.7.6
[1.7.5]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.4...v1.7.5
[1.7.4]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.3...v1.7.4
[1.7.3]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.2...v1.7.3
[1.7.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.1...v1.7.2
[1.7.1]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.0...v1.7.1
