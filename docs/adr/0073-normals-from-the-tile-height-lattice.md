# 0073 Normals from the tile's height lattice
Status: proposed (sonnet-spikes, 2026-10-20, at opus-planner's request; the owner decides). Chosen by opus-planner after SPIKE-23 as the root fix of the refresh cost; amends GEN-019 (the canonical normal) and GEN-010 (the mesh version); extends ADR 0033 point 4.
Date: 2026-10-20
Source: SPIKE-23 (`spikes/edit-refresh/results/results.md`); evidence of this ADR `spikes/lattice-normals/results/results.md`; GEN-019, GEN-020, GEN-025, COORD-007

## Context
- A tile's vertex normals come from four extra height evaluations per vertex (`normal_stencil`, +-t1 and +-t2 at one cell, in a tangent frame of the vertex): 4,356 of the 5,446 evaluations of a 32-cell tile (80 %).
- The evaluations are the whole cost of a tile job (about 40 ms of one thread, the stamp stage under 1 %, SPIKE-23), and so of every refresh, every dive and the start-up load.
- The lattice already holds the heights one cell either side of every vertex; the stencil measures the same two-cell baseline in other axes.
- A tile's normals must agree with its neighbour's at shared vertices (no lighting seam); today they do exactly, because the stencil is a function of the direction alone.
- Measured on the real worlds (256-cell bake and the 16384 store): the lattice normal differs from the stencil normal by a mean of 0.01 to 0.05 degrees, p99 at most 0.39, max 1.03 (steep tiles of levels 14 to 16); lit stills of the five static views and the coastal flights differ by at most one 8-bit level (RMSE at most 3.5e-4 of full scale; no pixel differs by more than 1 % of full scale).

## Decision
1. **The normal of a vertex is the cross product of the centred differences of its neighbours' positions along the lattice axes**: `n = (q(i+1,j) - q(i-1,j)) x (q(i,j+1) - q(i,j-1))`, normalised, `q = d x (R + h)` in f64, oriented outward (`n . d > 0`), the lattice being the tile's own `(cells + 1)^2` grid (the face lattice, COORD-007). One definition for the CPU mesh, the compact tile and every test oracle (the canonical normal of GEN-019).
2. **The ring.** A tile evaluates its `(cells + 1)^2` lattice points and the one-sample ring around them without the four corners (`4 x (cells + 1)` points) plus its origin: 1,222 evaluations at 32 cells instead of 5,446 (4.46 times fewer), one batch (GEN-020).
3. **Seams.** Within a face the shared vertices of two tiles of one level use the same four points and the same heights: bit-identical normals (measured 0.00000 degrees on 24 of 24 pairs). Across a cube edge the ring point beyond the face edge is taken from the neighbour tile's own lattice (the point one cell inside the neighbour, found through the face mapping, COORD-007), not from the extended cube plane: both tiles then read the same four points and agree exactly. The extended-plane ring (`sample_direction_ext`) was measured at up to 0.2 degrees across cube edges and is not used. Tiles of different levels are as today (skirts and morph).
4. **The read set is unchanged**: the normal reads one cell beyond the tile's edge, as the stencil did, so the keys' read records, the footprint find and the refresh of an edit name the same tiles.
5. **`MESH_VERSION` 9.** Every tile key changes (the version is a key input, GEN-010); the persistent mesh keys, the pre-filled coarse tile pack (STRM-021) and the committed known-answer mesh hashes are re-made once; repro bundles of mesh version 8 are refused with the versions named (TEST-008).
6. **Spec text.** GEN-019: the canonical normal is the lattice normal; the stored-versus-canonical bound (0.01 degrees) is unchanged. GEN-010: version 9. GEN-025 (the slot layout) is unchanged: the texel, the slot and the shader read the same words. New tests: the lattice normal against a converged analytic normal on a smooth field, the same-face and cube-edge seams (0 degrees), the evaluation count of a tile (1,222 at 32 cells).

## Consequences
- A tile costs about 4 times less to generate (40 to about 10 ms of one thread, estimated; SPIKE-23 model: the drawn-exact time of the 200 m / 150 m edit 508 to about 180 ms, within budget in all nine cases of the edit-latency scene).
- Normals differ from before by the figures above; fine detail is shaded with the same two-cell baseline filter, so it is neither smoother nor sharper (hillshades of levels 14 and 17 side by side in the evidence).
- Goldens: the debug `normals` and the shaded relief views change by one or two levels on many pixels: declared change (AUTO-E) on each adapter; every lit and water view changes by at most one 8-bit level and escalates to the owner's `/bless` (ADR 0017: any lit view). One re-bake of the 16384 store is not needed (the bake holds no normals).
- Numerics review required (core and generators, f64 position differences of 2R-sized numbers: the differences are of positions that share the radius, formed as `d (R + h)` with the radius factored per the CON-11 rule, to be checked).
- Easy to reverse: the version bump re-keys everything back.

## Licences of adopted code or techniques
None (the centred difference of a height lattice is textbook).
