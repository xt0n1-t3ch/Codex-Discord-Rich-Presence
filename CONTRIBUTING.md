# Contributing

Thanks for contributing to Codex Discord Rich Presence (`codex-discord-presence`).

## Development workflow

1. Fork and create a feature branch from `main`.
2. Keep changes focused and production-oriented.
3. Add or update the cheapest test that proves each behavior change.
4. Run `cargo xtask check` locally before proposing the change.
5. Open a PR with the problem, implementation, test evidence and risk/backout notes.

Checks run locally. There is no per-push or per-PR CI; the GitHub release workflow is manual-only.

| Task | Command |
| --- | --- |
| Full local gate | `cargo xtask check` |
| Format / check format | `cargo fmt --all` / `cargo fmt --all -- --check` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust tests | `cargo nextest run --workspace` |
| Release-tool tests | `cargo nextest run -p xtask` |
| Release compile | `cargo build --release` |
| Release artifacts | `cargo xtask build-release [--target <triple>]` |
| Release asset assembly | `cargo xtask release-assets` |
| Generate / check documentation maps | `cargo xtask llms` / `cargo xtask llms --check` |
| Check changelog / extract section | `cargo xtask changelog check` / `cargo xtask changelog notes <version>` |
| Generate release body | `cargo xtask release-notes <version>` |
| Check / apply repository metadata | `cargo xtask meta --check` / `cargo xtask meta --apply` |
| Diagnostics | `cargo run -- status` / `cargo run -- doctor` |

See the [documentation index](docs/index.md) and [test map](tests/index.md) for contracts and focused checks.

## Commit conventions

Use [Conventional Commits](https://www.conventionalcommits.org/) so intent is machine-readable and release notes remain reviewable:

| Prefix | Use for |
| --- | --- |
| `feat:` | New user-visible behavior |
| `fix:` | Correctness defect |
| `perf:` | Measured performance improvement |
| `refactor:` | Internal change with no behavior change |
| `docs:` | Documentation only |
| `test:` | Test-only change |
| `chore:` | Tooling, dependency or release maintenance |

Add `!` and a `BREAKING CHANGE:` footer only when a public CLI, configuration, core API or Discord output contract is intentionally incompatible. Keep the subject imperative and under 72 characters.

Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Write user-visible changes under the six [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) types: Added, Changed, Deprecated, Removed, Fixed and Security.

## Code quality expectations

- Keep modules cohesive and dependency boundaries clear.
- Prefer explicit error handling with actionable messages.
- Keep behavior deterministic and cross-platform.
- Preserve single-binary UX.
- Keep `codex-presence-core` UI-free and deterministic. The binary and downstream Pulse integration must not create a second telemetry owner.
- Keep unknown prices, limits and context values unavailable, never zero.
- Do not commit credentials, private session evidence, local settings or agent instruction files.

## PR checklist

- [ ] No unrelated refactors bundled.
- [ ] Tests prove changed behavior at the public seam when modules interact.
- [ ] Docs and changelog updated for user-visible changes.
- [ ] Local gate passes, with platform coverage and limitations recorded.
- [ ] No secrets, private data or local agent state in committed files.

## Release contract

Tags and release assets are immutable. Before an annotated `vX.Y.Z` tag is approved:

1. Synchronize the product and core version surfaces required by the release contract.
2. Add the matching changelog section with one summary paragraph and run `cargo xtask changelog check` and `cargo nextest run -p xtask`.
3. Prove configuration migrations, Windows runtime behavior and Linux/macOS builds on the appropriate platforms.
4. Generate and validate Windows SPDX software bills of materials and `SHA256SUMS.txt`.
5. Record the exact approved commit SHA. Never mutate an existing tag or published release asset.

Release titles and prose may be corrected without changing tags or assets. The title is `Codex Discord Rich Presence vX.Y.Z`; the body is the matching changelog summary and sections verbatim, without another title, asset list, integrity section or compare link. Only sections longer than 150 lines may append the asset, integrity and full-changelog material.

Read the [release procedure](docs/releasing.md) before promotion. Local development must stop before tag, push, PR or release creation unless the maintainer explicitly approves promotion.
