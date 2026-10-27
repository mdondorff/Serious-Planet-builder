## Why
The owner's live test of 2026-10-27 (build a7e739b5, `docs/milestones/M8/owner-test-2026-10-27/README.md`) found two bugs the tests could not see:
- No horizon map was baked in an hour of the editor over the production store (36 of 37 stall reports: `baked: 0, landed: 0`). Since #797 the window's chunk step switched the horizon jobs on with `frame.pipeline.draw.plan.lit.is_some()`; the lit plan is put on a copy of the draw plan later in the frame, so the field is always empty there. A plateau's shadow never reached a crater beside it.
- The K views (the coupled run's heights and drainage area) panicked on the pool workers at the mesh key (`cache/src/key.rs:354`): the lens stops the key walk at itself, but its reads were the top stage's, which says it reads cells whatever the tile while its records carry a cell query only when the tile has items or chunk reads.

Neither was visible because the window's smoke run never starts the chunk stage and a panic on a pool worker does not end the process.

## What Changes
- The window runs the horizon jobs on lit frames (`FrameFeatures { horizon: st.viewer.lit }`).
- The window counts the chunk step like the scenes (TEST-058): the smoke summary prints `chunk step:`, and the smoke verdict fails a run that drew lit over a chunk stage whose step never ran the horizon jobs. `PLANET_SMOKE_CHUNKS` starts the chunk stage in a smoke run; `PLANET_SMOKE_LENS` presses K once in it.
- `HeightField::reads_cells_for(tile, map)` (default: `reads_cells`); the lens answers it and its read records for the layer the key walk reaches below it; `field_inputs` asks it.
- The window smoke tests fail on a panic in stderr (a caught pool-worker panic left the run green).
- STRM-057 needed no change: the edit word of a chunk already hashes the items within its march halo, which reaches the neighbours; a test at the real chunk size pins it.

## Impact
- Spec: TEST-058 (the window's row and scenario; archived with `m8-shared-frame-loop`), GEN-043 (the lens's reads).
- Code: `app/window.rs`, `frame/viewer.rs`, `generators/{height,chain}.rs`, `cache/key.rs`; tests `gpu_window.rs`, `anet_world.rs`, `horizon_world.rs`, `viewer.rs`.
