## Why
Item 15 step 2 (design notes `openspec/changes/runtime-composition/design.md` sections 3 and 8; architect D6): a basin kept by H1 is drawn as a lake at the level of the balance, and a river does not run as a line through it. H1 already solves the balance per basin (`Basin::water_level_m`, `q_out`, `overflows`); what is missing is the lake record and the river segments without the cells inside kept basins.

## What Changes
- `bake::lakes` (new): `lakes_from_hydro(grid, routing, hydro) -> Lakes`: one `Lake` per kept basin (level, class, overflow, wet or playa, surface area at the level, `q_in`, `q_out`), by id; the river segments as `RiverPiece`s with the channel cells inside kept basins dropped (a piece ends at the last cell before a basin, the next starts at the spill cell).
- `build_resident` runs the producer after H1 (phase "lakes") and returns `Built::lakes`.
- GEN-163 (generator-pipeline) and TEST-108 (testing) added. In memory only: the store field and the kind-13 version bump are the next PR (design notes section 8).
- Tests: `tests/bake_lakes` (five tests) and the phase-plan test in `tests/bake_compact`.

## Impact
- Code: `generators::bake::{lakes, build}`, tests as above. No change to H1, K7 or any stored bit; the stage manifest is unchanged.
- Open (numerics review, for the architect): "inside a lake" is H1's label (cells more than `pit_min_m` under the filled surface), not "under the lake surface". On a gently sloping shore, channel cells under the level but within `pit_min_m` of the fill stay in a piece; for an endorheic lake below its sill, labelled channel cells above the level are dropped, so a river stops at the basin rim and leaves a dry gap to the water. The test fixture has vertical walls and does not show it. A rule on `z < level` needs `z` as an input of the producer and a decision on H1's spill definition.
