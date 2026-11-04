## Why
- Owner on SPIKE-67's stills (2026-11-04): the ground detail is uniform and very low frequency, variety is missing. SPIKE-100 (closed 2026-11-04, `spikes/ground-detail-m9/results/`) ranked a design; its prototype, two patch octaves that shift the slope input of the material weights, broke the rock/grass boundary on steep slopes into irregular patches (the direction of the realism reference s7).
- Inbox #57 (I8a), planner's slicing (2026-11-04): slice 1 is the patch break-up and the steepness growth of the rock's tilt, behind a switch, on the four materials that exist, no channels; slice 2 (five materials and the baked channels, after explorer's read API) and slice 3 (geometry below 100 m) follow as their own changes.

## What Changes
- The lit terrain shader (REND-055): with `g.detail.z` above 0, two gradient-noise octaves (32 m and 8 m cells, world-anchored like the detail's) shift the slope cosine that feeds the material weights by up to that share of it; they fade at twice the detail octaves' footprint. The 2 m and 25 cm rock tilt grows with steepness (up to 3x and 2x on slopes past about 60 degrees).
- The switch is the look file's `ground.patch` (LOOK-012, schema version 6, default 0 = off), carried by `LitPlan::ground_patch` into the plan's key and text and into the globals' spare lane, and by the stills poses file's `[look] ground_patch`.
- Off, every picture, key and plan text is what it was; the look file schema is 6.

## Impact
- Code: `frame` (`look.rs`, `look/file.rs`), `render` (`terrain.rs` shader and globals), `app` (`stills.rs`). Requirements: REND-055, LOOK-012 added; LOOK-002, TEST-053, VIEW-060 modified.
- GPU time of the lit pass off vs on, and stills before and after on the steep and flat sites: in the PR (indicative; no perf window).
