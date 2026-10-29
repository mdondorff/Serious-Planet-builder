Opened: 2026-10-28
Question: What do CPU height octaves from 12 m wavelength down to 0.5 m (G1 of SPIKE-53) add to the tile build time, and do they change existing heights or determinism?
Why it matters: M9's ground detail plan (SPIKE-53 recommended G1 at an inferred +8 to 16 % tile build time); the number was not measured.
Time box: 2 h
Success criteria: ms per tile and % against baseline for 0, 2, 4 and 5 extra octaves at the finest levels; whether existing heights or determinism change
Fallback if it fails: none (measurement)
Code location: branch `spike/m9-g1-octave-cost` (local, not pushed; spike code, never merged); patch in `proto/g1.patch.txt`
Closed: 2026-10-29

## Key finding
- **G1 as specified costs 13 to 17 % per tile (4 to 5 extra octaves) and adds at most 3 cm of relief at this site**, with the current persistence of 0.55 and the normalisation of the first 12 octaves kept. A 6 m to 0.4 m relief of 3 cm does not read at 1.8 m. The cost is measured; the visibility is an observation of the height change at one site (not a render).
- The design question for M9 is the amplitude law of the new octaves (a slower decay than 0.55, or a separate detail term with its own amplitude), not the octave count: each extra octave costs about the same (0.35 us per height read) whatever it adds.

## Method
- Spike code only (never merged): `erosion.rs` adds `PLANET_EXTRA_OCTAVES` (read once per process): octaves 12 to 12 + K are added to the fine crease sum (wavelengths 6, 3, 1.5, 0.75, 0.375 m); the normalisation stays that of the first 12 octaves, so existing octaves keep their weights. Bounds are not extended (a plan check was not part of this spike). `libm` and the same seed throughout.
- Test `g1_cost_report` (`globalperf.rs`, ignored): store `C:/DEV/SeriousPlanetData/global16384`, no chunk stage (`world.field()`), tiles of level 22, 20 and 18 at the site (a tile and its 8 neighbours, 9 tiles per level), cells 32; per tile one warm-up round, then 5 rounds of `tile_mesh_field` (the batch height call as the streamer runs it) and of 1,089 serial height reads; median per tile. One process per octave count; the heights of all 27 tiles (1,089 vertices each) written as raw f64.
- Quiet machine: runs 04:13 to 04:17 on 2026-10-29; no other lane built or ran (explorer and crusader idle by arrangement, no perf window open), CPU load 19 % before the runs; one process at a time.
- **Absolute times are unverified (opus-planner, 2026-10-29):** the laptop's CPU was throttled to about 17 % of its clock from at least 05:22 that day, possibly since window 27 (00:00 to 04:12); these runs (04:13 to 04:17) probably ran throttled, which likely explains the 13 ms per tile against the 2 to 3 ms in `docs/learnings.md`. The percentages (+7 %, +13 %, +15 to 17 %) compare runs on the same machine state and stand; the heights and the determinism result do not depend on speed.

## Results
Mesh path per tile (median of 9 tiles x 5 rounds):
| Extra octaves | Level 22 | Level 20 | Level 18 | Against baseline (mean of levels) |
|---|---|---|---|---|
| 0 (run 1) | 13.09 ms | 13.08 | 13.08 | baseline |
| 0 (run 2) | 13.62 | 13.23 | 13.31 | +1.5 % (run-to-run) |
| 0 (run 3) | 13.35 | 13.15 | 13.12 | +0.9 % |
| 2 | 14.04 | 13.98 | 13.92 | +0.9 ms, +7 % |
| 4 | 15.04 | 14.76 | 14.76 | +1.7 ms, +13 % |
| 5 | 15.04 | 15.16 | 15.28 | +2.0 ms, +16 % (15 to 17 % by level) |

1,089 serial height reads (median): 11.2 ms baseline, 12.0 (+7 %), 12.6 to 12.7 (+13 %), 13.0 to 13.2 ms (+16 %): about 10.3 us per read, +0.35 us per extra octave.

Heights against the baseline (27 tiles x 1,089 vertices = 29,403 values; run 3 as reference):
| Extra octaves | max change | p99 | mean | unchanged values |
|---|---|---|---|---|
| 2 | 0.0235 m | 0.0203 m | 0.0074 m | 0 of 29,403 |
| 4 | 0.0289 m | 0.0222 m | 0.0089 m | 0 |
| 5 | 0.0295 m | 0.0226 m | 0.0092 m | 0 |

Determinism: two baseline runs gave bit-identical heights (sha256 prefix 578882e72b174638); thread-count independence was not tested.

## Open points
1. **13 ms per tile here against 2 to 3 ms in `docs/learnings.md`** (`tile_cost_report`): different cells (64 against 32), different levels (15 to 3 against 22, 20, 18) and a different run (chunk layer on); not reconciled. The relative cost (+7 %, +13 %, +16 %) is what this spike measured; the 13 ms is a single-thread wall time (the mesh path took 2 ms more than the serial reads, so the batch call did not use more than one thread here).
2. **Existing heights change by up to 3 cm** (every value moves, since the new terms add). Any G1 must keep the existing levels' heights bit-identical (octaves added only below the current finest octave's scale, or as a separate detail term that existing consumers do not read) or declare a golden change; no golden run was made here.
3. Thread-count independence not tested.
4. Not measured: the plan's bounds with the extra octaves, held-back tiles, goldens, a render of the added relief, other sites (the amplitude depends on elevation and slope: 0.0295 m is this site's maximum).
5. `docs/learnings.md` is at 20,398 of 20,480 bytes, so the learning on the amplitude finding is not added here; it needs a housekeeping run first.

## Visuals
Nothing to look at: CPU timings and height differences only, no renders.
