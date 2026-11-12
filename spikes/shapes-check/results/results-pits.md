# SPIKE-108 follow-up (2): why 21 to 38 % of the land is under the fill at round 0

Run 2026-11-12 (architect's follow-up, relayed by the bulldozer): report only, no production change. Question: are the lifted cells of the start surface noise pits or basins, and what would a round-0 breach change? Numbers; the verdict is the architect's. Run by the spikes lane.

Probe: `shapes_check pits` on the local branch `spike/spike-108-probe` (never merged), built on main b8c5bbb5 (before GEN-172 and GEN-173; the start surface z0 does not depend on either). Same fixtures as `results.md`: R1h (face 1, seed 14, box 620 cells), R3a and R3b (seed 15, box 1,020 cells), margin 200, 12,288 cells a face edge (814 m). DEM: the 15 GLO-30 sample tiles as in `results.md` (statistics only; produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30). Raw: `raw/pits-*.txt`, `raw/dem-all-pits.txt`.

Definitions: a lifted cell is one the depression fill raised by at least 0.5 mm; a depression is an 8-connected component of lifted cells; its size is its cell count and its depth the largest fill depth in it (m). Percentages are of the land cells of the box. U routing, canonical flood, epsilon 1 mm.

## 1. The lifted cells by depression size (z0)

| Depression size (cells; 814 m cells of 0.66 km^2) | R1h | R3a | R3b | DEM pooled (15 tiles) |
|---|---|---|---|---|
| 1 to 30 (noise pits) | 4.03 % | 3.49 % | 4.18 % | 5.84 % |
| 31 to 300 | 9.57 % | 6.61 % | 8.33 % | 2.33 % |
| 301 to 3,000 | 14.34 % | 5.88 % | 5.64 % | 0.97 % |
| over 3,000 | 8.35 % | 9.51 % | 8.58 % | 0.00 % |
| Total lifted | 36.28 % | 25.46 % | 26.72 % | 9.14 % |
| Depressions over 300 cells (count) | 171 | 117 | 139 | 3 |

- The pits of 30 cells or fewer are 3.5 to 4.2 % of land in the generator and 5.8 % in the DEM: no excess there.
- Depressions over 30 cells hold 22 to 32 % of the land (R1h 32.3 %, R3a 22.0 %, R3b 22.5 %) against 3.3 % in the DEM; those over 300 cells hold 14 to 23 % against 1.0 %.
- The DEM tiles are 100 x 138 cells at most, so a depression over 3,000 cells is possible there (none occurs); the per-tile lifted share (fill >= 1 m) is 1.0 to 17.5 % (median 6.5 %).

By largest fill depth (% of land):

| Fill depth | R1h | R3a | R3b | DEM pooled |
|---|---|---|---|---|
| under 1 m | 0.33 | 1.20 | 1.05 | 0.54 |
| 1 to 10 m | 2.34 | 7.85 | 9.71 | 3.38 |
| 10 to 100 m | 11.72 | 13.89 | 15.23 | 4.91 |
| over 100 m | 21.89 | 2.55 | 0.74 | 0.31 |

- R1h's depressions are deep: 21.9 % of the land lies in basins filled by more than 100 m (the largest 9 depressions, over 3,000 cells each, hold 8.4 %). R3a and R3b are shallower: most lifted cells are filled by 1 to 100 m.
- Reading: the lifted cells are the closed basins of the uneroded, uplifted base relief, not noise pits. The rounds drain them (after round 1 the stage probe's "land under the fill" falls to 4.4 %, after round 2 to 1.6 % in R1h).

## 2. What a round-0 breach would change

Experiment: the breach of a round (the loop of `catchment_round`, region-wide, U rules; lowers a receiver that stands higher than its donor allows, by up to 500 m) applied to z0 before the 4 rounds, once or three times (re-routed between passes); then the 4 rounds under U as before. The library's round 1 applies the same breach after routing z0, so the lowered cells are the same ones; the difference is that round 1 then routes and sweeps a surface with the flats already breached.

| Fixture | Passes | Lifted at z0 after the passes | Cells lowered (mean / max m) | Lifted after the 4 rounds | Runs > 10 / > 20 | Chord 8 diagonal, 2.5 deg | Final surface against the one without a round-0 breach: \|dz\| p95 (floor, U under two seeds), mean dz | Relief p95 after the rounds |
|---|---|---|---|---|---|---|---|---|
| R1h | 0 | 36.28 % | 0 | 1.83 % | 0.0725 / 0.0216 | 0.0747 | n/a | 477 m |
| R1h | 1 | 5.06 % | 54,622 (65.1 / 811) | 2.26 % | 0.0563 / 0.0178 | 0.0719 | 99.6 m (84.4 m), +0.64 m | 495 m |
| R1h | 3 | 1.98 % | 54,839 (72.9 / 938) | 2.17 % | 0.0521 / 0.0140 | 0.0700 | 102.4 m, +0.43 m | 498 m |
| R3a | 0 | 25.46 % | 0 | 6.61 % | 0.1133 / 0.0574 | 0.1159 | n/a | 116 m |
| R3a | 1 | 5.44 % | 156,412 (6.6 / 476) | n/a | 0.1130 / 0.0594 | 0.1169 | 9.41 m (15.25 m), -0.22 m | 119 m |
| R3a | 3 | 5.47 % | 156,688 (6.6 / 476) | n/a | 0.1130 / 0.0594 | 0.1168 | 9.42 m, -0.22 m | 119 m |
| R3b | 0 | 26.72 % | 0 | 2.87 % | 0.0510 / 0.0131 | 0.0646 | n/a | 177 m |
| R3b | 1 | 1.40 % | 190,037 (15.4 / 433) | n/a | 0.0454 / 0.0108 | 0.0616 | 50.4 m (40.15 m), +3.42 m | 201 m |
| R3b | 3 | 1.41 % | 190,477 (15.3 / 433) | n/a | 0.0453 / 0.0108 | 0.0616 | 50.5 m, +3.42 m | 201 m |

- One pass removes most of the lifted share (to 1.4 to 5.4 % of land, the DEM's order); further passes change it little. What remains is flat at the sea level (R3a: components of over 300 cells less than 1 m deep, 4.6 % of land; the coastal plain).
- The straight-run share (runs over 10 cells) falls by 22 % (R1h, one pass) to 28 % (three passes), by 11 % in R3b and not at all in R3a; the diagonal chord share moves by 4 to 6 % (R1h), +1 % (R3a), 5 % (R3b). Against the matched DEM pools of `results.md` the run share over 10 cells is 2.0 to 2.1x (R1h, 2.8x without the breach), 10.0x (R3a, 10.0x) and 4.0x (R3b, 4.5x).
- R3a's straight runs lie on the sea-level flats that a breach does not remove (51 to 55 % of its channel steps are on lifted cells after the rounds).
- The final surface changes by about the noise floor in R1h (99.6 m p95 against 84.4 m) and R3a (9.4 m against 15.25 m), and above it in R3b (50.4 m against 40.15 m, mean +3.4 m, relief p95 +14 %).

## 3. Caveats

- One seed per region; the breach is emulated region-wide in the receivers-first order; R3b's rounds here run on the library before GEN-172, so its window-cut trenches are in the "without" and "with" columns alike.
- The DEM tiles are small, so large basins cannot be seen in them; their 1.0 % over 300 cells is a statement about 15 tiles of 1 x 1 degree.
- "Depth" is the largest fill depth of a depression, not its mean.

## 4. Addendum: the round-1 breach, its depth distribution and the deepest cells (architect, 2026-11-12)

Added on the bulldozer's order. Probe `shapes_check breach1` (local branch `spike/spike-108-rerun`, never pushed), on main 1041d28c: the breach of round 1 on z0 under U (the loop of `catchment_round`, region-wide, leaves first, up to 500 m per lowering event; a receiver is lowered towards its donor's current level minus 0.001 times the distance, so a lowered cell lowers its own receiver next). The lowering of a cell is the sum of its events. Cells of the box. Raw: `raw/breach1-*.txt`.

| Fixture | Lowered cells (% of land) | p10 / p25 / p50 / p75 / p90 / p95 / p99 (m) | p99.9 / max (m) | Cells lowered by > 10 m / > 100 m / > 500 m |
|---|---|---|---|---|
| R1h | 55,716 (20.81 %) | 0.56 / 3.59 / 20.22 / 76.61 / 215.56 / 338.35 / 498.03 | 578.2 / 794.1 | 34,486 / 11,413 / 215 |
| R3a | 159,595 (21.95 %) | 0.19 / 0.56 / 1.85 / 6.04 / 14.60 / 23.50 / 92.03 | 291.7 / 476.0 | 25,103 / 1,490 / 0 |
| R3b | 195,596 (22.14 %) | 0.26 / 0.87 / 3.07 / 9.67 / 26.56 / 59.21 / 295.77 | 389.9 / 433.5 | 47,584 / 6,829 / 0 |

By the size of the depression the lowered cell lies in (z0; p50 / p90 / p99 of the lowering, m):

| Depression | R1h | R3a | R3b |
|---|---|---|---|
| not lifted (a receiver outside any depression) | 1.08 / 60.7 / 462.5 (11,743 cells) | 0.55 / 5.1 / 28.2 (54,337) | 1.02 / 14.5 / 227.1 (60,138) |
| 1 to 30 cells | 5.20 / 67.0 / 384.3 (4,218) | 1.61 / 9.0 / 38.4 (13,195) | 2.00 / 15.6 / 254.7 (21,248) |
| 31 to 300 | 16.29 / 102.6 / 463.3 (11,263) | 3.22 / 14.3 / 38.8 (26,413) | 3.35 / 19.8 / 270.3 (41,103) |
| 301 to 3,000 | 47.63 / 306.3 / 500.0 (18,061) | 4.52 / 20.8 / 45.5 (24,515) | 5.20 / 36.2 / 285.0 (28,110) |
| over 3,000 | 51.66 / 248.6 / 487.8 (10,431) | 3.51 / 23.3 / 182.9 (41,135) | 6.88 / 49.6 / 348.7 (44,997) |

The deepest cells of R1h (window cell = global cell - (7856, 560); z0 and z after the breach in m; lowering events by different donors):

| Global cell | z0 | After | Lowering | Fill depth at the cell | Events (capped) | Lowest donor z0 | Depression (cells / depth) |
|---|---|---|---|---|---|---|---|
| (8478, 771) | 969.0 | 174.9 | 794.1 | 10.6 | 2 (1) | 935.7 | 1,268 / 810 m |
| (8478, 770) | 935.7 | 175.7 | 760.0 | 43.9 | 2 (1) | 790.3 | 1,268 / 810 m |
| (8656, 855) | 1,207.7 | 465.0 | 742.7 | 9.6 | 2 (1) | 988.8 | 301 / 785 m |
| (8597, 916) | 859.4 | 127.5 | 731.8 | 91.7 | 2 (1) | 592.4 | 678 / 852 m |
| (8596, 917) | 843.1 | 126.6 | 716.5 | 108.0 | 2 (0) | 859.4 | 678 / 852 m |
| (8374, 873) | 713.5 | 0.0 | 713.5 | 13.7 | 2 (1) | 580.1 | 18,191 / 726 m |

- The cell of 717 m (the maximum lowering inside the straight runs of `results.md`/the breach chains of its section 10) is (8596, 917) = window cell (740, 357) at margin 200: z0 843.1 m, lowered to 126.6 m (716.5 m), two events, neither capped; it lies in a depression of 678 cells whose deepest fill is 852 m. The deepest cell of the box is (8478, 771), 794.1 m, in a depression of 1,268 cells 810 m deep.
- Why: these cells are the sills and the slopes of the rims of closed basins of the uneroded base (basins of 726 to 858 m depth). The breach cuts a trench from the rim to the level of the donors below: a cell is lowered to its donor's current level minus 0.001 times the distance, the donor having been lowered before it (leaves first), so the lowering of a cell is its height above the lowest cell downstream in the chain. One event is limited to 500 m; a cell lowered by two donors can pass 500 m (215 cells in R1h do).
- In R3a and R3b the deepest cells (R3a 476.0 m at (1102, 4520) and its neighbours, R3b 433.5 m at (10094, 8497)) are plateau cells at 435 to 476 m cut down to about 0 to 3 m: in R3a the cells lie in or next to a depression of 16,475 cells and 454 m depth, in R3b outside any depression; the chain below them was already cut to the floor (0.001 m above the sea level), so the lowering equals the plateau height above the sea. The probe did not trace the chains further.
- Median lowering is 20 m in R1h, 1.9 m in R3a and 3.1 m in R3b; the heavy tail (p99 498 m in R1h, 296 m in R3b) is the rims of the deep basins.
