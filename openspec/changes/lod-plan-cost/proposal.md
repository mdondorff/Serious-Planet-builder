# Change: lod-plan-cost

## Why
- Perf window 22 (docs/perf/window-22.md): the plan job under ADR 0074 (#505) is over the +10 % bound: dive plan median 2.7 ms against 1.2 ms. Cause: the selection walks and balances the position-only selection and judges every tile of it, on every real field (the global height bound makes every fine level read its neighbours' bounds).
- opus-planner (2026-10-21) approved an opt-in monotone-bounds contract so that the common case, ground where nothing is steep, does no ceiling work.

## What changes
- `HeightField::bounds_monotone` (default false) and `MONOTONE_SLACK_M`: the bounds of a cap inside another lie inside the other's (slack 0.01 m). True for the reference, terrain and eroded fields, the water wrapper and the other forwarding wrappers over such a field, and the stamp stage over a base that says true (its bounds are monotone in the base range, the weights and the item set).
- Selection (`lod.rs`): where the field says true, `none_steep` proves from the cached bounds of the tiles round the camera (one cap per tile, adaptively halved, per level that reads bounds) that no tile of the position-only selection can be too steep; then the ceiling is empty and the position-only walk, its balance and the judging of its tiles are skipped. Otherwise (not proved, or a field that does not say true) the full path runs unchanged, with `certified` skipping the judging of the subtrees whose covering cap leaves room at every level below.
- `SelectionCache` holds the tile caps and the certificates for one set of parameters and one item revision (an edit starts them over).
- Not changed: the selection, the ceiling, the cover and the plan are what they were (the tests compare the two paths node for node); no pixel moves.

## Tests
- `bounds_monotone.rs` (generators, tier A): random nested caps over every field that says true, 12000 pairs per field, including the stamp stage over each base with hills, craters, plateaus, a ridge, a channel and a brush of all four stroke kinds; the default is no, and a stage over a field that does not say so does not.
- `lod_prune.rs` (frame, tier A): over 90 random spike fields and cameras the selection equals the full path node for node and ceiling for ceiling (steep and gentle cases both present); over gentle ground the cover visits fewer tiles than the full path.
- `lod_capping.rs` now runs its spike field with `bounds_monotone` true.

Expected golden changes: none.
