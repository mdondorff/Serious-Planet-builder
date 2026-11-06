## Why
ADR 0104 point 3 (as amended 2026-11-05): the carve reads the climate. A river from a wet range keeps its power across a desert, and vegetation cover changes how readily a slope erodes. The orographic climate (GEN-117) is on main; both carves ignore it (plan row runoff, planner inbox #87).

## What Changes
- Discharge: the drainage area of the carve's routing becomes `Q = sum(w dA)`, `w` the runoff weight of a cell (runoff over a reference runoff), so the stream-power number is `f = k_dt K sqrt(Q) / dx`. The routing gives each cell the area `w dx^2` to start with; the multiple-flow-direction sweep is unchanged.
- Runoff: `R = P - E` with the Budyko-Choudhury evaporation `E = P PET / (P^n + PET^n)^(1/n)`, `n` 2.6, `P` the annual precipitation and `PET` the potential evapotranspiration of the orographic climate (GEN-117); `w = clamp(R / 300 mm, 0.02, 8)`.
- Vegetation: a local erodibility factor of `P` that peaks in semi-arid climates (Langbein and Schumm 1958), multiplied into K by `BakeInputs::climate_k`.
- `generators::runoff` (the laws and the Earth defaults as named constants, ADR 0108), `Rock.runoff` weights in the carves, `RockFields::runoff` and `vegetation`, key bits 2 and 3 of the `rock` byte, `RUNOFF_VERSION` in `BakeInputs::id`.
- The stored area channel (GEN-110) holds `Q / dx^2` of a bake with runoff: an effective drainage area in cells of reference runoff.
- GEN-129 in the generator-pipeline delta.

## Impact
- Code: `generators::{runoff, bake::{route, carve, face, tile, inputs}}`, `cache::{layer_store, layer_tiles}` (key bits), the bake examples. No golden moves.
- Relief and elevation change where the climate is not Earth-average; the preview table is in the PR body.
- Memory: +0.62 GB per face for the runoff weights (f32 per cell, the face with its halo, 1.56e8 cells) while the carve runs.
