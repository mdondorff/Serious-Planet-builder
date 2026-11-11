# SPIKE-106 round 2, addendum: G-SURF-I on tiles with a river over 200 m3/s

Run 2026-11-11 by the spikes lane on the bulldozer's slot. Reported, not gated (architect, 2026-11-11). Main c5305536 and later (the units fix; the width law GEN-165 and the lake producer are in). Release build, Windows, 8 threads, BelowNormal, margin 200 regional cells; about 30 s a tile; other load on the machine was possible (the slot allowed the explorer's prepush to start meanwhile; this is not a perf measurement). Probe: `crates/generators/examples/gate_surf.rs` on the local branch `spike/gate-remeasure` (commit 0e59c594), spike code, never merged. Raw output: `gsurfi-reread/`.

## Correction to section 3 of `remeasure-2026-11-10.md`

The G-SURF-I numbers of that section (179 vertices over 0.25 m, maximum 2.76 m, burn count 95,820,316, "moved since the ledger rows") were measured on a copy of `k7_region` from before the units fix: H1 got no runoff, so `Q` was an area in m2, the width law turned areas into trenches of kilometres, and the numbers are the units bug, not a change of K7. With the fix the prototype tile (65, 7) reproduces the ledger row of ea6b1b2b (below). The section's other findings stand (own measurement equals the library's; the vertex sets differ by 170; the ledger's step classes do not match `nodes_by_step`).

## 1. Where the rivers over 200 m3/s are (the prototype region, seed 14 face 1)

`gate_surf` with `PLANET_LIST=1` (25 s): H1 on the region's solved surface, channel cells on land by local tile (1,024 cells = 128 regional cells; a local cell is 101.8 m).

| Tile | Peak Q (m3/s) | Width law 7.2 Q^0.5 | Channel cells | Cells at 200 m3/s or more |
|---|---|---|---|---|
| (65, 8) | 735 (regional cell (8378, 1105)) | 195 m = 1.92 cells | 1,141 | 106 |
| (64, 8) | 502 | 161 m = 1.59 cells | 1,059 | 195 |
| (63, 8) | 404 | 145 m = 1.42 cells | 462 | 41 |
| (64, 7) | 400 | 144 m = 1.41 cells | 964 | 65 |
| (65, 7), the prototype tile | 161 | 91 m = 0.90 cells | 254 segments | 0 |

8 of 61 tiles hold a channel cell over 200 m3/s. The widest trench of the whole region is 1.92 cells; the 9,376 m3/s river of region-12288's 4,000-margin window (a trench of about 700 m = 6.9 cells) is not in this region.

## 2. G-SURF and G-SURF-I (own code; the library's `surface_gate` beside)

| Tile (peak Q, law width) | Centres, before / after quantisation | Vertices | Over 0.25 m (own / library) | p50 / p99 / max (own) | Overshoot of the runtime read |
|---|---|---|---|---|---|
| (65, 7) (161 m3/s, 0.90 cells) | 5.7e-14 / 0.125 m | 28,730 / 28,900 | 189 (0.66 %) / 192 (0.66 %) | 0.040 / 0.224 / 2.554 m | 2.63 m, 38 vertices over 0.25 m |
| (65, 8) (735 m3/s, 1.92 cells) | 1.4e-14 / 0.125 m | 28,900 | 1 (0.00 %) / 1 | 0.019 / 0.109 / 0.617 m | 0.18 m, 0 over |
| (64, 8) (502 m3/s, 1.59 cells) | 5.7e-14 / 0.125 m | 28,730 / 28,900 | 20 (0.07 %) / 20 | 0.033 / 0.117 / 1.017 m | 0.45 m, 4 over |

- G-SURF at the centres passes on all three (0 before quantisation, half a step after).
- (65, 7) reproduces the ledger row of ea6b1b2b: 192 of 28,900 vertices (0.66 %) over 0.25 m, maximum 2.55 m (library); the own measurement counts 189 of its 28,730 (the vertex sets differ by 170, see the correction).
- `B` at the vertices, own lattice value against `base`: at most 5.7e-14 m. The stored change equals `(z - b)` as f32 on all 1,048,576 core cells of each tile.

