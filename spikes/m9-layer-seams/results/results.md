# SPIKE-68: erosion-layer seams

Opened: 2026-10-31
Question: How large is the height step across tile core edges when the M9 bake (ADR 0088) runs per tile with a 256-cell halo? Do channels end or jump there, and does a halo blend hide it?
Why it matters: ADR 0088's open risk (tile seams); the m9-bake change's PR 4 plans a 64-cell halo blend if the step exceeds 1 m p99.
Time box: 2 h (opus-planner, 2026-10-31); used about 2 h, with 45 min of machine time.
Success criteria: step p50, p99 and max across core edges; the routing difference at the edges; one blend tried if p99 is over 0.5 m.
Code location: offline crate (`proto/s68seam.rs.txt`, `proto/Cargo.toml.txt`) on f32 windows dumped by SPIKE-65's `relief_window_dump` (branch `spike/m9-relief-source`); never merged.
Closed: 2026-10-31

## Method
- **Windows:** 6,656^2 cells of the base field at about 100 m (store `global16384`, read only; dumps in `SeriousPlanetData/spike-r3/relief68/`, `raw/dump.log`).
  | Site | Latitude, longitude | Land | Heights |
  |---|---|---|---|
  | alps | 11.5, -18.5 | 84.6 % | -43 to 5,644 m |
  | blackforest | 2.5, -124.5 | 96.3 % | -33 to 3,379 m |
