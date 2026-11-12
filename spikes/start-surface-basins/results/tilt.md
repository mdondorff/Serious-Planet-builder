# SPIKE-109 follow-up: a sea-ward tilt added to the start surface (coast-plain probe)

Measurement only; no verdict. Measured 2026-11-12 by the spikes lane. Order: the bulldozer, for the start-surface fix (the smallest regional sea-ward gradient added to e0 that drains the R3 plains); the explorer writes the code. Windows, definitions and the share metric as in `results.md`.

## What was tilted
- z = z0 + T x S on land, S = the smooth land depth of the continent stage (`continents::continent_smooth` minus `SEA_LEVEL_C`, at least 0): the one global sea-ward field the base already has (the field the coast distance follows).
- T is set per target so that the median gradient of T x S over the box's land equals the target (m per km). The sum is the production form `e0 += T x S` except for the land floor (a raise never meets it) and the class weights (read from `e_smooth`, not moved here).
- S over the box land (p5 / p50 / p95 / max): R3a 0.046 / 0.133 / 0.173 / 0.197; R3b 0.069 / 0.315 / 0.437 / 0.488; R1h 0.101 / 0.237 / 0.402 / 0.452. Its gradient p50 per km: R3a 0.0004, R3b 0.0008, R1h 0.0009.
- The coast term already is `COAST_TOP_M x smoothstep(0, COAST_RAMP_DEPTH, land_depth)` = 620 m x smoothstep(0, 1.2, land_depth): at the R3 median S its slope is about 306 (R3a) and 600 (R3b) m per unit S, the box sits on the foot of that ramp (the coast term at the median S is 20 m in R3a and 106 m in R3b).

## Result: share of land in interior basins over 30 cells (%) / basins over 30 cells
| Median tilt gradient (m/km) | 0 | 1 | 2 | 4 | 8 | 16 |
|---|---|---|---|---|---|---|
| R3a | 21.97 / 1,227 | 19.03 / 837 | 10.78 / 562 | 6.81 / 291 | 3.53 / 140 | 1.39 / 68 |
| R3b | 22.52 / 1,960 | 10.14 / 1,613 | 4.91 / 1,096 | 1.52 / 449 | 0.29 / 81 | 0.10 / 19 |
| R1h | 32.25 / 996 | 27.75 / 972 | 25.31 / 978 | 21.77 / 918 | 16.92 / 841 | 11.02 / 689 |

(Finer steps 0.25 and 0.5 m/km in `raw-tilt/tilt-*.txt`: R3a 20.86 / 20.34 %, R3b 18.16 / 15.06 %, R1h 30.70 / 29.61 %.)

What the tilt costs: T per m/km is 2,383 (R3a), 1,210 (R3b), 1,058 (R1h) m per unit S, that is 8 and 2 times the coast ramp's own slope at the R3 medians, and 4 m/km adds this many metres over the box land (p50 / p95):
| Tilt | R3a | R3b | R1h |
|---|---|---|---|
| 2 m/km | +636 / +823 m | +762 / +1,057 m | +502 / +851 m |
| 4 m/km | +1,272 / +1,646 m | +1,524 / +2,114 m | +1,003 / +1,702 m |
| 8 m/km | +2,545 / +3,292 m | +3,048 / +4,227 m | +2,006 / +3,405 m |

- The gradient of z0 itself over the R3 land (m/km, p25 / p50 / p95): R3a 1.7 / 3.7 / 38; R3b 1.7 / 2.9 / 8.1. The tilt starts to drain where its gradient is about the size of the median gradient of z0: R3b falls from 22.5 to 4.9 % at 2 m/km and to 1.5 % at 4 m/km; R3a (with its 454 m plateau bowl, see `results.md`) needs 8 m/km for 3.5 %.
- Deep basins are not drained by a tilt: R3a's maximum depth stays 344 m at 4 m/km and 334 m at 8 m/km; R1h's depth p95 stays 810 to 830 m up to 16 m/km (share 11 %).
- The depth p50 of the surviving basins stays 6 to 10 m in R3 (R3a rises to 9.6 m at 8 m/km, R3b 11.4 m at 16 m/km: the shallow ones go first).

## Tilt on a quieter surface (R3, share % / basins)
| Surface | 0 | 1 | 2 | 4 | 8 m/km |
|---|---|---|---|---|---|
| R3a, band on | 21.97 / 1,227 | 19.03 / 837 | 10.78 / 562 | 6.81 / 291 | 3.53 / 140 |
| R3a, 8 km hill octave off (mask 32) | 17.37 / 931 | 15.11 / 411 | 8.46 / 216 | 5.19 / 113 | 2.85 / 53 |
| R3a, band off (mask 256) | 11.79 / 77 | 11.95 / 36 | 7.04 / 31 | 4.45 / 33 | 2.70 / 28 |
| R3b, band on | 22.52 / 1,960 | 10.14 / 1,613 | 4.91 / 1,096 | 1.52 / 449 | 0.29 / 81 |
| R3b, 8 km hill octave off | 15.62 / 1,312 | 4.95 / 773 | 2.05 / 380 | 0.45 / 88 | 0.17 / 30 |
| R3b, band off | 10.89 / 111 | 2.81 / 58 | 1.09 / 36 | 0.23 / 14 | 0.10 / 3 |

- Without the 8 km hill octave the share at the same tilt is 0.42 x (R3b, 2 m/km: 2.05 % against 4.91 %) and 0.78 x (R3a, 2 m/km: 8.46 % against 10.78 %).
- The gradient of z0 itself falls when the band is off (R3a p50 3.7 -> 1.0 m/km, R3b 2.9 -> 1.2 m/km), so a tilt of 1 to 2 m/km is already the size of the whole surface's relief gradient there.

## Reading (no verdict)
- A sea-ward tilt on S drains the shallow R3 plains, at a gradient of 2 to 4 m/km (R3b) and 4 to 8 m/km (R3a) on the band-on surface, which adds 0.8 to 3 km of height at the box's medians because S rises slowly (gradient 0.0004 to 0.0008 per km): the added height is the gradient times the distance from the coast, so the cost grows with the continent's width.
- It does not touch the deep basins (R3a plateau bowl, R1h mountains), and on R1h it takes 16 m/km (+4 km median) for 11 %.
- Quieter surface plus a smaller tilt: the band-amplitude and 8 km-octave candidates of `results.md` (c) halve the tilt needed in R3b.
- Not tested: a tilt that saturates beyond a depth (flat beyond it), a tilt on a metres-to-coast distance instead of S, a shorter coast ramp (`COAST_RAMP_DEPTH` below 1.2 steepens the foot of the existing ramp; its effect on the R3 medians follows from the 306 and 600 m per unit S above), and the tilt's effect on the class weights (they read `e_smooth`, which the tilt does not enter here).

## Reproduction
- Probe branch `spike/spike-109-probe` (local), mode `z0tilt` of `shapes_check.rs.txt`; env `PLANET_WIN`, `PLANET_MARGIN=200`, `PLANET_TILTS=0,1,2,4,8` (median gradient targets in m per km), `PLANET_BASE_OFF=<mask>` for the quieter surfaces. Raw: `raw-tilt/` (`TILTFIELD`, `TILT`, `BASINRAW`, `|grad z|` lines).
- Runs: BelowNormal, one at a time, 1 to 2 min a window and mask, machine at about 10 % load.
