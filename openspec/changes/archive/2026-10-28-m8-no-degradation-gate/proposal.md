## Why
Owner decision 2026-10-28 (`docs/milestones/M8/owner-test-2026-10-27/README.md`, Decisions 1 and 2): no change that touches generators, streaming, render or the editor may degrade what the owner saw on 2026-10-27 (37 stalls up to 14.8 s, no horizon map baked in an hour, 78-116 held-back tiles, a worker panic). Every scene so far ran a scene-only path; the owner's failures were all in the window path with the chunk stage on.

## What Changes
1. `app/gate.rs`: the gate counters (frame time with its longest phase, held-back tiles per frame with the edits in reach, horizon bakes, caught worker panics) and the verdict against the limits of Decision 2; GPU-free, tier A.
2. The window collects them when `PLANET_GATE` is set (a report file path), replays a camera path file (`PLANET_GATE_PATH`) and scripts one large edit (`PLANET_GATE_EDIT`); the smoke run fails on the verdict and prints the table.
3. `cargo xtask gate`: the real-store run (read-only store, outputs under `target/gate/`) over the capture sites of the owner test; `cargo xtask gate --small` the same counters on a generated world on the software adapter (CI, tier C).
4. `cargo xtask prepush` runs the small gate when the diff touches generators, streaming, render, shaders or the editor.
5. Known failures at landing are quarantined, one Tracked entry each in `docs/status.md`; no limit is weakened.

## Impact
- Spec: TEST-061 to TEST-063 (new). Code: `app/gate.rs`, `app/window.rs`, `app/editwin.rs`, `xtask/gate.rs`, `xtask/prepush.rs`. The chunk stage's behaviour is not touched.
