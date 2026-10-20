# SPIKE-22 results: is the base LOD refused in the real eroded world? (base-lod-refusal)

Diagnosis only (planner brief, 2026-10-20; first called SPIKE-21, renamed after a clash with the wet-floor spike). Spike code: branch `spike/base-lod-refusal` (never merged). Machine: Rasierklinge. Question (crusader, #431): over a field that carries only a global height bound, the BASE LOD (viewport/tau 240, cells 32) is refused at level 12 below about 31 km in 5 to 9 % of poses. Is that real in the editor's world?

## Answer in one paragraph
**No. In the real eroded worlds it does not happen: 0 refusals in 33,000 random poses and 0 in 12,800 poses over the steepest tiles of every level.** The rule is real and the finding is correct for a field whose tiles have only the global bound (`HeightField::height_bounds` default, +-9,735 m for every tile: a test stub, or any field without tile bounds): level 12 is the first level whose limit (extent 13.7 km) is under the global range (19.5 km). The eroded field's tile bounds are local, and the tile extents of the real world are 13 to 75 times under the limit at every level. Nothing to fix in the world; the risk that remains is a field or an authored item whose **local** range is steep (a near-vertical wall), which the plan refuses by design with the "too steep" message.

## The rule, per level (the base LOD of the window: cells 32, viewport/tau 240)
A level-L leaf is refused when `split(L) + 2 rho > 0.95 x split(L-1)` with `rho = extent(0.82 x edge(L) x (1 + H/R), half_range)` (`morph_interval_for`, `first_empty_morph_interval`), i.e. when the tile's height half-range exceeds `sqrt((0.225 x split(L-1))^2 - (0.82 x edge(L))^2)`:

| level | refused above (tile height extent) | steepest extent measured over 40,000 random tiles | factor |
|---|---|---|---|
| 8 | 219,341 m | 2,909 m | 75 |
| 10 | 54,835 m | 960 m | 57 |
| 12 | 13,709 m | 274 m | 50 |
| 14 | 3,427 m | 83 m | 41 |
| 16 | 857 m | 26 m | 33 |
| 18 | 214 m | 8 m | 27 |
| 20 | 54 m | 2 m | 27 |
| 22 | 13.4 m | 1 m (median 0) | 13 |
The global bound's extent is 19,470 m: under the limit at levels up to 11, over it from level 12 on. That is crusader's "level 12, below about 31 km" (`split(11)` = 31.7 km: the distance below which level-12 leaves exist).

## Method and evidence
1. `spike22_sweep`: the window's base LOD at 1280x720, 3,000 poses (random direction, altitude from 2 m to 30,000 km log-uniform, random yaw, pitch +-0.05 to 1.35 rad), the eroded field of the editor's world (seed 1, 256-cell bake): **0 refused** in every altitude band (0 to 100 m, to 1 km, 10 km, 31 km, 100 km, 1,000 km, above). The same at 6,000 poses for 1280x720, 1920x1080, 2560x1440, 320x200 (the ratio viewport/tau is 240 for every size: `tau = 6 x height / 1440`) and for seed 5: 0 of 33,000 poses in all.
2. `spike22_steep`: for each of the levels 8 to 22, 40,000 random tiles, the field's own `height_bounds` per tile, the 40 steepest, 7 to 8 poses each at 0.2, 0.5, 0.9, 1.5, 3 x `split(L-1)` and 8 headings (levels 14 to 22) or 100 m to 60 km (levels 8 to 12): 1,600 poses per level, **0 refused**.
3. The stub (`Bound9735`: zero height, global bound only) plans, at altitudes below 31 km, thousands of nodes per frame (every volume is +-9.7 km) and does not finish a few hundred poses in 8 minutes; crusader's refusal is the first check of such a plan.
- Stamps (ADR 0067) keep the bound local: `height_bounds` of the stamp stage adds the deltas the stamps can add **inside the cap of the tile**, not their declared maxima (`stamp.rs`, GEN-026), so a hill or crater widens the extent of the tiles it covers by its local relief (hundreds of metres at level 16 for kilometre-high features), well under the limits above. Not swept: authored items with near-vertical walls (a plateau with a sharp edge: local range = its full height at every level above its edge width: refused from level 14 for a 3.4 km wall); the plan then refuses on purpose.

## What to do
- Nothing in the world or the window for the base LOD. Keep the plan's check (it is the no-crack guarantee).
- The tests that use a field with only a global bound must not plan below level 11 (a test field needs tile bounds, or a larger `viewport_h_px / tau_px`): that is the status.md item of #431.
- If the editor ever offers steep stamps, the same two defences as SPIKE-19 apply: report the refusal as a message naming the item and the limit table above (not a dead window), and check the bound of a stamp against the limit when it is placed. Not a failing test today: no world, field or stamp the editor can produce today is refused.

## Files (spike branch)
`crates/app/examples/spike22_sweep.rs` (the pose sweep, `SP22_STUB`, `SP22_STORE`, `SP22_COARSE`), `crates/app/examples/spike22_steep.rs` (the steepest tiles and the theoretical limit per level).
