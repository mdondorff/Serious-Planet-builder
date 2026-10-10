## Why

M2 is code-complete. Acceptance needs a milestone report with an explicit go/no-go recommendation, the M2 criteria list in `cargo xtask accept`, and measured numbers for the gate (tile-area distortion of the tangent warp, ADR 0003).

## What Changes

- `xtask accept M2` lists the criteria that need the owner or Rasierklinge.
- A test measures the tile-area ratio of the tangent warp (about 1.40 at level 6, matching the published 1.41).
- `docs/milestones/M2-report.md`; status updated.

## Capabilities

None (no spec-level behaviour change; `--skip-specs` on archive).

## Impact

`xtask`, one test in `core`, docs.