## 3. Vertices by trench class

Classes from the probe's own geometry: in the law's trench = within the law's half width plus half a cell of a channel whose law width is 1.25 cells or more (68 and 65 such channels on the two tiles); in a burnt cell of a narrower channel; outside any trench.

| Tile | In the law's trench (vertices, over 0.25 m, p99, max) | In a burnt cell of a narrower channel | Outside any trench |
|---|---|---|---|
| (65, 8) | 130, 0, 0.099, 0.106 m | 245, 0, 0.108, 0.178 m | 28,525, 1 (0.00 %), 0.109, 0.617 m |
| (64, 8) | 108, 5 (4.63 %), 0.321, 0.335 m | 174, 0, 0.119, 0.129 m | 28,448, 15 (0.05 %), 0.116, 1.017 m |
| (65, 7) | none | 191, 2 (1.05 %), 0.228, 0.819 m | 28,539, 187 (0.66 %), 0.224, 2.554 m |

## 4. The trench width, law against burnt, in metres and cells

For the five widest channel stretches of each tile: the burnt width is twice the distance from the centreline at which the share of burnt cells (the burn mask of K7) falls below one half, over the stretch of 5 points around the peak `Q`.

| Tile | Law width (peak Q) | Burnt width at the sampled stretches |
|---|---|---|
| (65, 8) | 195 m = 1.92 cells (735 m3/s) | 255 m = 2.5 cells, 305 m = 3.0 cells, 255 m = 2.5 cells; 0 at two stretches (burnt share 18 % and 11 % even within the law's half width) |
| (64, 8) | 161 m = 1.59 cells (502 m3/s) | 102 m = 1.0 cell twice, 305 m = 3.0 cells, 255 m = 2.5 cells; 0 at one stretch (25 %) |

Reading: where the burn is measurable it is as wide as or wider than the law (1.0 to 3.0 cells against 1.6 to 1.9; the burn takes the cells whose centres lie within half the width of an 8-connected line, so a width a bit above the law is expected). At 3 of the 10 sampled stretches the burnt share at the line is under one half: the burn never raises a cell, and the cause (cells already at or below the profile, or the line of another branch) was not isolated. The width is a sampled reading, not a statistic.

## 5. Findings and caveats

- **The width law does not reach a visible size on this region.** Its widest trench is 1.9 cells. On the wide-trench vertices G-SURF-I is 0 over 0.25 m of 130 on (65, 8) and 5 over of 108 on (64, 8) (maximum 0.335 m): small samples, not different from the tile's other vertices in kind.
- **The tiles with rivers are better than the prototype tile on G-SURF-I** (0.00 % and 0.07 % over 0.25 m against 0.66 %), and on the prototype tile 187 of its 189 vertices over 0.25 m lie outside any trench, so its rise is not the trench of a river (the probe did not look into what causes it).
- The maximum of (64, 8), 1.02 m, is a vertex outside any trench (15 over 0.25 m outside); the probe did not look into its cause.
- A trench several cells wide (a river over about 1,000 m3/s) is not covered: seed 15 face 4 (mouth 1,561 m3/s) and face 2 (2,104 m3/s) were not run (the region here had rivers over 200 m3/s, so the fallback was not needed). A run on one of them would test a 3 cell trench.
- Reported, not gated. Three tiles of one region and one seed; the vertex set of the own measurement (169 x 170) differs from the library's (170 x 170) on two tiles.

## 6. Reproduce

```
cargo build --release -j 4 -p planet-generators --example gate_surf    # from spike/gate-remeasure
PLANET_LIST=1 gate_surf 200 8                                          # the tile list of section 1
PLANET_TILE=65,8 gate_surf 200 8                                       # then 64,8 and 65,7
PLANET_REGION=15,4,368,4720,620 PLANET_TILE=6,39 gate_surf 200 8       # the seed 15 face 4 fallback (not run)
```
