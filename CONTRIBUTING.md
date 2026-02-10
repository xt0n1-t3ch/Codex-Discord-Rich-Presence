# Contributing

Thanks for contributing to `codex-discord-presence`.

## Development Workflow

1. Fork and create a feature branch.
2. Keep changes focused and production-oriented.
3. Add or update tests for behavior changes.
4. Run validation locally:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```

5. Open a PR with:
- problem statement
- implementation notes
- test evidence
- risk/backout notes

## Code Quality Expectations

- Keep modules cohesive and dependency boundaries clear.
- Prefer explicit error handling with actionable messages.
- Keep behavior deterministic and cross-platform.
- Preserve single-binary UX.

## PR Checklist

- [ ] No unrelated refactors bundled.
- [ ] Docs updated if behavior changed.
- [ ] CI green.
- [ ] No secrets or credentials in committed files.
