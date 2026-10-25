## Why
M8-6 (m8-plan; ADR 0076 proposed; SPIKE-30 D1; SPIKE-32 step 3): ground materials follow from physical maps, not only from height and climate. The seams exist: `MaterialParams::weights` (LOOK-008) takes eight inputs and three are reserved and read 0 (soil depth, moisture, insolation); a compact tile carries a 9 x 9 climate block the shader samples (GEN-021, GEN-025); the chunk stage yields the final height and the drainage area (`Solution::area_m2`, the stored coarse inflow included) per 8 m cell. ADR 0076 stays proposed; building the three slots on it is easy to reverse because the slots default to neutral values that leave today's look unchanged (planner, 2026-10-26).

## What Changes (in the order of the build)
1. **Drivers module** `generators::drivers` (GEN-070): pure `libm` functions of the final chunk raster, per core cell from the cell and its eight neighbours at 8 m: slope, mean curvature, **wetness**, **soil depth**, **insolation**. Constants in one struct, part of the stage's definition hash. No threads, fixed order (CON-14).
   - Wetness `W = ln(A / (w tan s))` with `A` the drainage area, `w` the cell width and `s` the slope, **tan s floored at 0.01** (0.57 degrees: flats and carved beds would otherwise send W to infinity) and **`A` clamped to 1e8 m2** (the river threshold: a river cell is as wet as a cell can be); stored as `moisture = clamp((W - 4) / 12, 0, 1)` (W from 4 on a divergent flank to 16 in a convergent hollow with the clamped area: ln(1e8 / (8 x 0.01)) = 20.9, ln(64 / (8 x 1)) = 2.1).
   - Soil depth: the SPIKE-30 stand-in, 2.5 m times a slope factor exp(-slope / 18 degrees), a curvature factor 1 + 0.5 tanh(curvature), a flow factor 0.5 to 1 from log10 of the area, a lithology factor 0.5 to 1.5 (position-only noise bands) and an altitude factor falling to 0 from 2,500 to 4,500 m, clamped 0 to 8 m; stored square-root coded in 8 bits.
   - Insolation: **the annual mean** at the cell's latitude for its slope and aspect (a fixed tilt formula, 0 to 1 of the flat value at the equator), **never the current sun**: the lit pass already shades by the sun and a second use would darken slopes twice.
2. **Chunk channels and the range blend** (GEN-071): `ChunkResult` gains three u8 channels at the raster (soil depth, moisture, insolation; +1.1 MB for 608 x 608 cells) with box-filtered pyramid levels, kept in the chunk's disk record; `ANET_VERSION` re-keys them. A consumer reads a driver as `neutral + w (chunk - neutral)` with `w` the same chunk weight that fades the height (the chunk fade, STRM-0xx), so materials fade in with the relief and never pop at the chunk range edge. Neutral values: soil 1 m, moisture 0.5, insolation from latitude alone.
3. **Materials, CPU side** (GEN-072): the reserved inputs of `MaterialParams::weights` read the three drivers; with the neutral drivers the weights, the albedo and every golden of today are bit-identical (GEN-073 below), so no existing golden moves.
4. **After crusader's m8-chunk-fade archive merges**: the tile driver block (second 9 x 9 block of three unorm8 channels in one word per texel, 324 B, slot 9,168 to 9,492 B, `MESH_VERSION` 10), the shader read, and the debug views (soil, wetness, insolation). I tell crusader when this step starts; it touches `compact.rs` and the slot.

## Out of scope (stays in ADR 0076)
Suitability rules and instances, sediment, authored driver offsets, a stored global flow and soil grid (the chunk's area replaces it inside chunks; outside, the neutral values apply), the photo layer.

## Tests
- Tier A: each driver on a synthetic raster with a known answer (a plane gives its slope, a cone its curvature, a hollow with a clamped area the maximum moisture, a flat the floor); one hash at 1 and 8 threads; pyramid levels are box means; the blend is the neutral value at weight 0 and the chunk value at weight 1 and monotone between; neutral drivers give the weights and albedo of today bit for bit.
- Tier B/C (step 4): the shader reads the block and agrees with the CPU function within the bound; debug views auto-blessed under ADR 0061; the new lit looks go to the owner's /bless through `golden-review`.
- numerics-reviewer on `drivers.rs` before merge; verifier.

## Risks
- Soil and moisture are stand-ins (SPIKE-30): constants exposed, calibration is the owner's look review.
- `compact.rs`, the slot size and `MESH_VERSION` are shared with the chunk-fade work (step 4 waits for it).
- Cost: one sweep over 370k core cells per chunk (expected well under 0.1 s, to be measured in the PR), +1.1 MB per resident chunk, +324 B per tile slot.