- **Tiles:** each window holds 2 x 2 cores of 3,072^2 (SPIKE-61's tiles), each with a 256-cell halo: four core edges of 3,072 cells meet at the centre.
- **Base:** the window amplified once by x6 about the 20 km box mean (SPIKE-65), so every tile sees the same base.
- **Bake** (ADR 0088's order without deposition):
  - the transient carve with uplift and the breach (U/K 20, K dt 0.05, 20 steps, re-route every 5th; SPIKE-66 (d));
  - thermal at 33 degrees, 100 passes (SPIKE-64).
  - Every hash (jittered D8, the thermal pair hash) uses the global cell, so a tile and the whole window jitter the same cell alike.
- **Reference (joint):** the whole window baked at once. Inside it there are no seams; the measures compare the tile mosaic against it.
  - The joint bake's own change against the base: p50 41 m and p99 1,856 m (alps); p50 17 m and p99 601 m (blackforest).
- **Variants**, each mosaicked hard (each cell from its own core) and blended (neighbouring tiles cross-faded over 64 cells centred on the core edge):
  | Variant | The tile's window edge | Inflow |
  |---|---|---|
  | no inflow | base level at its sampled height (SPIKE-61) | none |
  | inflow | the same | area from outside the window, at the first interior cell down the joint routing of the base |
  | seq | as inflow, and the cells inside an earlier tile's baked core held at that height (tiles in order 0 to 3) | as inflow |
  | ring (oracle) | the edge ring follows the joint bake's heights, base to joint linearly over the carve steps | as inflow |
- **Measures** (cores only, land):
  - **step:** the mosaic's height difference across a core edge minus the joint's, over 12,154-12,288 edge pairs;
  - **|mosaic - joint|** by distance from the nearest core edge;
  - **crossings:** cells with at least 1 km^2 of drainage area whose receiver lies across a core edge, each surface routed on its own; matched if the joint has one within 3 cells with an area within x2;
  - **ponds:** cells within 4 cells of a core edge filled more than 1 m by the flood.
- **Runs:** at BelowNormal under the heavy lock, CPU 2-6 % before each (`raw/run1.log`, `raw/seq.log`, `raw/ring.log`). A tile bake takes 37-51 s, the joint 133-195 s.

## Results
Step across the core edges (m) and channel crossings (carve and thermal):
| Site | Variant | Mosaic | Step p50 | Step p99 | Step max | Crossings mosaic / joint / matched | Ponds at seams (joint) |
|---|---|---|---|---|---|---|---|
| alps | no inflow | hard | 0.01 | 171.9 | 481 | 199 / 223 / 180 | 4,972 (2,188) |
| alps | no inflow | blend64 | 0.00 | 24.6 | 93 | 214 / 223 / 203 | 3,811 |
| alps | inflow | hard | 0.01 | 171.9 | 481 | 199 / 223 / 180 | 4,972 |
| alps | seq | hard | 11.70 | 192.4 | 337 | 175 / 223 / 78 | 28,826 |
| alps | ring (oracle) | hard | 0.01 | 165.8 | 410 | 196 / 223 / 179 | 4,929 |
| alps | ring (oracle) | blend64 | 0.00 | 24.7 | 93 | 216 / 223 / 204 | 3,770 |
| blackforest | no inflow | hard | 0.00 | 133.8 | 399 | 269 / 281 / 222 | 8,413 (1,115) |
| blackforest | no inflow | blend64 | 0.00 | 27.4 | 108 | 296 / 281 / 233 | 9,043 |
| blackforest | inflow | hard | 0.00 | 133.8 | 400 | as no inflow | as no inflow |
| blackforest | seq | hard | 7.08 | 101.2 | 239 | 267 / 281 / 134 | 28,927 |
| blackforest | ring (oracle) | hard | 0.00 | 83.4 | 286 | 277 / 281 / 225 | 8,145 |
| blackforest | ring (oracle) | blend64 | 0.00 | 20.7 | 85 | 294 / 281 / 232 | 8,695 |
- After the carve alone (before thermal), the hard step is p99 189 m, max 1,221 m (alps), and p99 150 m (blackforest); thermal trims the tail.

|mosaic - joint| p99 (m) by distance from the nearest core edge (hard, no inflow, carve and thermal; p50 is 0.00 in every band):
| Site | 0-1 | 2-8 | 9-32 | 33-128 | 129-512 | over 512 cells |
|---|---|---|---|---|---|---|
| alps | 136 | 142 | 165 | 133 | 120 | 14 |
| blackforest | 153 | 134 | 137 | 109 | 63 | 15 |

### Look (`images/`)
- `alps-seams.jpg`, `blackforest-seams.jpg`: hillshade of 1,024^2 cells (104 km) round the corner where the four cores meet. Columns: joint, hard mosaic, blend64, and |hard - joint| (0 to 50 m, red).
- The differences are whole river trees, not lines along the seams. Valleys the joint bake cuts across tiles are shallower or missing in the mosaic, and the error runs far into the cores.

## Findings
1. **Tiled baking with a 256-cell halo is not close to the joint bake.** Most cells agree exactly (p50 0), but the step across core edges is p99 134-172 m, max 400-480 m after thermal (up to 1,221 m after the carve alone). The 1 m p99 threshold of PR 4 is missed by two orders of magnitude.
2. **The error is routing, not the seam line.** It follows whole channel networks: p99 63-120 m even 129-512 cells from the edge, 14-15 m beyond 512 cells. Where a depression spills and which way a large river runs depend on terrain outside the window, so a tile cuts different valleys than the whole.
3. **Inflow at the edge does nothing measurable,** even taken from the joint routing at full resolution. The entering water meets the window edge, which is base level, and leaves again.
   - SPIKE-61's tile code put its inflow on edge cells, which are outlets. That inflow never entered its routing, so SPIKE-61's timings were in effect without inflow.
4. **An oracle edge ring does not fix it either:** p99 166 m (alps) and 83 m (blackforest). So a coarse global carve that only supplies tile edge heights would not be enough.
5. **Holding finished neighbours fixed is worse:** median step 7-12 m, about 29,000 ponds per site. The held cells are already uplifted while the tile's own start at the base, so the seam becomes a dam.
6. **The 64-cell blend hides the step** (p99 21-27 m, max 85-108 m) but not the misplaced channels. It is a cosmetic fix, not a seam fix.
7. **For ADR 0088 and the m9-bake change:** the transient carve's routing must be global, or at least global for the large rivers:
   - **(a) Global routing first.** Route once per face (or planet) on a coarser grid. Impose its receivers on every tile cell above an area threshold, so large rivers take the same path in every tile. Route locally only the small catchments that fit inside the halo. This needs one more spike (the threshold, and the coarse grid against the 100 m one).
   - **(b) Bake by drainage basin, not by square tile.** Basins can be larger than any window; continental basins need (a) anyway.
   - **(c) Run the carve at about 400-800 m for the whole planet,** a face at a time (12,288^2-24,576^2 cells). Keep only thermal and deposition per tile: they are local, with a reach of 100 passes, under 100 cells.
   Option (c) is the simplest and needs no new invariant; it moves R4's time estimate. The carve's own scale question (how much of the relief SPIKE-65 measured at 100 m survives a 400-800 m carve) is the next measurement.

## Open points
- Deposition was not part of the bake (SPIKE-64's sweep is local to a route and would inherit the routing difference).
- The carve is aggressive at the Alps: the joint bake changes heights by p99 1.9 km at x6. A gentler carve gives smaller absolute steps but the same mechanism.
- Two sites, one 2 x 2 block each; the window is not aligned to SPIKE-61's tile grid, which does not matter for the mechanism.
- Thread counts were not varied: the prototype is single-threaded except thermal, a Jacobi gather.

## Visuals
- `images/alps-seams.jpg`, `images/blackforest-seams.jpg`: joint / hard / blend64 / |hard - joint|, 104 km round the four-tile corner.

## Handover
- **Dump:** `proto/dump.sh.txt` on branch `spike/m9-relief-source` (RELIEF_W=6656).
- **Build:** the crate from `proto/s68seam.rs.txt` (as `src/main.rs`) and `proto/Cargo.toml.txt`; `cargo build --release`.
- **Run:** `s68seam <dump dir> <out dir> alps,blackforest`, with `S68_MODE` unset (no inflow and inflow), `seq` or `ring`; under the heavy lock at BelowNormal (`proto/run.sh.txt`).
- **Pictures:** `python -I proto/render.py.txt <out dir> <sheet dir> <site> joint,hard,blend64`.
- The full-window results (`<site>-joint|hard|blend64|seq-hard|ring-hard.f32`) are in `SeriousPlanetData/spike-r3/relief68/out/` (about 1.8 GB; delete after R4's design is settled).
