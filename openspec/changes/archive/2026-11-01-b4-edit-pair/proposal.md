## Why
Option B item B4-4 (design PR #910; planner order 2026-10-30): the owner sees a dragged edit look different from the same edit at rest, everywhere (owner test 2). `gpu_preview.rs` (REND-033, tier C) already measures preview against exact tiles, but on a generated world in two views; nothing measures the pair on the real store, in the lit frame, or says whether the difference is shading, shape or shadow.

## What Changes (design only; no spec delta until the planner reserves a requirement ID)
1. A scripted pair scene over the real store (read-only): the same pose twice, (1) with the edit as the REND-033 preview over the stale tiles, (2) after the refresh, with exact tiles.
2. Sites and poses: one lowland and one hill site, 200 m and 2 km altitude, sun 20 degrees; each pair with the horizon jobs off, then on.
3. Per pair the report gives: the share of pixels changed by more than 3 grey levels against the bound of the existing test (the preview removes all but a twentieth of the pixels the edit changes), the mean and largest difference, and enlarged diff images in `target/review/`.
4. The cause class per pair: the same pair drawn in the normals view (shading), the relief or height-contour view (shape) and, with the horizon on, the shadow views, so the report says which view carries the difference.
5. The verdict is a table, never a skip: a pair that did not run is NOT RUN with its reason; a pair over the bound with the horizon off is a finding with its evidence and no fix in the same change.

## Impact
- Docs and OpenSpec only in this step. Later: `crates/app` scene (tier C, `--real`), a pure pair-comparison function in `planet_frame` or `planet_perf` with a planted-failure unit test (tier A), a line in the owner package (B4-5, TEST-074).
- The real runs need the real store on an unlocked, display-on machine and are not perf windows.
