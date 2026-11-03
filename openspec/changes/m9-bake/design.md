## Context
- Revised 2026-10-31 after SPIKE-68 and SPIKE-70: the carve per tile left core-edge steps of p99 134-172 m (routing is non-local), so the carve runs per cube face at about 800 m and the tiles keep only local work (decision 4).
- ADR 0088 (proposed): two baked channels, the runtime base `m + k (e - m) + k creases`, and the erosion layer from a transient carve with breach, thermal and deposition.
- Spikes:
  - SPIKE-61: the 100 m grid, tiles, halo, inflow and time;
  - SPIKE-62: routing cadence and flow direction;
  - SPIKE-64: thermal and deposition kernels;
  - SPIKE-65: the levers;
  - SPIKE-66: the factor in the generator, bounds, level steps, the Alps factor, the breach.
- M9 draft decision 7 (owner): the erosion result is a separate layer first, folded into one store once accepted.
- Today's field (`erosion.rs`): `e + A(e, carve) S`, the bake plus creases; the landform driver (GEN-062) lifts the fine creases.

## Decisions
### 1. Channels (proposed values; the spec holds them)
| Channel | Grid | Value | Bounds |
|---|---|---|---|
| Relief driver `k` | 4,096 cells per face edge (about 2.4 km) | 1 on plains, about 4-6 on hills, about 7.5 on mountains; a curve of the bake's 50 km elevation spread, calibrated at the scorecard sites; times a coast taper from 1 at the coast band (ADR 0056) to full 400 m above the sea | min and max per block |
| Ring mean `m` | 4,096 cells per face edge | the bake's mean over 10 km (9-point ring as SPIKE-66, or a box) | min and max per block |
- A bilinear read of each channel at a point stays within the batch footprint (GEN-020): one cell of reach, against the 10 km stencil SPIKE-66 showed failing.
- Determinism: the channels are computed from the bake in f64 with `libm` on the bake's own grid; a stored channel is read, never recomputed at runtime.

### 2. Runtime base and bounds
- `h = m + k (e - m) + k A(e, carve) S`, plus the erosion layer (decision 4). With the switch off, `k = 1` and `m` is not read: today's field bit for bit.
- Bounds over a cap: `e` from the bake scan, `k` and `m` from their blocks. Then `m + k (e - m)` from the intervals, the creases times `[k_lo, k_hi]`, and the layer's block range added. The bounds of nested caps nest (LOD-012).
- Level steps grow with `k` (SPIKE-66: x6 at every level). The gate (criterion 6) and a tier-B level-step check on the baked field decide whether tau or the morph range needs a change.

### 3. Bake kernels (`generators/src/bake/`)
| Kernel | From | Parameters (proposed) |
|---|---|---|
| Flood | Barnes et al. 2014, improved (SPIKE-62) | 1 mm over flats |
| Receivers | jittered D8 on the filled surface (SPIKE-62) | slope x (1 + 0.5 u), `u` an integer hash of cell and direction |
| Order | donor stack from the receivers (Braun and Willett) | none |
| Carve | implicit stream power with uplift, n 1, m 0.5 (SPIKE-65); per face at about 800 m (SPIKE-70) | U/K 20, K dt 0.05, 20 steps (K t 1), re-route every 5th step |
| Local carve | the same kernel per tile at 100 m with the drainage area capped (SPIKE-70 hybrid); it only incises: no fill, no breach, no uplift (SPIKE-72, SPIKE-73: with uplift, undrained cells make spikes) | cap about 1 km^2 |
| Breach | before each fill: a receiver at most 0.1 % below its donor, by at most 50 m (SPIKE-66) | as stated |
| Thermal | Jacobi gather at repose (SPIKE-64), on active cells (a full Jacobi costs about 13 h for the planet, SPIKE-70) | 33 degrees x (1 + 0.25 u), 1/16 per pass (at most 1/8), a pair under 1 mm over its critical difference does not move, until no cell moves or 100 passes |
| Deposition | one sweep on the last route (SPIKE-64) | `kc` 1e5, `zeta` 0.5 |
- All parameters live in the spec and are tuned in R7 against the scorecard.

