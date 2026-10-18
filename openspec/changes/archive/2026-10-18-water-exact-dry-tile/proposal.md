## Why
After the horizon and loose height cuts (ATMO-020) the water pass still draws 400 to 700 nodes per frame that show no water: tiles over land that the coastal band and the coarse answers call wet. The loose bound `height_bounds` is a conservative lower bound; the tile's own mesh has an exact minimum (docs/perf/water-attribution.md, section "Terrain occlusion and the exact dry tile": 238 to 372 nodes per view dropped, 0 visible-water violations on the 16384 store).

## What Changes
- The tile's worker job keeps the lowest height of the tile's own `(cells + 1)^2` mesh vertices in the `WaterBook` beside a wet entry; the plan cuts a node whose minimum is at or above `W + A + 4 eps_f32 max(1, |W|, |m|)` (ATMO-020, ATMO-012).
- A tile with no minimum (a root, a tile not yet loaded, a cancelled job) keeps the loose bound's verdict. Nothing is computed on the plan thread.
- Counter `cut_exact` (recorded only).

## Impact
Spec: ATMO-020 modified (height test, definition of `m`, scenarios), ATMO-012 modified (the book holds the minimum). Plan data only: no shader, no `MESH_VERSION`, no stored data, no golden; the picture is pixel-identical. Refinement within ADR 0056, no ADR. No performance measurement in this change (the next perf window checks the water pass time).
