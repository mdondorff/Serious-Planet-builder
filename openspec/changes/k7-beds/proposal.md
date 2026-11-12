# k7-beds (GEN-176, TEST-124)

## Why
- G-PROFILE on production geometry (R1h tile (21, 2)) fails for two K7 defects, not the carry (72 rises over 0.25 m, the carry explains about 10): (1) land lines burnt to -0.5 to -26.5 m, the final pass skips cells at or under 0 m, so a sub-sea trench cell stands below its downstream land cell (71 of 72 rises have a sub-sea upper cell); (2) thermal, deposition and pit spreading filled burned one-cell trenches by 17-30 m before the final pass (breaking GEN-165's one law for burn and drawing). The reports undercount (cutoff_binds counted land path cells only; the burn's carry reported nothing).

## What
- `k7::burn_channels_report` (floor `sea_floor`, `BurnReport`); `thermal::thermal_fixed`; `deposit::deposit_on_own_route_fixed`, `deposit::spread_pits_fixed`; `carve::carve_fixed` / `carve_rock_fixed`; `k7_tile` fixes the channel cells after the burn (all four stages) and reports `burn_cutoff_binds`, `fixed_cells`, `thermal_exported_m3`, `deposit_exported_to_fixed_m3`; the final pass counts every path cell of the window on the cells' own heights.
- The incising carve is fixed too (it is a stage after the burn): without it the local carve differs between windows at line cells by hundreds of metres (229 m, 249 m, 0.001 m at face cell (191, 172) in tiles (2,2), (2,3), (3,2)), which thermal and deposition used to mask.

## Declared moves
- The plain-flood K7 digest with a channel (`bake_k7`); K7 with no channel is unmoved.
- `PASS2_KERNEL_VERSION` 8 to 9 (K7 is in pass 2); the stage manifest line of K7 moves (stage-reviewer report in the PR).