### 4. The erosion layer
- Two passes (SPIKE-70):
  1. per cube face: the base sampled once per 8 x 8 cell (814 m, 12,288^2 a face; SPIKE-71: 1/47 of the block means' cost, relief within 1-3 %), the carve (about 5.5 GB), its change stored per face (checkpoint);
  2. per tile: the base plus the face change (Catmull-Rom, C1: owner 2026-11-03), the local carve, active-cell thermal (exact, SPIKE-71), deposition; the core's change stored.
- Grid: 98,304 cells per face edge (101.8 m at 16384, 48 per coupled cell), tiles of 3,072^2 core cells with a 256-cell halo (SPIKE-61); 2,140 land tiles.
- Content: the core's change against the sampled base (decision 2 without the layer), quantised (proposed 0.25 m, i16) and compressed per block, about 0.34-0.42 bytes per land cell (SPIKE-61); about 7-10 GB.
- Inflow: not needed per tile (the face carve holds the large rivers); faces do not route across cube edges (a known limit).
- Seams: core edges show none at four of four sites with the incising local carve (SPIKE-72). Without it, basins crossing a core edge fill to window-dependent levels. PR 5 re-measures the seam with the deposition sweep in place (it restores the flats the unfilled pits lose: 0.9-2.5 % against real 6-9 %), and PR 4 measures the cube-edge step of the face carve. A halo blend hides a step but does not fix routing (SPIKE-68).
- Checkpoints: one file per finished tile plus an index; `--resume` skips finished tiles; a key mismatch (bake, channel versions, kernel parameters) refuses to resume.
- Read: a block cache like the bake's; bilinear at 100 m.

### 5. Switch and rollout
- A world definition flag (or a viewer option during M9) turns on the channels and the layer together.
- With it off, nothing changes: goldens stay, and the gate and envelope run on today's field.
- The owner's look at R4 (stills at 300 m, 30 m and 1.8 m, plus the scorecard report) decides the default.

## Risks
0. Bake time: about 6-7.5 h estimated (central 6.9 h, SPIKE-71), within criterion 7's 10 h; no margin at the top of the range.
1. Level steps at production tau: a visible pop would need finer LOD near the camera (cost) or a smaller `k` at fine levels (a scale-dependent factor). Measured in PR 2.
2. Tile seams of the layer: measured in PR 4; the blend is a fallback.
3. Coast and water: the taper keeps lowland above the sea. The ocean connectivity (ADR 0071) and the coastal band (ADR 0056) read the new heights, so their channels may need a re-bake with the layer.
4. Badlands and desert must not get worse (criterion 1): their `k` stays near 1 by the curve; the scorecard checks.
5. Bake time: the estimate (2.5-4 h) comes from spike medians under load; PR 4 times one tile per landform before the full bake.

## Open questions
- Requirement IDs: TBD, ask the planner.
- Whether `m` uses SPIKE-66's 9-point ring or a separable box (same cost at bake time; the box is smoother).
- The `k` curve's input: the bake's 50 km spread or a new landform map (M10 drivers). The spread is proposed for M9.

### 5. The stored form of the class weights and the relief channels (PR 1b)
| | Class weights (STRM-067) | Relief channels (STRM-068) |
|---|---|---|
| Kind byte | 11 (`GlobalClassWeights`) | 10 (`GlobalRelief`) |
| Records | 6, one per face, filed under the face's level-0 tile | 6, the same |
| Key | stage key of the bake (source), bake resolution (tag 4), cells (tag 1), class count (tag 14); not the class-list version (found and refused naming both) | the same plus tag 13: channels' version, class-list and class-source versions, class-lattice hash, `max_step`, ring radius, taper |
| Payload | codec u8, class-list version, class-source version, cells, lattice hash (16), weights (`u8` x 8 per point) | codec u8, cells, `k` plane, `m` plane (f32 LE); ranges recomputed on load |
| Size at Earth's size | 257 x 257 x 8 x 6 = 3.2 MB at 256 cells | 2 x 2049 x 2049 x 4 x 6 = 201 MB at 2048 cells |
- No store holds these kinds yet; an older store opens as before and reads them as absent. The kind bytes are persisted and never change meaning.
- `max_step` is in the relief key although the tapered `k` does not depend on it today: a bake parameter changes the bytes it is the limit of, and the key must not claim otherwise (planner, 2026-11-02).
- The loaders return `Ok(None)` for absent, an error naming the record for damaged or incomplete.

### 6. Open for R7 (the scorecard)
- A 9-sample ring on a 4.9 km lattice aliases relief under 10 km (the numerics review measured a neighbour-to-neighbour `|dm|` of about 122 m on a 7-9 km test surface), and the runtime base `m + k (e - m)` multiplies it by `(1 - k)`, up to 6.5. SPIKE-66 and ADR 0088 chose the stencil; the scorecard on the baked field is where it shows. No change now.

### 7. Run modes: a coarse planet pass and full-resolution regions (owner, 2026-11-02, the I5b snapshot)
The bake code takes the resolution and the region as parameters, so one code path serves the snapshot (I5b), the full bake (I6) and, later, global authoring.
| Parameter | Full bake | I5b snapshot |
|---|---|---|
| Planet resolution (`--planet-res N`, cells per face edge of the grid of the pass that runs; PR 4 runs pass 1 only, so there it is pass 1's grid) | 98,304 (101.8 m) | a divisor of it, e.g. 12,288 (814 m: pass 2 on the face-carve grid, about 1/64 of the cost) |
| Face carve grid (pass 1) | 12,288 (814 m) | the same (SPIKE-71 estimated 3 min a face; PR 4 measured about 4 us per cell, so 10 to 13 min and 6 GB a face at 12,288: point 8) |
| Region (`--region <tile list>`) | every land tile (2,140) | 30-40 sample tiles at full resolution (101.8 m), picked from a list file |
| Channels (`k`, `m`) | 2048 cells per face edge | the same or coarser: the reader takes the cells from the channels |
- The erosion layer's key carries the grid resolution (cells per face edge) and the region mode, so a coarse layer and a full layer never share a key; a tile record carries its level and cells (STRM-066 follows).
- A region run reads the face change from pass 1 (Catmull-Rom, C1) and runs the tile driver on the listed tiles only; a planet pass at reduced resolution runs the same tile driver with a coarser grid over every land tile. The two do not need each other: a region can be run without the planet pass.
- Non-editor: the modes are `generate` options (`--planet-res`, `--region`, `--erosion-layer DIR`) under the heavy-run lock, with checkpoints and `--resume`; they write the layer and preview maps (land and sea, elevation with hillshade, the layer's change, the `k` and `m` channels) beside it, so the snapshot needs no viewer.
- **Which PR makes I5b runnable:** PR 3 (kernels) and PR 4 (face carve, `--planet-res`, parameterized resolution) come first; **PR 5 (tile driver with `--region` and the layer writer) is the first PR that makes the snapshot runnable in full** (pass 1 on the planet, pass 2 on the sample tiles and optionally at the reduced planet resolution, preview maps from the stored layer). After PR 4 alone a planet-level preview of pass 1 (the carved base at 814 m, no thermal or deposition) can be rendered; it is a partial look, not the snapshot. PR 6 (the runtime layer) and PR 7 (the overnight bake) are not needed for the snapshot.
- Order for the earliest snapshot: PR 3 splits into 3a (flood, receivers, order, carve, breach: what pass 1 needs) and 3b (active-cell thermal, deposition and the timing gate: what pass 2 needs); the order is 3a, 4, 3b, 5. The snapshot's maps are produced by PR 5.

### 8. Open points
- A body whose relief scale is not 1: the relief channels are of the unscaled bake, so `ErosionField::with_relief` returns an error naming the scale (interim departure, accepted by the planner 2026-11-02, no ADR). How a scaled body's `m` and `e` scale together is decided at the app wiring.
- Pass 1 timing and memory (PR 4, `cargo xtask bake pass1`, Rasierklinge, 20 threads, a correctness run, not a perf measurement): one face takes 3 s at 1024 cells per edge, 21 s at 2048 and 66 s at 4096 (about 4 us per cell, the base sampled on all threads, the carve on one); the peak working set of the process is 52 MiB, 177 MiB and 666 MiB (about 40 bytes per cell). Extrapolated to 12,288 cells (151 M cells, 36 times 4096's cells; an extrapolation, not a run): about 6 GB and about 10 to 13 minutes a face, one hour or more for the planet. SPIKE-71's 5.5 GB agrees; its 3 minutes a face does not (the measured cost includes the class source and lattice evaluated twice per cell, the base and the carve factor). The faces run one at a time; the concurrency flag exists and refuses values above 1 until a measurement says two fit.
- The cube-edge step of pass 1 (PR 4, planner's decision 2026-11-03: a halo, not a fade): without a halo the border cells are outlets that keep their base heights, so the change one cell in from a border was at most 1,201 m (256 cells per edge, p99 408 m) and 660 m (1024 cells, p99 188 m) while the border cell was 0. With a halo each face is carved on its grid extended by `ceil(cells / 128)` cells (at least 4) beyond each edge and keeps the extended plane (the layer record has a halo field); a halo alone does not make the faces agree (the two lattices route the shared ground differently: the change differs by up to 900 m between the faces at one place), so the reader cross-fades the two faces within the halo's width (`bake/map.rs` `change_at`). Measured at 1024 cells on seed 14 (the cell at an edge to the cell across it, the corrected read of the neighbour's halo): halo 8 at most 413 m (p99 147 m), halo 32 at most 625 m (p99 201 m), against 893 to 907 m (p99 250 to 222 m) between neighbouring cells inside a face; at 512 cells and halo 4 (the default) at most 492 m (p99 174 m) against 1,111 m (p99 406 m) inside. So the seam is below the carve's own cell-to-cell roughness; it is not zero (two independent lattices disagree where a valley runs along an edge), and a wider halo does not lower it (the cross-fade only mixes more). The change map shows no seam lines. At a corner the two neighbours across the two nearest edges both count and the three faces at the cube vertex weigh alike (tested on constants). Cost at 1024 cells: the grid grows by `(1 + 2 halo / cells)^2` (3 % at halo 8), the measured wall time and peak memory differ from no halo by less than the run-to-run noise (about 6 to 7 s and 52 to 53 MiB for two faces); at 12,288 cells the halo is 96 cells (`ceil(cells / 128)`, at most just under half the face) and 3 % of the cells. The record grows by the same share (about 18 MB a face).
- The face grid's cell size is the centre-line `dx` everywhere, but the tangent warp makes cells near a face's edges and corners smaller along the edge (about 0.71 of `dx` at an edge's middle), so the carve's slope and area are off there by tens of percent and add to the cube-edge step. Not measured; a halo or fade (the point above) hides it near the edge, a per-cell metric would fix it. Decide with the cube-edge option.
- Memory not in the 6 GB: a face record at 12,288 cells is 604 MB and is built and encoded in memory (about 1.2 GB more at the write), and the maps load all six faces (3.6 GB). Fine for a coarse run; PR 5 streams the plane when it matters.
- Thermal cost grows steeply with relief (PR 3b, a synthetic tile of 3,584 cells a side, 100 passes, 4 threads, Rasierklinge, a correctness run): relief x1: 0.5 s, 1.0 % of the full run's cell evaluations; x1.4: 11 s, 7.8 %; x2: 59 s, 25 %; x3: 245 s, 41 % (2.7 M cells still moving in pass 100). The ground is synthetic (hills with incised valleys), not SPIKE-61's real tile, and x2 and x3 are steeper than real ground; but SPIKE-64's real tile scaled x4 kept 0.8 M cells moving, so the 5 s of the planning (a nearly stable upland) is not the cost of a bake whose relief is realistic. For the planet at 2,140 land tiles four at a time, 10 to 60 s a tile is 1.5 to 9 h. The real-tile gate is PR 5's; the open question for the planner is whether thermal's pass cap (100) or a tile budget bounds it.
- Pass 2 in PR 5 (`bake/tile.rs`, `cache/layer_tiles.rs`, `cargo xtask bake layer`): `--face-res` is pass 1's grid and `--planet-res` pass 2's (so a snapshot runs pass 1 at 768 to 4096 cells and pass 2 at 3072 to 12,288); the tile (default 3,072) and the halo (default a twelfth of the tile, 256 at 3,072) are options, and the tiles a face edge are a power of two. The run takes every land tile (a tile is land when a sample every `tile / 96` cells of the base plus the pass-1 change is above the sea: an island smaller than that can be missed) or a region list of at most 64 tiles; a planet pass above 12,288 cells a face edge and a pass 1 above 4,096 are refused without the planner's go. The pass-1 store is dropped after a complete planet pass unless `--keep-pass1` (the owner approved kind 12 as temporary working data, 2026-11-03); a region run keeps it. Thermal is bounded by its 100-pass cap only (planner, 2026-11-03: a wall-clock budget would make the result depend on load and thread count); the cells still moving at the cap are stored and reported per tile (`tiles.txt`); if the real-tile gate shows a cost over budget the levers are a lower cap or a coarser thermal grid, decided on measured numbers. The tile layer's key carries a digest of the six pass-1 face keys, so a tile of another pass 1 is refused, not mixed. A tile's core change contains the pass-1 change (the layer is base plus one change), so the runtime reads the tiles alone once pass 1 is dropped.
- Pass 2 seams (PR 5, tier A on a synthetic planet; TEST-084's judged half, not yet on SPIKE-70's four sites): on a steep synthetic base the step of the tiles' change across a shared core edge has a p99 of 120 m against 132 m between neighbouring cells beside the edge (tiles of 24 cells, halo 6, cells of 100 m); on a smooth base 7.0 m against 5.1 m, so a seam exists and is hidden by the steep base's own roughness. Cause (numerics review): the local carve caps its catchment area but the deposition sweep routes the whole window, so its loads and capacities depend on where the window border falls. Levers if the four sites show it: a larger halo, capping the deposition's area, or truncating its inflow to the local catchment; decided on measurements. The cube-edge step is the pass-1 figure above.
- Other pass-2 decisions (PR 5, numerics review): the local carve's area cap is 96 cells (1 km^2 at 101.8 m) so a coarse planet pass keeps its drainage (the cap in square metres would have made every cell of a 3 km grid the same); the planet pass looks for land on pass 1's own lattice (a tile with no land cell there has no pass-1 change to lose when pass 1 is dropped; the blended pass-1 change over sea cells beside a coast is lost with it, small); the statistics `eroded`, `deposited` and `exported` are window totals (the halo counts, so the planet sum overcounts by about 1.36 at the defaults), `land_cells` and `clamped_cells` are the core's; the halo is at most an eighth of the face edge; `PASS2_KERNEL_VERSION` (key) covers the window, the face-change blend, the local carve, thermal, deposition and the quantisation; the layer's change is against the class source's base (`BakeInputs::base`), not yet the relief channels' `m + k (e - m)`: PR 6 reads both and must rebase one on the other.
- The plan check (TEST-083) and the level-step probe with the switch on need the baked channels of `global16384`.
- Pass 1 at 12,288 with the multiple-flow-direction area and a C1 read of the change (owner, 2026-11-03, after the staircase diagnosis; PR `change/pass1-mfd`, kernel versions 2): the stills of the preview showed grid-aligned strips (a bilinear read) and staircase valleys and flat blocks at the pass-1 lattice scale (a D8 area imprints the eight directions). Measured on the mountains tile (face 2, tile 12/10; `spikes/erosion-bake-cost/results/` has the pictures): the share of the strongest low-passed gradients of the change within 5 degrees of the grid's axes (isotropic 11.1 %) was 18.4 % as built (4096 cells, bilinear), 18.0-18.6 % with more jitter or with smoothing in pass 2 only, 14.0 % with the area by multiple flow direction, and 11.2 % at 12,288 cells with that area and the C1 read. The area costs about twice the pass-1 time at 4096 cells (52 s to 101 s a face, local, indicative), the output is the same bytes on 2 and 10 threads, and more cells stay unsettled at the thermal cap on steep ground (195,185 at 12,288 against 53,867 as built on the mountains tile; the residual of the thermal relaxation is reported in SPIKE-98). The receivers stay D8 because the implicit carve solves along one receiver per cell.
- Two consequences of the area by multiple flow direction (numerics review, 2026-11-03): the area no longer grows along the D8 tree (a cell can send part of its area to a neighbour that is not its receiver), so in the deposition sweep the capacity `kc sqrt(A) S` can fall along a D8 path where the flow diverges and the sediment settles there (a fan; wanted, and measured: 14-19 % of the eroded volume deposited on the three region tiles); and the area is built before the donor lists, with the filled surface dropped after it, and an unstable sort (the order is total), because a face at 12,288 cells has 1.6e8 cells.
