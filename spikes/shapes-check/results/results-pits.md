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
