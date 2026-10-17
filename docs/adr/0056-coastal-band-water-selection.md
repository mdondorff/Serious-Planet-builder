# 0056 Water selection: the coastal band, tile water in the tile job, a pixel-footprint wave fade
Status: accepted (opus-planner, delegation, 2026-10-17)
Date: 2026-10-17
Source: window 6 stalls and coastal captures (docs/log.md), ADR 0041, ADR 0051 point 6, ADR 0053 points 4 and 5; spec ATMO-012, ATMO-013, ATMO-019

## Context
- The bake calls a point land when its continentalness exceeds the sea threshold. The eroded terrain over low land lies below the sea level (mean about 17 m under it, on plains 200 to 400 km wide beside some coasts), so the selection of ADR 0053 drew a dry flat seabed behind a staircase of water.
- The selection reads the bake on the plan thread: a new tile of level 0 to 3 touches up to 25 store blocks of about 20 ms each (window 6: 8 s frames).
- Wave shading fades by the vertex spacing only; a far grazing pixel spans many wavelengths and shows aliased stripes.

## Decision
1. The plan extends W beyond the bake's sea (departs from ADR 0041 point 2 and ADR 0053 points 4 and 5 for the plan; the field W itself and GEN-023 are unchanged). A tile with no sea of its own carries the sea when one of its samples is band (land by the bake, low enough for erosion to cut it below the level) and an ocean vertex lies within the reach of the tile. The pass floods per vertex where `W - h_morph > 0`, as before.
2. The band's elevation is derived from the erosion's amplitudes and follows the level; the reach, the probe grid and their limits (steep ground, narrow inlets) are numbers and statements of ATMO-019.
3. An inland basin has no ocean vertex within the reach and stays dry (owner constraint 2).
4. The tile's water is computed in the tile's worker job, kept in a book beside the tile, and the plan looks it up. Nothing is stored in the tile or its keys (no `MESH_VERSION` change). A tile with no entry is computed on the plan thread and counted.
5. A wave whose wavelength is only a few pixels fades out of the shading by the pixel's footprint on the surface (ATMO-013); the CPU reference uses the same weight.
6. The coast views of the lit scenes and the capture stand at a beach found by terrain height (ATMO-019).

## Consequences
- Pits of the band within the reach of the ocean are flooded (estuaries); band beyond the reach stays dry. The reach approximates connectedness; a per-vertex mask (ADR 0051 option 3) replaces it if it shows.
- The first lit frame of a tile costs nothing on the plan thread; the tile job pays the water query.
- The lit known-answer hashes of the water frames change (pixel fade); no golden changes.
- Tests: a coverage oracle over a real bake (it fails on the previous selection), band, basin, level and thread-order tests at tier A, a job-fill count test, GPU against CPU footprint weights.
