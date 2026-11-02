# SPIKE-24 evidence for ADR 0073: normals from the tile's height lattice (lattice-normals)

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
A cube corner (three faces): with the rule of the ADR (each tile takes the ring point beyond a face edge from the neighbour tile's own lattice, one cell inside it, on the row that continues the shared edge) the three tiles at a corner read the same three points and compute `(a - c) x (b - c)`, cyclically invariant: the normals agree to f64 rounding, not bit for bit (to be implemented and tested at all eight corners at levels 3, 12 and 22 against the renderer's 1e-6 degrees seam scenario; the spike did not implement it).

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

## For the change's proposal (the spec guardian's list; a task list, not part of the decision)
Spec text that states the version or the canonical normal and changes with ADR 0073:
- GEN-010 (`generator-pipeline/spec.md:128`): add "version 9 defines the canonical normal as the lattice normal (GEN-019): every key changes, the vertex heights and positions are unchanged, the committed mesh hashes are re-recorded".
- GEN-019 (`:311`, scenario `:319`): the lattice-normal definition as a normative sentence (the formula, the ring, the seam rule at an edge and at a corner), `MESH_VERSION` 9 in the scenario; the 0.01 degrees stored-versus-canonical bound unchanged.
- GEN-025 (`:503`, scenario `:519`): "`MESH_VERSION` SHALL be 9", the known answers re-recorded; the layout unchanged.
- GEN-003 (`:31`): the determinism manifest's mesh hashes (committed tiles, every face, the cube corners) and the version it records.
- Renderer (`renderer/spec.md:143`, "Seam normals": every level-2 tile, coincident border vertices within 1e-6 degrees across the twelve cube edges; corner vertices included) holds with the ring rule; the pixel-hash scenario at `renderer/spec.md:294` names `MESH_VERSION` 8 and is re-recorded.
- STRM-021 (`streaming-cache/spec.md:619, 654`): the pre-filled pack is made again (`generate --prefill-dir`, 1,920 tiles; payload version 3 stays); the store check of `testing/spec.md:602/614` re-run.
New tests (tier A): lattice normal against a converged analytic normal on a smooth field (bound to be set by the change: within twice the lattice truncation error) (GEN-019); same-face, cube-edge and all eight cube-corner seams at levels 3, 12 and 22 with 1e-6 degrees (renderer seam scenario, COORD-007); the evaluation count of a tile (GEN-020); the existing footprint-halo test over the ring's directions (WDEF-007); known answers of the committed meshes and the manifest (GEN-010, GEN-003). `surface_normal` and `surface_normal_field` (`mesh.rs`) have no caller outside `mesh.rs` and `compact.rs`: the change removes them or keeps them as non-canonical.
Goldens to name in Expected golden changes with their bound tests: the debug `normals` golden `terrain_cube_corner_normals` and any shaded or debug view that shows normals or lighting (measured by the change); the lit, sky and water goldens that move escalate. On acceptance ADR 0033's status line gets "point 4 refined by 0073" (CON-23).

## Reviews of the draft (numerics-reviewer and spec-guardian, 2026-10-20) and what the evidence does not show
- Numerics: the f64 form `q = d * (R + h)` is accurate to 1.3e-6 degrees at level 22, 2.1e-5 at 26 and 6.5e-5 at 28 against a 60-digit reference (the floor is the rounding of the unit direction; the radius-first form measured the same). The cube-corner rule needs one common expression (canonical order of the three edge points, `(p1 - p0) x (p2 - p0)`): the simple "one cell inside" rule gives three different floating-point expressions of one triangle normal (54 % of 20,000 simulated corner cases differ in the last bits, at most 1.9e-6 degrees; the oct-encoded words matched in all). The ring across an edge must come from the neighbour's integer lattice mapping: re-projecting the extended plane lands off the vertex's row by up to a cell at the corner. The `n . d` flip should be an assertion (every face has `U x V` outward); the 1,225-point ring of the spike includes the four ring corners the implementation drops (1,222).
- What the spike measured and did not: the accuracy table and the seam rows were measured with the extended-plane ring through `field.heights` (not `heights_and_climate`), and as printed angles (five decimals), so the same-face "0.00000" is not a bit test and the cube-edge claim is unmeasured: the change's tests compare the shared vertices' stored `u32` words and the CPU mesh's f64 bits at one and three threads, same face, the twelve edges and the eight corners at levels 3, 12 and 22. The edge and corner vertices of the proposed ring are first-order estimates on relief (the neighbour path bends at the edge); the accuracy test sets its own bound for them or excludes them and says so. The lit stills show plains and haze; a 1 degree normal change moves a Lambert shade by up to about five 8-bit levels under a low sun, so the lit and shaded goldens are measured by the change, not assumed to be within one level. The per-tile time with the path is untimed (the saving is bounded by 4.46 times).
