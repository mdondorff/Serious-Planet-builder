# SPIKE-106 round 2, addendum: G-PROFILE read on the finished K7

Run 2026-11-11 by the spikes lane on the bulldozer's slot. Reported, not gated. Main c5305536 and later (units fix in). Release build, Windows, 8 threads, BelowNormal, margin 200 regional cells; 62 to 131 s a tile in the final runs (an earlier run of one tile took 702 s: other load on the machine, which the slot allowed). Probe: `crates/generators/examples/gate_surf.rs`, local branch `spike/gate-remeasure` (commit 879d4bf7), spike code, never merged. Raw output: `gprofile/`. Tiles: (65, 8) and (64, 8) of face 1 seed 14 (the tiles of the G-SURF-I re-read, rivers up to 735 and 502 m3/s) and the prototype tile (65, 7).

## Result

**The channel's bed rises downstream by more than 0.25 m on the finished K7 surface, on most segments of every tile, and the rise is not made by thermal, deposition or the pit spreading.** G-PROFILE (A3 section 12.1: along every segment, at the local grid's nodes, no rise over 0.25 m, `C` off) does not hold on these tiles by this reading.

| Tile | Segments with samples in the core (of those meeting the window) | Over 0.25 m by the rise above the lowest upstream cell | Over 0.25 m by a single step up | Largest rise / largest step |
|---|---|---|---|---|
| (65, 8) | 360 (461) | 147 | 61 | 1.40 / 1.40 m |
| (64, 8) | 302 (456) | 176 | 164 | 14.68 / 12.95 m |
| (65, 7) | 185 (254) | 141 | 139 | 25.27 / 17.62 m |

Method (own code, written from GEN-148 and A3 section 12.1): the path of a segment is the cells GEN-148 burns, `round(point)` at `ceil(max(|dx|, |dy|))` steps of each link of the segment's polyline (consecutive duplicates removed); the bed is the surface `B + L` with `L` quantised to 0.25 m (what the sampler returns at a cell centre) at those cells; only the cells two in from the core's edge count, a segment cut by the edge gives pieces. The rise of a segment is the largest height of a path cell above the lowest path cell upstream of it, also the largest single step up. (A first version took the lowest of the 2 x 2 cells around each sample; it was noisier, I discarded it; the picture was the same.)

## Where the rises are

- Most of the large rises are on headwater segments (peak `Q` of 1 to 6 m3/s): 25.3 m on a Q = 5 segment of (65, 7), 14.7 m on a Q = 3 segment of (64, 8), 1.4 m on a Q = 2 segment of (65, 8).
- Rivers of 150 m3/s and more:

| Tile | Segment (peak Q) | Rise (m) |
|---|---|---|
| (65, 8) | the five widest stems, Q = 735 | 0.116, 0.111, 0.257, 0.221, 0.441 |
| (65, 8) | stem, Q = 725 | 1.055 (single step 0.334) |
| (65, 8) | Q = 161 | 1.126 |
| (64, 8) | the five widest stems, Q = 502 | 0.230, 0.128, 0.143, 0.238, 0.272 |
| (64, 8) | the four largest of Q = 452 to 464 | 9.35 (Q 452), 6.87 (455), 5.08 (464), 4.95 (455) |
| (65, 7) | Q = 156 to 161 | 0.246 to 0.976 |

So the widest stems of the re-read (Q 502 and 735) are at or just over the 0.25 m line (up to 0.44 m); the rivers of 450 to 460 m3/s of (64, 8) rise 5 to 9 m.

## Which stage

The same measure with a stage off, and with only the early stages on (the surface at the end of each):

| Surface | (65, 8) largest rise, segments over 0.25 m | (64, 8) | (65, 7) |
|---|---|---|---|
| Finished K7 | 1.40 m, 147 | 14.68 m, 176 | 25.27 m, 141 |
| Thermal off | 1.40, 147 | 14.68, 176 | 25.27, 141 |
| Pit spreading off | 1.40, 147 | 14.68, 176 | 25.27, 141 |
| Deposition off | 1.33, 141 | 17.14, 171 | 26.17, 145 |
| Thermal, deposition and pits off | 1.33, 141 | 17.14, 171 | 25.27, 145 |
| Burn, breach and incision | 1.33, 141 | 17.14, 171 | 25.27, 145 |
| Burn and breach | 2.80, 191 | 10.89, 135 | 32.96, 132 |
| Burn only | 2.74, 146 | 10.89, 138 | 32.41, 136 |

- **Thermal and the pit spreading change nothing** at the maximum or the counts. **Deposition moves the counts by 4 to 6 segments** and the maximum by under 3 m; with it off the rises are as large.
- **The rises are already in the burn-in surface** (136 to 146 segments over 0.25 m after the burn alone, up to 32 m on the prototype tile). The burn only lowers: each path cell gets `min(own height, level)` with the level the regional running minimum along the segment. A cell already below its level, or lowered by the trench of a neighbouring or crossing channel (a confluence, a parallel branch), stays low, so the path's heights are not monotone. In the path heights printed in `gprofile/tile-64_8.txt` (burn only) a headwater path reads 177.7, 169.6, 167.2, 171.8, 178.1 m downstream.
- **The breach and the incision change the picture but do not cure it:** the incision takes the rise down (65, 8: 2.80 to 1.33 m; 65, 7: 32.96 to 25.27 m) or up (64, 8: 10.89 to 17.14 m). The incising carve lowers each cell by its own drainage area and slope, so two path cells can differ by several metres (a path reads 248.6 m then 252.7 m after the incision on a Q = 6 headwater).
- **One location is made after the incision:** on the Q = 452 river of (64, 8) the path cells (168, 504) to (172, 500) read 0.00 to 0.43 m after the incision and 1.7 to 7.9 m in the finished surface (a single step of 6.1 m). That is the thermal, deposition or pit stage; the per-stage runs do not split it, since removing one stage moves the worst place of the segment.

## Caveats

- Reported, not gated. Three tiles of one region and one seed; the path is the polyline's rasterisation, and a path cell can lie in a trench that another channel made; whether water would follow the bed (a D8 route on the finished surface) was not measured. The rises of the headwaters are the largest and carry little discharge.
- "Deposition off" is `kc = 1e18` (a capacity far above any load by the formula in `deposit.rs`); I did not read the deposition report to confirm that nothing is deposited. "Pit spreading off" is zero passes, "thermal off" zero passes.
- Stages are cumulative surfaces at the end of the burn, the breach (carve with `k_dt` 0, one step) and the incision; the breach variant uses the standard breach limit, the burn-only variant a limit of 0.
- Other load on the machine was possible; this is not a perf measurement.

## Reproduce

```
cargo build --release -j 4 -p planet-generators --example gate_surf    # from spike/gate-remeasure
PLANET_PROFILE=1 PLANET_TILE=65,8 gate_surf 200 8                       # then 64,8 and 65,7
```
