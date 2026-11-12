# pre-breach (GEN-174, TEST-121)

## Why
- SPIKE-108 follow-up: the start surface holds large closed basins on 21 to 32 % of the land (R3b: 21.3 % in basins over 30 cells; the DEM 3.3 %); the fill turns each into a flat and the carve incises along the flat's routes.
- Architect, 2026-11-12: one constrained least-cost breach pass at round 0 (Lindsay); a basin that cannot be drained stays a lake.

## What
- `bake::breach::breach_basins` (planar grids): the rule of GEN-174; `PreBreach { passes, halo, max_depth_m, min_lift_m }`, `PreBreach::PROFILE` (1 pass, halo 128, 500 m, 0.5 m); `flood::lifted_flats` (the flats of a filled surface as cell lists, shared with the grade of GEN-173).
- `CarveParams::pre_breach` (default `None`), called by `carve_rock`, `solve_rounds_with` and `solve_rounds_memo` before the first round; layer key byte 0xC7 and its parameters only when set; the default `PlanParams` and the examples that run the round rules turn it on.
- The bound rule: path length at most `halo - 1 - diameter`, so basin and path lie inside the halo of every tile of its cells (as GEN-173); a basin touching the border ring stays a lake.

## Measured (R3b start surface, 1,020 x 1,020 cells; `examples/breach_share.rs`)
- Before: 5,459 closed basins (lift >= 0.5 m); 988 over 30 cells hold 21.3 % of land.
- One pass (500 m): 5,373 drained, 9 kept as lakes (30,393 cells), 77 beside the window border; cut 18,861 cells; 0.45 s; after: 202 basins over 30 cells = 10.6 % of land.
- Passes 2, 3: 9.1 %, 9.0 % (31 basins over 30 cells); pass 4 onward drains almost nothing. What remains: the border basins (the window cut) and the nine lakes.
- The depth limit hardly matters on R3b: 20 m keeps 23 lakes, 50 m 11, 100 m and 500 m nine.
- One pass leaves nested basins (a sub-pit the first path did not connect); `passes` is a parameter; the profile's value is the architect's choice.

## Declared moves
- R3b's final surface moves above the noise floor (intended); the plan and k=1 hashes of the examples change.
