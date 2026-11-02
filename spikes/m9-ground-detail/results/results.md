# SPIKE-63 B8: ground detail at 1.8 m (R6 as a spike)

Opened: 2026-10-31
Question: What do G3 (the shader detail, REND-034) and G1 (height octaves below 12 m) under two amplitude laws look like at 1.8 m at three fixed sites, and what does each cost in GPU time?
Why it matters: M9 criterion 4 (the owner's look at 1.8 m) and R6; SPIKE-54 found G1 at persistence 0.55 adds only 3 cm.
Time box: 3 h (opus-planner, 2026-10-31); used about 1.5 h, with 40 min of machine time.
Success criteria: one side-by-side per site for the owner; GPU time per variant, measured headless.
Fallback if it fails: report what blocks the stills.
Code location: branch `spike/m9-ground-detail` (pushed, no PR; commit 6c44f5bb; spike code, never merged).
- `erosion.rs`: the G1 term, `PLANET_G1_LAW=a|b`.
- `stills.rs`: `SPIKE63_COST=1` times the terrain pass after each still.
Closed: 2026-10-31

## Method
**Sites** (store `global16384`, read only):
| Site | Latitude, longitude | Ground | Heading |
|---|---|---|---|
| hills-b | 8.972, -16.238 | 3,755 m | 90 |
| river-r1 | -11.1657, 23.0493 | 286 m | 131 |
| alps (SPIKE-55's first Alps match) | 11.5, -18.5 | 1,316 m | 0 |

**Poses:** eye 1.8 m, 60 m behind the anchor, 2560 x 1440:
| Pose | Pitch | Sun (local hours) |
|---|---|---|
| eye-midmorning | 6 degrees down | 10 |
| eye-lowsun | 6 degrees down | 17.25 |
| feet-lowsun | 35 degrees down | 17.25 |

**G1:** a term added to the eroded field below its finest crease octave (12 m).
- Five gradient-noise octaves: 6, 3, 1.5, 0.75 and 0.375 m.
- Scaled by a share `0.3 + 0.7 x` the fine creases' amplitude fraction: 0.3 to 0.4 on plains, 1 on hills in a range zone.
- The tile bounds and the largest height are widened by the term's largest magnitude.

| Law | 6 m octave | Ratio per octave | Largest magnitude | Character |
|---|---|---|---|---|
| off | - | - | 0 | today's field (SPIKE-54's 0.55 law adds at most 3 cm) |
| a | 0.25 m | 0.5 | 0.48 m | every octave the same slope |
| b | 0.12 m | 0.71 | 0.39 m | finer octaves steeper (rough) |

**G3:** today's REND-034 term, at the window's natural strength (0.6) and at full (1.0); hills-b also with G3 off.

**Captures:** `test-render --scene capture-stills` on the RTX (Vulkan), one process per site, law and strength (21 runs).
- 07:44 to 08:04 and 08:18 to 08:20, under the heavy lock, at BelowNormal.
- CPU load 0 to 10 % before each run (`raw/driver.log`).

**GPU time:** after each still at strength 1.0, the terrain pass is timed by REND-044's variants:
- the product shader;
- the shader with the shadows, the ground detail, or both compiled out.
- Each variant is the median of 3 round medians of 41 frames.
- Not a perf window and not `cargo xtask perf`: no readiness gate. The machine was quiet, so the values are indicative.

**Pixel differences:** the mean absolute luma difference (0-255) of G1 a and b against G1 off, in three bands of rows (`raw/diff.txt`).

## Results
### Look (`images/`)
Each sheet has columns for G1 off, law a and law b. Its rows are the poses at G3 natural and G3 full. Cells are the lower middle of the still (the near ground), halved.
- **G1 alone** (hills-b, G3 off):
  - Today's ground is smooth facets with crisp crease edges and a sawtooth shadow edge.
  - Laws a and b turn it into bumpy ground at 3-80 m, most visibly in low sun.
  - Law b is lumpier at the near metres.
- **G1 with G3:**
  - In low sun at 10-80 m, G1 breaks up the flat facets and the straight crease bands that G3 does not hide.
  - At the feet, G3's normals dominate. G1 shows only as a slow change of tilt, and a and b are hard to tell apart.
  - At midday, all variants look alike beyond about 10 m.
- **G3 at the feet (both strengths):** the 3 cm octave reads as an axis-aligned grid of stretched blocks (`images/alps-feet-g3full-crop.jpg`, 1:1). This is an artefact, the most visible defect at 1.8 m, and it does not depend on G1.
- **The sites:**
  - The Alps match is a smooth meadow (SPIKE-55: generator mountains are 4-9x too gentle).
  - river-r1 is a plain, so G1's share is low there (0.3-0.4).

Mean absolute luma difference against G1 off, G3 natural, law a / law b:
| Site | eye-lowsun mid band | eye-lowsun near band | feet-lowsun near band |
|---|---|---|---|
| hills-b | 10.8 / 11.2 | 12.9 / 12.8 | 16.7 / 14.6 |
| river-r1 | 3.5 / 3.9 | 6.3 / 5.9 | 12.8 / 13.1 |
| alps | 5.8 / 6.2 | 9.2 / 8.5 | 14.0 / 13.7 |
- The large feet values are mostly G3's fine pattern shifted by the new heights, not visible relief.

### GPU (terrain pass, ms, 2560 x 1440; strength 1.0)
| Site | Pose | Full pass: G1 off / a / b | G3 share (compiled out): off / a / b | Shadows share (G1 off) |
|---|---|---|---|---|
| hills-b | eye-midmorning | 3.51 / 3.37 / 3.42 | 1.47 / 1.32 / 1.29 | 0.63 |
| hills-b | eye-lowsun | 3.35 / 3.41 / 3.46 | 1.30 / 1.37 / 1.35 | 0.28 |
| hills-b | feet-lowsun | 2.48 / 2.42 / 2.49 | 0.95 / 0.92 / 0.94 | 0.34 |
| river-r1 | eye-midmorning | 2.86 / 2.86 / 2.83 | 1.16 / 1.18 / 1.12 | 0.46 |
| river-r1 | eye-lowsun | 2.92 / 2.89 / 2.84 | 1.18 / 1.22 / 1.14 | 0.39 |
| river-r1 | feet-lowsun | 2.13 / 2.09 / 2.12 | 1.00 / 0.96 / 0.98 | 0.26 |
| alps | eye-midmorning | 2.54 / 2.85 / 2.91 | 0.81 / 1.01 / 1.09 | 0.35 |
| alps | eye-lowsun | 2.79 / 2.88 / 2.90 | 1.05 / 1.13 / 1.17 | 0.26 |
| alps | feet-lowsun | 2.21 / 2.28 / 2.22 | 0.94 / 0.99 / 0.89 | 0.25 |
- **G1 costs no GPU time.** It changes heights, not vertex counts or shader work. The differences lie within the round spread (up to 0.53 ms; alps eye-midmorning +0.3 ms in both laws is the one case above 0.15 ms).
- **G3 at full strength costs 0.81-1.47 ms (median 1.12).** This is above REND-034's +1.0 ms in 17 of 27 values.
  - The share is measured with the code compiled out (material weights, tone and gloss included).
  - M7 measured it with the strength at 0 at runtime (+0.70/+0.90 ms, #534), so the two do not compare directly.
- **The whole terrain pass is 2.1-3.5 ms**, inside terrain's 5.0 ms share (BUDG-002), with shadows on today's horizon maps.
- **CPU cost of G1** (not measured here): SPIKE-54 measured +0.35 us per height read per octave, so five octaves add about +16 % per tile build.

## Findings
1. **G1 is visible where G3 is not:** at 3-80 m in low sun it replaces smooth facets and straight crease bands with bumpy ground; at the feet G3 dominates.
2. **Laws a and b look much alike with G3 on.** Without G3, b is lumpier near the eye. Law a has the larger magnitude (0.48 m against 0.39 m), and both scale with the fine-crease share, so they are low on plains.
3. **G3's 3 cm octave shows as a block grid at the feet.** This is today's most visible defect at 1.8 m, and R6's first G3 fix: band-limit or replace the finest octave.
4. **G3 at full strength is at or over its 1.0 ms budget** (median 1.12 ms, compiled out). G1 adds no GPU time.
5. **For R6:** G1 law a or b behind a switch, the 3 cm octave fixed, and a GPU check of G3 in a loaded pass (ADR 0087).
6. **For the owner:** pick a law from the sheets. On the evidence here G1 is worth its CPU cost (+16 % tile build) for the 3-80 m range.

## Open points
- Physics and bounds with G1 were not tested: held-back tiles and the gate (SPIKE-53 E1) are R6's check.
- Determinism across thread counts was not tested (`libm` and integer-hash noise, as SPIKE-54).
- Only one Alps match: the generator has no Alpine relief yet (SPIKE-55), so the mountain look waits for R4.
- GPU values are ungated (quiet machine, no perf window).

## Visuals
- `images/hills-b.jpg`, `images/river-r1.jpg`, `images/alps.jpg`: per site, columns G1 off / law a / law b; rows the three poses at G3 natural and G3 full.
- `images/hills-b-g3-off.jpg`: hills-b with G3 off, G1 off / a / b: what G1's geometry adds alone.
- `images/alps-feet-g3full-crop.jpg`: a 1:1 crop of the feet pose at G3 full, the block grid of the 3 cm octave.

## Handover
- **Rerun** from the worktree root on branch `spike/m9-ground-detail`, one process per law and strength; the poses files are in `raw/poses/`:
  ```
  PLANET_G1_LAW=a SPIKE63_COST=1 target/release/planet.exe test-render --scene capture-stills --adapter hardware --store C:/DEV/SeriousPlanetData/global16384 --poses raw/poses/hills-b-d1.0.toml --out-dir target/s63/hills-b/g1-a-d1.0
  ```
- Full stills: `target/s63/` in the spikes worktree (not in git).
