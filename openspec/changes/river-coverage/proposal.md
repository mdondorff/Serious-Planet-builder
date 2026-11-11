## Why
Item 15 PR 8 (river drawing), part 1 (architect, 2026-11-11, design section 18): the plan decides what is river water, as a coverage texture of signed distances so that the banks are smooth, and the drawn width is the burned width.

## What Changes
- `generators::bake::k7::trench_width_cells`: the burn's width function extracted (same value as before: the law's width capped at 32 cells); the burn calls it, the drawing takes `max(1 cell, it)`.
- `frame::river` (REND-059): `drawn_width_cells`, `river_distance` (the signed distance of the cell centres to the nearest edge, exact, window independent), `river_coverage` (the 8-bit codes, +-4 cells), `distance_code`.
- Tests (tier A, `crates/frame/tests/river_coverage.rs`): the width function equals the burn's with the minimum and the cap; exact distances and codes of a straight river; no staircase on a diagonal river; one-cell river and confluence; two windows give the same texels; the coverage agrees with the burn on which cells are river.

## Not in this change
- A river within 4 cells of a cube face edge: the stored channels of a neighbouring face are not in this face's plane (PR 7 limit), so the coverage of a node near the edge ignores them and jumps along the edge; a river crossing the edge has a notch. Reported as a Tracked item; the fix needs the cube-edge resolver of PR 7 on the reader side.
- The river plan node, the texture upload and the shader (REND-059's C verification), the water level of the river (REND-060), the draw scene and its goldens (TEST-117), the flight scene cache peak.

## Impact
- `frame::river` plan part (REND-059): `river_distance_grid` (any sample lattice), `node_window` (a terrain node's samples in face cell-index space), `RiverSource`, `plan_rivers` (nodes within a step limit and with a river in range; the whole channel is the dedupe key; errors name the tile), tests in `crates/frame/tests/river_plan.rs`.
- Code: `generators::bake::k7` (one function extracted, no value moves; the burn tests pass), `frame::{river, lib}`, tests. No key, record or golden moves.
- Requirement REND-059 added.
