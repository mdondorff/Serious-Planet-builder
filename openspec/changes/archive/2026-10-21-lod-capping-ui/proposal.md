# Change: lod-capping-ui

## Why
- ADR 0074, change 3 of 3: the capped tiles are visible and said. The colour of capped tiles in the stale-tile overlay was reserved; the planner never set the state.

## What changes
- The planner sets `TileState::Capped` on a plan leaf that is capped (exact otherwise); the draw plan keeps it for an exact tile. The F3 stale-tile overlay shows it grey (legend: "held back, the ground is too steep to morph"); the tiles and mesh overlays draw a capped tile grey whatever its level (the compact fragment stage; both read the state of the node record).
- `CappedReporter`: the window prints one line in 5 s with the number of capped tiles of the drawn plan, nothing while none is, no crash report (VIEW-040).
- VIEW-044 and VIEW-040 (text); the sweep test of SPIKE-26 was already un-ignored (#505) and passes.

## Tests
- `lod_capping.rs`: the state of a plan node follows its capped flag. `pipeline.rs`: the reporter. `gpu_overlay.rs` (tier C): a capped tile has its own colour in tiles and mesh, a stale one keeps its level's, and nothing changes with the overlay off.

Expected golden changes: none (no golden node is capped; the shader branch is taken for state 4 only).
