# SPIKE-83: the bounds pyramid

Opened: 2026-10-31
Question: does SPIKE-82's lever work as built? A min/max pyramid per 32 x 32 block of the erosion layer, plus analytic bounds for the runtime detail: its size, build time and lookup cost per cap, and SPIKE-82's plans at the Himalaya and Alps poses with the pyramid's bounds (capped leaves, node counts).
Why it matters: SPIKE-82 found that the steep classes stop refining at level 13-15 with today's style of bounds; R4's layer format must carry the fix.
Time box: 2 h (opus-planner, 2026-10-31); used about 45 min, with under 1 min of machine time.
Success criteria: size, build time and lookup cost; capped leaves and node counts with the pyramid's bounds; no bound that misses a sample.
Code location: branch `spike/m9-plains` (commit dd972a59), the ignored test `spike83_pyramid` in `crates/app/src/plancheck.rs` (`proto/spike83_pyramid.rs.txt`, `proto/run.sh.txt`), with `global::spike83_detail` and `spike83_detail_bounds`. Spike code, never merged.
Closed: 2026-10-31

## Method
- **Baked layer (stand-in for ADR 0088's erosion layer):**
  - SPIKE-82's steep combined field, sampled on SPIKE-61's face grid (98,304 cells per face edge, 101.8 m) over a 2,048^2-cell window (208 km) round each site, stored as f32 nodes and read bilinearly;
  - outside the windows the field falls back to SPIKE-82's `Steep` (coarse tiles far from the camera).
- **Runtime detail:** one octave of gradient noise at about 25 m, amplitude `1.5 k` m (up to about 11 m), with the generator's per-octave noise bounds over a cap (`layer_noise_bounds`).
- **Pyramid:** per 32 x 32 block the min and max of its 33 x 33 nodes (shared edges), then each coarser level from four blocks below, up to the window (7 levels).
- **Bounds per cap:**
  - the cap's face box (the angular radius times 2 in face units, as `LandformLattice::bounds`);
  - from the finest pyramid level whose blocks are at least half the box (at most about 3 x 3 blocks), or the raw nodes for boxes under 16 cells;
  - plus the detail's bounds.
  - Bilinear values lie between the nodes of their cell, so the bound holds by construction for the layer.
- **Plans:** as SPIKE-82 (`LodParams::earth_1080p`: cells 32, tau 6, finest 22; 200 m and 2 km above the ground; Himalaya site 2 (11.75, -111.25) and Alps site 1 (13.5, -117.25) of SPIKE-81). Each bounds call is timed. The bound against the sampled range of the centre tile per level is checked for misses.
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`).

## Results
### Plans with the pyramid's bounds (nodes, capped leaves, deepest level)
| Site | Altitude | Today (SPIKE-82) | Steep, production-style bounds (SPIKE-82) | **Steep layer, pyramid bounds** |
|---|---|---|---|---|
| Himalaya | 200 m | 1,022, 0, 18 | 625, 62, 13 | **1,075, 0, 18** |
| Himalaya | 2 km | 742, 0, 15 | 635, 62, 13 | **805, 0, 15** |
| Alps | 200 m | 1,039, 0, 18 | 968, 85, 15 | **1,094, 0, 18** |
| Alps | 2 km | 758, 0, 15 | 897, 28, 15 | **818, 0, 15** |
- **No capped leaf** at either site or altitude; the plans reach today's depth with 5-8 % more nodes than today.
- Plan time 0.02-0.09 s (SPIKE-82's steep field 0.2 s, today's field 0.1 s).

### Bound against the sampled range (centre tile)
| Level | 10 | 12 | 14 | 16 | 18 | 20 | 22 |
|---|---|---|---|---|---|---|---|
| Himalaya range (m) | 1,038 | 338 | 82 | 35 | 7.6 | 2.6 | 0.4 |
| Himalaya bound / range | 1.5x | 2.4x | 2.8x | 2.6x | 6.1x | 10.5x | 66x |
| Alps bound / range | 2.7x | 2.0x | 1.4x | 2.8x | 3.6x | 14.7x | 64x |
| SPIKE-82 steep, production-style (Himalaya) | 11x | 21x | 53x | 25x | 44x | 104x | 425x |
- No bound missed a sample.
- At levels 10-18 the pyramid's bounds are 1.4-6 times the range, against 11-53 times with production-style bounds.
- At levels 20-22 the detail's analytic bound dominates (up to about +-11 m on a range under 3 m). These levels are finer than the layer, and their absolute width does not cap a tile.

### Size, build time and lookup cost
| Measure | Window (2,048^2 nodes) | Planet (SPIKE-61: 1.72e10 land cells) |
|---|---|---|
| Blocks (all levels) | 5,461 | about 2.2e7 |
| Size as 2 x f32 | 43.7 KB (0.26 % of the f32 nodes) | about 180 MB |
| Size as 2 x u16 (rounded outward) | 21.8 KB | about 90 MB: about 1-2.5 % of the 7-10 GB layer |
| Build | 3 ms (one thread) | about 12 s (estimate, one thread); negligible against the 6-7.5 h bake |
| Lookup per cap | 2.5-4.0 us (pyramid, noise bounds and the `k` range) | 4,500-6,300 lookups per plan: 15-25 ms of plan time on one thread |

## Findings
1. **The pyramid fixes SPIKE-82's degradation:**
   - with min/max bounds from a 32 x 32-block pyramid of the baked layer, the steep Himalaya and Alps plan to level 18 with no capped leaf, at 5-8 % more nodes than today;
   - SPIKE-82 had level 13-15 and 28-85 capped leaves.
2. **It is cheap:**
   - about 90 MB for the planet as u16 pairs (1-2.5 % of the layer);
   - seconds to build after the bake;
   - 2.5-4 us per bounds call, so 15-25 ms per plan on one thread.
3. **The bounds are tight where the layer lives:** 1.4-6 times the sampled range at levels 10-18. Below the layer's 102 m, the runtime detail's analytic bound sets the width; it is wide in ratio but small in metres.
4. **Correction to SPIKE-82 and decision 1:** "0.2-0.3 % of the layer" compared against uncompressed f32 nodes. Against the stored layer (7-10 GB, SPIKE-61), the pyramid is about 1-2.5 %.

## Open points
- The layer is a stand-in: the steep field sampled at 102 m, not ADR 0088's bake; two 208 km windows, not a store. A real pyramid lives in the layer's blocks and is read with them.
- u16 quantisation (rounded outward) and the store's block layout are not built.
- Caps that cross a face edge or a window fall back to SPIKE-82's interval bounds. A planet pyramid needs the face-edge case: a cap that crosses takes both faces' blocks.
- The detail octave stands in for R6's runtime detail (G1/G3); its real amplitude and bounds come with R6.

## Handover
- On branch `spike/m9-plains`: `bash proto/run.sh.txt` (needs `target/s81/s81-fields.f32` from SPIKE-81; `S83_N` sets the window).
