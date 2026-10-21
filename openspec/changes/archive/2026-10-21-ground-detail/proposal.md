# Change: ground-detail

## Why
- M7 G2a (opus-planner, 2026-10-21): SPIKE-27 found that the smear at the feet is the shading, not the geometry. Option A, a procedural per-material term, makes the ground read as ground at 1.8 m and 0.5 m for +0.45 to +0.85 ms at 1440p in the spike. Change 1 (ground-materials, #528) made the weights a function with data curves; this change draws with them.

## What changes
- Shader: `ground_detail` in the lit fragment stages (terrain and atmosphere paths): six octaves (32 m to 3 cm; the two mid octaves are new against the spike), per-material albedo multiplier and normal tilt for rock, sand, grass and soil, weights from `material_weights` (the WGSL twin of `MaterialParams::weights`, reading the curve table from the globals). The sand ripples are weaker than in the spike.
- Node record: no growth (the integer cell of the origin at 1/32 m and its fraction ride in the unused fourth lanes of the first six grid phases; the spike added 128 B). Globals: +272 B (strength and the curve table).
- `LitPlan::ground_detail` (0 to 1, default 0 = off), written in the plan's text only when not 0 (LOOK-009).
- REND-034, LOOK-009.

## Design notes
- Anchor: octave cells are the arithmetic shift of one integer cell (two bits per octave), the fraction inside the cell is `(remainder + fraction) / cells`, exact in f32 up to 1024 cells; no planet-radius value in the shader (CON-11). Tested on the record (tier A) and by the slot-independence of the picture (tier C).
- Seam for the photo layer (owner, 2026-10-21): per material the procedural term is one source of `(albedo multiplier, normal tilt)`; a texture source returns the same pair from the node's decade phases, blended in by a distance weight before the weights combine them. The weights function, the curve table and the node record do not change. The shader marks the place.
- Reserved inputs: the region's surface type and the three driver slots read 0 until their sources exist (M8); a look that gives a material a curve on them needs no shader change.
- The albedo's slope threshold reads the detail normal (the tilt moves the rock edge a little with the noise); the weights read the geometric slope.
- Roughness and specular are G2b, a separate change.

## Not tested
- The WGSL shift and remainder for negative cells and a GPU comparison of `material_weights` with the CPU function are not probed directly (numerics review, 2026-10-21): the CPU anchor test mirrors the shift, the slot test and the table test (tier C) cover the behaviour. A probe in the `lit.rs` harness style is a follow-up if a seam shows.

## Tests
- Tier A: the anchor of the node record (REND-034); the plan token (LOOK-009).
- Tier C: off is inert, on changes the ground, the table drives the picture, the climate views are unchanged, the picture does not depend on the slots (REND-034). The whole tier-C suite passes with the goldens as they are (the term is off by default).

Expected golden changes: none (the strength is 0 by default; the existing goldens do not move).
