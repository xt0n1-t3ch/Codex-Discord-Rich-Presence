## Summary

<!-- State the user-visible problem, the owner changed, and why this is the smallest sufficient mechanism. -->

## Contract and compatibility

- Config schema impact:
- `codex-presence-core` API/schema impact:
- Platform impact (Windows/macOS/Linux):
- Privacy or data-exposure impact:

## Validation

- [ ] `cargo --locked fmt --check`
- [ ] `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo --locked test --workspace --all-features`
- [ ] `cargo --locked build --workspace --release --all-features`
- [ ] Config migration is covered by a fixture when the schema changes
- [ ] `status` and `doctor` were exercised on the affected platform
- [ ] Discord/TUI runtime behavior was observed, not inferred from a unit test

## Runtime proof

<!-- Attach redacted terminal/Discord evidence. For telemetry changes, include source, freshness, scope, and absence behavior. -->

## Performance

<!-- Record before/after startup, idle CPU/memory, parser reads, or state publications when affected. Write “not affected” with a reason otherwise. -->

## Risks

## Backout plan

## Release notes

- [ ] `CHANGELOG.md` updated for user-visible behavior
- [ ] Commit messages follow Conventional Commits
