## Why
- Crusader's #124 diagnosis (2026-11-13): the F9 bundle does not record `--commit` or `--site`, and nothing replays a camera path in the window, so the F1 stalls (render submit 4 to 6.5 s) could not be re-flown or counted.
- The gate already replays a path in a window in real time (`PLANET_GATE_PATH`, TEST-061) but leaves the GPU phases out of its frame time and cannot open a commit.

## What changes
- VIEW-071: the bundle's head records `open_commit` and `open_site` of the run.
- BUILD-054: `cargo xtask repro <path> --window [--real] [-- OPTIONS]` runs the editor window over the recorded world with the gate's path replay; the gate line gains a stall count (frames of 1 s or more with the GPU phases) that does not fail the gate.

## Not in this change
- The gate's limits and its known list (the stall count is reported, not judged); the resident cap (VIEW-070, its own change).

## Impact
frame (terrain_text.rs), app (window.rs, gate.rs, skeleton.rs), xtask (windowrepro.rs, testcmd.rs). Specs: viewer, build-tooling.
