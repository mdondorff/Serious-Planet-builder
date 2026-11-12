# pre-breach (GEN-174, TEST-121)

OFF by default (architect, 2026-11-12): the default `PlanParams`, the bake profile and the examples do not set `pre_breach` (`PreBreach::PROFILE` stays defined); the production-path measurement made the 45-degree defect worse (see the spec text), a redesign follows. The measurements below are the start surface only.

The breach ran to the fixed point (cap 8 passes) in the profile, architect 2026-11-12 (now off); R3b: three passes of 0.45 s (share of land in basins over 30 cells 21.3 % to 4.9 % at three passes, 3.8 % at six).

## Why
- SPIKE-108 follow-up: the start surface holds large closed basins on 21 to 32 % of the land (R3b: 21.3 % in basins over 30 cells; the DEM 3.3 %); the fill turns each into a flat and the carve incises along the flat's routes.
- Architect, 2026-11-12: one constrained least-cost breach pass at round 0 (Lindsay); a basin that cannot be drained stays a lake.

## What
- `bake::breach::breach_basins` (planar grids): the rule of GEN-174; `PreBreach { passes, halo, max_depth_m, min_lift_m }`, `PreBreach::PROFILE` (to the fixed point, cap 8 passes, halo 128, 500 m, 0.5 m); `flood::lifted_flats` (the flats of a filled surface as cell lists, shared with the grade of GEN-173).
- `CarveParams::pre_breach` (default `None`), called by `carve_rock`, `solve_rounds_with` and `solve_rounds_memo` before the first round; layer key byte 0xC7 and its parameters only when set; the default `PlanParams` and the examples that run the round rules turn it on.
- The bound rule: path length at most `halo - 1 - diameter`, so basin and path lie inside the halo of every tile of its cells (as GEN-173); a basin touching the border ring stays a lake.

## Measured (R3b start surface, 1,020 x 1,020 cells; `examples/breach_share.rs`; after the review fixes: an exit must have a filled level below the floor, outlets are never entered)
- Before: 5,459 closed basins (lift >= 0.5 m); 988 over 30 cells hold 21.3 % of land (the DEM: 3.3 %).
- One pass (500 m): 5,352 drained, 30 kept as lakes (33,320 cells), 77 beside the window border; cut 19,583 cells; 0.9 s; after: 225 basins over 30 cells = 8.3 % of land.
- Passes 2 to 6: 5.8, 4.9, 4.5, 4.1, 3.8 % of land (46 basins over 30 cells after six). One pass leaves nested basins (a sub-pit the first path does not connect); `passes` is a parameter, the profile's value (1 or more) is the architect's choice.
- The depth limit hardly matters on R3b (20 m to 500 m keep 23 to 30 lakes).

## Declared moves
- R3b's final surface moves above the noise floor (intended); the plan and k=1 hashes of the examples change.
- The cap of the breach is per cell per round (architect, 2026-11-12): the lowering of a cell by all the round's breach events is at most its limit at the round start height (carve_rock and catchment_round, `carve::breach_limit`); `BAKE_KERNEL_VERSION` 6 to 7 and `PASS2_KERNEL_VERSION` 7 to 8 (the breach is in both); results change only where two events met a cell.
