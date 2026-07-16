# Contributing

Thanks for contributing to Codex Discord Rich Presence (`codex-discord-presence`).

## Development Workflow

1. Fork and create a feature branch from `main`.
2. Keep changes focused and production-oriented.
3. Add or update tests for behavior changes.
4. Run validation locally:

```bash
cargo --locked fmt --check
cargo --locked clippy --workspace --all-targets --all-features -- -D warnings
cargo --locked test --workspace --all-features
cargo --locked build --workspace --release --all-features
```

5. Open a PR with:

- problem statement
- implementation notes
- test evidence
- risk/backout notes

## Commit Conventions

Use [Conventional Commits](https://www.conventionalcommits.org/) so intent is machine-readable and release notes remain reviewable:

| Prefix | Use for |
| --- | --- |
| `feat:` | New user-visible behavior |
| `fix:` | Correctness defect |
| `perf:` | Measured performance improvement |
| `refactor:` | Internal change with no behavior change |
| `docs:` | Documentation only |
| `test:` | Test-only change |
| `chore:` | Tooling, dependency, or release maintenance |

Add `!` and a `BREAKING CHANGE:` footer only when a public config, core API, or runtime contract is intentionally incompatible. Keep the subject imperative and under 72 characters.

## Code Quality Expectations

- Keep modules cohesive and dependency boundaries clear.
- Prefer explicit error handling with actionable messages.
- Keep behavior deterministic and cross-platform.
- Preserve single-binary UX.
- Keep `codex-presence-core` UI-free and deterministic. The binary and downstream Pulse integration must not create a second telemetry owner.

## PR Checklist

- [ ] No unrelated refactors bundled.
- [ ] Docs updated if behavior changed.
- [ ] CI green.
- [ ] No secrets or credentials in committed files.

## Release Contract

Releases are immutable and tag-only. Before an annotated `vX.Y.Z` tag is approved:

1. Synchronize the product version and `codex-presence-core` version surfaces declared in `scripts/release-contract.json`.
2. Add the matching `CHANGELOG.md` section and run every PowerShell release-contract suite.
3. Prove config migration fixtures, Windows runtime behavior, and Linux/macOS compile gates.
4. Generate and validate the Windows SPDX SBOM plus `SHA256SUMS.txt`.
5. Record the exact approved commit SHA. Never mutate an existing tag or release.

Local development must stop before tag, push, PR, or release creation unless the maintainer explicitly approves promotion.
