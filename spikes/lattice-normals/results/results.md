# Evidence for ADR 0073: normals from the tile's height lattice (lattice-normals)

Spike code: branch `spike/lattice-normals` (never merged): `crates/generators/src/compact.rs` (an env switch `PLANET_LATTICE_NORMALS`, `ring_directions`, `lattice_normals`), `crates/app/examples/spike_normals.rs` (errors and seams), `spike_hillshade.rs` (the pictures), `crates/app/src/litperf.rs` (`CAPTURE_STILLS`: the lit stills of the static views). Worlds: the editor's default (single planet, seed 1, eroded field) over the 256-cell in-memory bake and over the 16384-cell store (`sonnet-m4/target/accept/global16384`, read-only). No perf run; the stills ran on the software adapter.

## The method
Current: the four height samples at +-t1, +-t2 (a tangent frame of the vertex, one cell of angle) per vertex: 5,446 evaluations per 32-cell tile. Lattice: the heights at the `(cells + 1)^2` lattice points and the one-sample ring (`sample_direction_ext`) around them: 35 x 35 = 1,225 directions (1,222 needed without the four ring corners and with the origin), the normal the cross product of the two centred position differences along the lattice axes, `n . d > 0`. Same baseline (two cells, centred) in both; only the axes differ (a tangent frame against the face axes).

## Accuracy: angle between the lattice normal and the current normal per level (cells 32; 24 tiles per level: 4 of the steepest by height bounds of 400 random tiles and 20 others; 26,136 vertices per level)
256-cell bake:
| level | spacing m | mean deg | p99 deg | max deg | max deg, 4 steepest tiles |
|---|---|---|---|---|---|
| 6 | 4,886 | 0.0167 | 0.1233 | 0.3036 | 0.3036 |
| 8 | 1,222 | 0.0275 | 0.2019 | 0.4808 | 0.4808 |
| 10 | 305 | 0.0354 | 0.2671 | 0.6370 | 0.6370 |
| 12 | 76.4 | 0.0421 | 0.3003 | 0.7909 | 0.7909 |
| 14 | 19.1 | 0.0444 | 0.3639 | 0.9078 | 0.9078 |
| 16 | 4.77 | 0.0370 | 0.3583 | 1.0039 | 1.0039 |
| 18 | 1.19 | 0.0114 | 0.1785 | 0.7114 | 0.5884 |
| 20 | 0.298 | 0.0114 | 0.0564 | 0.2929 | 0.2182 |
| 22 | 0.075 | 0.0170 | 0.1101 | 0.4307 | 0.4307 |
16384 store: the same shape: mean 0.007 to 0.045, p99 0.057 to 0.388, max 0.217 to 1.026 degrees (level 16 the largest). The two normals are two estimates of one surface from the same samples at one-cell baselines; where the field has structure at the scale of a cell (levels 10 to 16 on the eroded relief) they differ by the aliasing of that structure; the difference is of the size of the oct-encoded texel step only at the smooth levels (0.0072 degrees).

## Seams
Largest angle between the normals two neighbouring tiles of one level compute for the same shared vertex:
| case | current stencil | lattice, ring on the extended cube plane | lattice, ring through the neighbour's lattice (the ADR) |
|---|---|---|---|
| same face, levels 8, 12, 16, 20 (6 pairs each) | 0.00000 | 0.00000 | 0.00000 (same four points) |
| across a cube edge, levels 8 to 22 (chosen tiles, 4 sides each) | 0.00000 | 0.00000 to 0.2 degrees (level 22 south side 0.1995; most under 0.06) | 0 by construction (the shared vertex reads the same four points from both tiles): to implement and test |
A cube corner (three faces) needs the ring point of the tile in the diagonal direction beyond the corner: the corner vertex's four neighbours are two inside the own face and two on the two neighbouring faces; the normal at a cube corner is the one the three tiles must agree on: define it as the normal of the lowest face index's lattice with the extended ring (a tolerance of 0.2 degrees on the 8 corner vertices of the planet at each level) or leave the ring corner cases to the implementation's choice with a test (an open point for the implementation).

## The effect on shading
- Lit stills (software adapter, 1280x720, the five static views of the lit scenes, one frame each, both normals, same tiles and sun): the root-mean-square difference is 1.4e-5 (coast-3m) to 3.5e-4 (aerial-20km) of full scale; at a 1 % fuzz no pixel differs. The same for the coastal flights (24 frames of each of four flights: RMSE 1e-5 to 9e-5). `images/lit-ground-1m.png` and `images/lit-aerial-20km.png`: current | lattice | difference x 32 (black). These worlds' lit views are plains and haze; they cannot show a difference.
- Where there is relief, the shaded hillshade: `images/hillshade_L14.png` and `images/hillshade_L17.png` (6 x 6 tiles of 32 cells around the steepest tile of level 15 of the world at 2,120 m; one low sun at 20 degrees; left current, middle lattice, right the difference x 32 in grey levels): the two shadings show the same ridges and creases at both levels; the difference is isolated pixels of one grey level (an angle of mean 0.067, max 0.395 degrees at level 14; mean 0.032, max 0.348 degrees at level 17).
- Fine detail at the finest levels (20 and 22, spacing 0.3 and 0.075 m): the same two-cell filter as before, so no detail is gained or lost; the lattice normal is not smoother. A four-vertex-averaging or a larger baseline would smooth it; not proposed.

## Cost
Evaluations per 32-cell tile: 1 + 1,089 + 132 = 1,222 against 5,446 (4.46 times fewer); the height evaluations are the whole tile job (SPIKE-23: 39.7 ms with the stamp stage under 1 %), so a tile costs about 9 to 10 ms of one thread, estimated (not measured: the spike's `PLANET_LATTICE_NORMALS` path was not timed).

## Open
- The ring through the neighbour's lattice (and the cube corners) is not implemented in the spike; it needs a function in `planet_core::tile` beside `sample_direction_ext`.
- The measured tile time with the path: one line in a window.
- The CPU mesh builder (`mesh.rs`, the canonical mesh and its known-answer hashes) takes the same change; the spike changed only the compact tile.
