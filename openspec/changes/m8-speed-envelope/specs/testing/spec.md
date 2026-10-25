## ADDED Requirements

### Requirement: TEST-046 The coarse share of a frame
`planet_frame::scenes::detail_lag` SHALL also give the coarse share of a frame: the share of the screen, weighted by the projected pixels of the nodes of the fresh selection ([`node_weight`]), whose node is drawn from a tile coarser than the selection wanted or is not covered at all. A node drawn at its own level, or covered by finer tiles, counts 0; a node drawn from an ancestor or uncovered counts its whole weight. The share is a number in `[0, 1]` (0 for a selection without weight) and a pure function of the fresh plan and the drawn set.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-V), M8 exit criterion 7

#### Scenario: The share
- **WHEN** a fresh plan of nodes with known weights is judged against drawn sets that draw every node, one node from its parent, finer tiles under one node, and nothing
- **THEN** the shares are 0, the weight of that node over the total, 0, and 1

### Requirement: TEST-047 The speed-envelope scene
`planet test-render --scene envelope` (`cargo xtask perf envelope`) SHALL, for each altitude of 30 m, 100 m, 300 m and 1 km, fly level over ground that is not in the cache (a start place of its own per trial, an empty tile cache) at a constant speed along a meridian, 8 degrees below the horizon, and find the highest speed whose worst window of 1 s of frames has a coarse share (TEST-046) of at most the bound (default 1 %, `--share-bound`). The search SHALL start at 10 m/s, double the speed while a trial passes up to 3,000 m/s, then halve the gap between the highest passing and the lowest failing speed four times; a trial that fails at the first speed reports no speed. The scene SHALL write `perf/logs/envelope.json` (or `--out`) with, per altitude, the highest speed, the worst window's share at it, the trials (speed and share), whether the cap was reached, the bound, the window, the frame step, the source and the adapter, and a one-line summary per altitude on the output. The scene SHALL also count the chunk stand-in share of the frames (STRM-039: the plan nodes at or finer than the pyramid top level whose own chunk is not ready, over those nodes, node-weighted) against the chunk field of its world, take its worst 1 s window like the coarse share, and judge the larger of the two; the log records both (`coarse_share`, pixel-weighted; `stand_in_share`, node-weighted), the chunk node counts (`chunk_nodes`, `chunk_stand_in`) and the two weightings in words. A world without a chunk field gives `stand_in_share` `null`, `chunk_field` false and the reason in the log; the app builds no chunk manager for a world yet, so the share stays `null` until one does. The scene runs the readiness gate and the reference-adapter check (ADR 0020), and is never run by an agent without the architect's window; `--smoke` runs one short trial per altitude on the software adapter and measures nothing.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-V), M8 exit criterion 7 (owner, 2026-10-23, D4)

#### Scenario: The search
- **WHEN** the search runs against a fake trial that passes below a limit speed and fails above it, against one that never fails, and against one that fails at the first speed
- **THEN** it returns a speed within one bisection step below the limit, the cap with the capped flag, and no speed, with the trials listed in the order they ran

#### Scenario: The worst window
- **WHEN** the shares of a series of frames are judged over windows of 1 s, with a spike shorter than the window, a spike of the window's length, and a series shorter than the window
- **THEN** the worst window's mean is the spike spread over the window, the spike's own value, and the mean of the whole series

#### Scenario: The flight and the log
- **WHEN** the cameras of a flight are built at an altitude and a speed from two start places
- **THEN** consecutive cameras are the speed times the frame step apart along the surface, each stands the altitude above the field, the two flights share no place, and the log text holds every altitude with its speed, share, trials and the bound

#### Scenario: The chunk stand-in share
- **WHEN** the measure of a flight is made from per-frame coarse shares and per-frame chunk shares, with and without a chunk field, and the log is written for both
- **THEN** the stand-in share is the worst window of the node fraction, the node counts are summed over the frames, the judged share is the larger of the two (NaN when either is missing), and the log without a chunk field says `null`, false and why, and names both weightings

#### Scenario: The scene on the software adapter
- **WHEN** the scene runs with `--smoke` over an in-memory bake
- **THEN** it ends without error, writes the log with four altitudes, and says it is not a measurement
