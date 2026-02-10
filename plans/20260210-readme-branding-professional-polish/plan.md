# Plan: README + Branding Professional Polish

## 1) Discovery & Context

- Current README card and wording are visually inconsistent with desired black/professional aesthetic.
- `README.md` still contains legacy informal wording and verbose sections.
- `assets/branding/social-card.svg` and `assets/branding/project-mark.svg` use older gradient style that does not match requested look.
- GitHub repository metadata description still uses legacy wording.

## 2) Scope & Non-Goals

### Scope

- Redesign README hero card to dark/black professional style with OpenAI-style mark.
- Remove legacy informal wording in docs-facing surfaces.
- Make README concise with direct install/use/config instructions.
- Keep badges, credits, and brand notice consistent.
- Update GitHub About description to match new tone.

### Non-Goals

- No runtime parser/tracker logic changes in this pass.
- No CLI/API behavior changes.
- No CI workflow refactor unless required by README/branding updates.

## 3) Architecture & Data Flow

- Documentation-only flow:
  1. Replace static branding SVG assets.
  2. Rewrite README information hierarchy (overview -> install -> usage -> config).
  3. Keep technical references pointing to existing docs under `docs/`.
  4. Align GitHub metadata text with README top-level positioning.

## 4) Interfaces & Schemas

- Public CLI/API interfaces: unchanged.
- Config schema version: unchanged (`3`).
- Public docs surfaces changed:
  - `README.md`
  - `assets/branding/social-card.svg`
  - `assets/branding/project-mark.svg`
  - `assets/branding/README.md`

## 5) Implementation Phases

1. Rewrite README copy to concise professional structure.
2. Replace social preview SVG with black-background professional card.
3. Replace project mark SVG with matching icon treatment.
4. Update branding note wording for trademark clarity.
5. Update repository About description via GitHub CLI.
6. Add changelog entry under `Unreleased`.

## 6) Validation & Acceptance

### Validation

- Search check for removed wording:
  - no legacy informal wording in README/branding.
- Markdown render sanity:
  - README headings and badges display correctly in GitHub preview.
- SVG sanity:
  - both SVG files are valid XML and render in browser/GitHub.

### Acceptance Criteria

- Hero card appears black/professional with requested visual direction.
- README is concise and operationally clear (build, run, config, release).
- Credits remain present and non-invasive.
- GitHub description reflects professional tone.

## 7) Rollout, Risks & Backout

### Rollout

- Commit docs/branding changes to `main`.
- Push to origin and verify README render on GitHub.

### Risks

- Trademark usage interpretation can vary by jurisdiction/platform policy.
- SVG rendering differences across viewers.

### Mitigation

- Keep explicit OpenAI trademark/brand-guideline note.
- Use straightforward SVG elements compatible with GitHub rendering.

### Backout

- Revert the branding commit if visual direction is not approved.
- Restore previous SVG assets and README copy from prior tag/commit.
