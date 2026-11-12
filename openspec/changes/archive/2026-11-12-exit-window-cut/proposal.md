## Why
- Spikes' MFD comparison (SPIKE-107, 2026-11-11) on the R3b window (face 2, 12,288 cells, seed 15, margin 0): the round rules (`exit_base`) leave the land mean 3.8 m higher than the breach without it (|dz| p95 8.4 m, maximum 402 m); the incised valleys behind the window's rim are lost (3x3 relief p95 109.6 against 123.3 m).
- Stage measurement (explorer, 2026-11-12): the catchments that differ have their outlet on the window's border (e.g. window cells (626,0), (508,0), (278,0), (1019,305), (67,1019)); `exit_base` floors the whole receiver chain at that border cell's height, while the surface continues beyond the window at an unknown, lower level.
- Architect, 2026-11-12: a window cut is not a spill level; the outlet kind is set by the caller, not inferred from heights.

## What Changes
- `CarveParams::window_cut` (default off): the land outlets on the border of the grid are window cuts; `exit_base` gives their chains no base (the breach works as without the rule); an interior land outlet (a basin's rim) keeps its base (GEN-162).
- `Grid::window_cut(z, sea)`: the border cells above the sea level (none on a region bounded by the sea).
- `CatchmentInputs::exit_window_cut` and the catchment key (a word bit, only when set); the pass-1 layer key adds a byte only when the flag is on.
- The window tools that solve a window with land border cells (`rounds_region`, `region_scale`, `k7_region`, `edit_cases`) set the flag. The production build solves sea-bounded landmasses: no window-cut outlet, bit-identical.
- GEN-172 added (generator-pipeline); TEST-118 added (testing).

## Impact
- Code: `generators::bake::{carve, round, mod}`, `cache::layer_store` (key byte), four examples, `tests/bake_round`.
- No kernel version bump: a build without the flag (the default and the production path) is bit-identical; the keys of such builds are unchanged.
- Window gates (G-ROUND and the like) read an inner box at margin 200 (architect, 2026-11-12); spikes re-runs the UaCb and C rows after this change.
