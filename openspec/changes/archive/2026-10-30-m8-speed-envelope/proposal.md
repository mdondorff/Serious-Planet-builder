# Change: m8-speed-envelope

## Why
- M8 exit criterion 7 (owner, 2026-10-23, D4): a perf scene flies level over unvisited ground at fixed altitudes and raises the speed until the stand-in share of the screen exceeds a bound; it reports the highest speed per altitude. M8 records the curve, the owner sets the floor from it, later milestones keep it as a regression check.
- Built on today's tile streaming; the chunk stage's stand-in share joins it when M8-2 step 3d exposes it.

## What changes
1. `planet_frame::scenes::detail_lag` also returns the coarse share of a frame (TEST-046): the pixel-weighted share of nodes drawn from a coarser tile or not covered.
2. `app::envelopeperf`: the speed search (doubling, then four bisections), the level-flight cameras, the worst 1 s window, the trial on the real streaming rig, the log `perf/logs/envelope.json` (TEST-047).
3. `cargo xtask perf envelope [--store DIR | --allow-memory-bake] [--share-bound F] [--smoke] [--out F] [--machine-ready]`.

## Tests
- Tier A: the search on a fake trial, the worst window, the flight cameras, the log text, the coarse share (frame).
- Tier B: the scene on the software adapter with `--smoke` over an in-memory bake.

Expected golden changes: none. No measurement is made by an agent: the window on Rasierklinge is the architect's.
