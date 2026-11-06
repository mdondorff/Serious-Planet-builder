## Why
ADR 0102 makes erodibility a field K(x, z) (lithology, GEN-116) and ADR 0103 an uplift field U(x) (GEN-123); both exist on main as analytic fields with previews, but no carve reads them (plan PR 4, planner inbox #75). Hard caps over soft rock, belts that rise faster than cratons and badlands need both carves to read them.

## What Changes
- Hardness steps: the rock gives the lower edge of its band and one step never lowers a cell past it (`Layer`, `LAYER_SKIP_M`).
- `ROCK_K_SCALE` 1.16, a fixed incision-matched constant of the default lithology (`k_calib`); the rock `rock` byte in the layer and tile keys.
- `carve::carve_rock(.., &Rock, ..)` (`carve` is the same with no fields): per step the erodibility `K(cell, z)` of every cell at its height at the start of the step multiplies the class carve factor; an uplift array `U` replaces `u_over_k` and is not scaled by K or by the factor, only tapered by the starting height.
- `face::RockFields` (closures of a direction), `pass1_face_rock`, `tile::pass2_tile_rock`, `layer_store::run_pass1_rock`, `Pass2Inputs::rock`: pass 1 reads K and U, pass 2 (incise only) reads K.
- `BakeInputs` holds the lithology and the uplift (`rock_k`, `uplift_at`); `BakeInputs::id` folds `LITHOLOGY_VERSION` and `UPLIFT_VERSION`, so every layer key of a bake with the fields is new. The bake examples pass the fields.
- GEN-127 in the generator-pipeline delta states the carve's field inputs (amends GEN-100 and GEN-101).
- No kernel version bump: a carve without fields is bit for bit today's (tested).

## Impact
- Code: `generators::bake::{carve, face, tile, inputs}`, `cache::{layer_store, layer_tiles}`, the bake examples. No golden moves.
- Relief changes where the fields are read; the class amplitudes and the scorecard are re-tuned on the preview (owner's round).
- Cost: one erodibility evaluation per cell and step (a column, a few `fbm`), parallel over rows. Memory at 12,288 cells (halo 96, 1.56e8 cells a face): +0.62 GB for the erodibility (f32), +0.31 GB for the layer room (16-bit), +0.62 GB for the uplift (f32, exactly sized, filled 64 rows at a time): +1.55 GB while the carve runs, next to the existing arrays; the peak against the 8 GB budget (CON-21) is not measured here.
