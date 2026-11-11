## Why
Item 15 step 2 (design notes `openspec/changes/runtime-composition/design.md` sections 3 and 8; architect, 2026-11-10): a basin kept by H1 is drawn as a lake at the level of the balance, and a river does not run as a line through it. H1 already solves the balance per basin (`Basin::water_level_m`, `q_out`, `overflows`); what is missing is the lake record and the river segments without the cells inside kept basins.

## What Changes
- `bake::lakes` (new): `lakes_from_hydro(grid, routing, hydro) -> Lakes`: one `Lake` per kept basin (level, class, overflow, wet or playa, surface area at the level, `q_in`, `q_out`), by id; the river segments as `RiverPiece`s with the channel cells inside kept basins dropped (a piece ends at the last cell before a basin, the next starts at the spill cell).
- `build_resident` runs the producer after H1 (phase "lakes") and returns `Built::lakes`.
- GEN-163 (generator-pipeline) and TEST-108 (testing) added. In memory only: the store field and the kind-13 version bump are the next PR (design notes section 8).
- Tests: `tests/bake_lakes` (five tests) and the phase-plan test in `tests/bake_compact`.

## Impact
- Code: `generators::bake::{lakes, build}`, tests as above. No change to H1, K7 or any stored bit; the stage manifest is unchanged.
- Closed in PR 1b (architect, 2026-11-10): "inside a lake" is now "under the water": `water_cells` takes the cells of a wet lake's basin with `z` under its level and the cells under the level that drain into them (receiver chains, so nothing climbs over the sill or follows the outflow); `lakes_from_hydro` takes the surface `z` H1 ran on. A gentle shore under the level is water; labelled cells above the level of an endorheic lake are not, so the river continues down to the water's edge; a dry lake has no water.
- PR 1b review (numerics reviewer): the routing runs on the filled surface, so a channel through an endorheic lake's water would go on over dry basin cells up to the spill; those labelled cells downstream of the water are not drawn (`Water` holds them). The water test is a predicate on the labels and heights; only the shore and that dead ground are stored (a few MB at k=4, not the water of the basins); the spread follows donors only, so a nested basin's labelled cells are never joined to another lake's water.
- Open (H1, not this change): when a shelf under the level lies on the outflow path between the last basin cell and the spill, H1 adds the basin's `q_out` only at the spill cell, so the piece starting on that shelf has the local runoff as `Q_w`.
