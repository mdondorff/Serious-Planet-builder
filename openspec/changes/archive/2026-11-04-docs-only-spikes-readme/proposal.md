## Why
- The spike catalogue `spikes/README.md` (owner, 2026-11-04, #1152) gets a row at every spike close; it sits under `spikes/`, outside the docs-only paths, so each of those PRs ran the full CI.

## What Changes
- `spikes/README.md` joins the docs-only paths: `paths-ignore` of `ci.yml` (both triggers), `paths` of `docs.yml` (both), the expected list of the repository check test (`xtask/src/repocheck.rs`), BUILD-005, and the docs-only list of CLAUDE.md. The existing test enforces that the lists agree.

## Impact
- A PR that touches only docs, openspec, `.claude`, root `*.md`, `spikes/**/results/**` or `spikes/README.md` runs the light `docs` workflow. This PR touches `ci.yml`, so it runs the full CI itself.
- Requirement: BUILD-005 modified (synced; archived with this change).
