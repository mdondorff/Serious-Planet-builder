## ADDED Requirements
### Requirement: REND-068 The lit paths shade with the cell's bilinear texel normal
The compact renderer's lit fragment stages (the lit and the lit-with-atmosphere views) SHALL shade a fragment with the bilinear interpolation, at the fragment's tile coordinates, of the stored normals of the 4 texels of its cell, normalised (`tile_normal`), so that the shading of the ground does not depend on the direction of the cell's triangulation (the `a-c` split of `grid_indices`).

Verify: C
Status: active
Source: Lead 2026-11-24 (#200; the viewer mesh split note, option c1; G3-9), spikes #184g, REND-016, REND-009, ADR 0062
Detail:
- the fragment's cell is `(floor(u CELLS), floor(v CELLS))` of its tile coordinates `aux` clamped to the tile, the last row and column taking the cell of the highest index; the weights are the fractional parts; the normals are the slot's texel normals as the vertex stage decodes them (GEN-025);
- a previewed node (REND-033) keeps the vertex normal, which carries the preview's slope;
- the debug stage (`fs_view`: modes 1 to 8 and relief) keeps the interpolated vertex normal, so its pictures stay bit-identical; the lit-stage overlays (VIEW-014 tiles and mesh, VIEW-044 stale, the region tint) tint the lit colour and change with it;
- the reference renderer (ADR 0062, REND-031) has no lit path: its `tile_normal` returns the interpolated vertex normal only so that the shared shader tail compiles, and a lit plan never reaches it;
- unchanged: the triangulation, the mesh and its keys, the slot layout, the pools and the morph (REND-009, REND-014); no planet radius enters the shader (CON-11).

#### Scenario: A scarp across the diagonal
- **WHEN** a smooth scarp through the centre of a face is lit by a low sun across it, once at 45 and once at 135 degrees to the tile grid, and the 135-degree picture is turned back by a quarter turn
- **THEN** the two pictures agree within TEST-153's bounds on the lit stage, and the lit-with-atmosphere stage's mean difference is within its bound
