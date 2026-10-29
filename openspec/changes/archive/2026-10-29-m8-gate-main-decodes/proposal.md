## Why
Planner (2026-10-28): no main-thread call decodes a bake block (VIEW-061, #858). The gate counted frames over 100 ms but not why; the stall probe now counts the blocks the main thread decodes per phase, and the gate must fail on them so the rule cannot come back.

## What Changes
1. `Gate::frame` takes the phase decodes (`StallProbe::main_decodes`); a slow frame says how many blocks it decoded; a run that decoded any fails ("bake blocks decoded on the main thread (VIEW-061): N in M frames; first in frame F in phase P"), judged where frame time is judged (not on the software adapter); the report line carries the count.
2. `cargo xtask gate` lists the failure as known up to a ceiling of 6 per run (Tracked: "Gate main-thread decodes"): 8 runs at frame-102263 gave 4, 1, 2, 1, 2, 2, 2, 2, each run's first in "camera, ground heights" at frame 25 (the replay's first pose).

## Impact
- Spec: TEST-061 (modified). Code: `app/gate.rs`, `app/window.rs`, `xtask/src/gate.rs`. Tests: `gate` (TEST-061), `xtask gate` (TEST-063).
