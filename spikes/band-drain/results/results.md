# SPIKE-110: band relief shaped to drain instead of capped

Measured 2026-11-12 by the spikes lane (BelowNormal, one process at a time, machine shared). Measurement only, no verdict (the architect's). Order: the bulldozer, from the architect (D117): positive-only relief on a connected floor, or ridges oriented down e0's slope, against the bars of the start-surface fix (z0 basins over 30 cells at most 7 % of land and depth p95 at most 270 m on R1h / R3a / R3b) plus the relief kept (land slope and the 5 x 5-cell local relief near the cap-off values).
Probe: `probe-src.diff` (changes in `crates/generators/src` on main a74445d0, GEN-179 in) and `shapes_check.rs.txt` (mode `z0bump`, `z0tilt` with `PLANET_SPEC`); the code stays on the local branch `spike/spike-110-probe`.

## Metrics and fixtures (as the explorer's table; the probe reproduces it exactly)
- Windows R1h `1,12288,8056,8675,760,1379,14`, R3a `4,12288,168,1187,4520,5539,15`, R3b `2,12288,9768,10787,8104,9123,15` (814 m cells, margin 200); land cells of the box only.
- Basins: interior depressions over 30 cells of z0 (U routing, canonical flood), share of land and depth p50 / p95 (as `spikes/start-surface-basins/results/results.md`).
- Slope: central differences of z0, rise over run. Local relief: range (max - min) of z0 in the 5 x 5 cells around a land cell (about 3.3 km).
- Cap-off (the plain build) reproduced: R1h 32.25 %, depth p95 809.8 m, slope 0.0226 / 0.1830, relief 94.1 / 654.8 m; R3a 21.97 %, 16.0 m, 0.0037 / 0.0383, 14.3 / 166.9 m; R3b 22.52 %, 14.5 m, 0.0029 / 0.0081, 11.7 / 28.4 m. The cap c = 1 (GEN-179) reproduced too: R1h 12.29 % / 93.7 m, 0.0048 / 0.0408, 20.7 / 158.7 m.

## What the band is (reading of the code, then measured)
- The plain band is `amp x noise`; the noise of each octave is the signal (1 - |n|)^2 of a gradient noise n: 1 exactly on the zero set of n, lower toward the extremes. The zero set of a 2-D noise is made of closed curves, so every crest line is a closed ring of constant height around a lower cell: a basin as deep as the relief whatever the slope of e0 (SPIKE-109 measured the excess at 20 points of the 32 % in R1h, and the 8 km hill octave and the 16 km ridged octave as the largest terms).
- Variant A, bumps on a connected floor: each octave's signal is `s = min(max(|n| - c, 0) / (top - c), 1)`, hills around the extremes of n on a floor `{|n| < c}`; c is the quantile q of |n| (the floor fraction; 0.6 to 0.9 swept, the floor percolates from about 0.6), `top` the p99.9 of |n|. The gains hold the standard deviation of the plain band (calibrated on 150,000 directions; ridged and hill separately; the ridged mean replaces the 0.45 centre). A further gain multiplies the amplitude.
- Variant A+B, the same bumps dragged into streaks along e0's ascent (4 samples of e0 at 3 km, as `e0_slope`): the bump signal is the maximum over K samples half a wavelength apart along the unit tangent of the ascent (its height is kept, its shape becomes a spur along the flow). Calibrated gains (K samples, a fixed tangent: the noise is isotropic).
- Not tested: a distance-to-channel-tree relief (candidate C of the design reading).

## Result 1: the band alone (tilt 0), share % over 30 cells / depth p95 (m) / slope p50 p95 / relief p50 p95 (m)
| Variant | R1h | R3a | R3b |
|---|---|---|---|
| cap off | 32.25 / 810 / 0.0226 0.183 / 94.1 654.8 | 21.97 / 16.0 / 0.0037 0.0383 / 14.3 166.9 | 22.52 / 14.5 / 0.0029 0.0081 / 11.7 28.4 |
| cap c = 1 | 12.29 / 93.7 / 0.0048 0.0408 / 20.7 158.7 | 9.90 / 59.0 / 0.0017 0.0383 / 8.0 166.9 | 9.10 / 27.7 / 0.0013 0.0074 / 5.8 28.7 |
| A q 0.6, gain 1 | 14.64 / 92.2 / 0.0136 0.146 / 64.1 558.0 | 16.33 / 13.1 / 0.0034 0.0381 / 13.1 166.6 | 13.80 / 17.2 / 0.0026 0.0082 / 10.6 28.8 |
| A q 0.7, gain 1.3 | 14.03 / 86.9 / 0.0139 0.1848 / 68.6 716.5 | 16.10 / 17.3 / 0.0041 0.0384 / 16.2 167.0 | 13.46 / 19.9 / 0.0030 0.0101 / 12.6 33.3 |
| A q 0.8, gain 1.3 | 12.54 / 131.9 / 0.0113 0.1641 / 59.1 674.1 | not run | not run |
| A+B q 0.85, K 3, gain 1 | 13.66 / 190.1 / 0.0123 0.1589 / 58.7 614.6 | 17.17 / 14.6 / 0.0033 0.0382 / 13.1 166.6 | 14.01 / 24.4 / 0.0025 0.0084 / 10.4 28.3 |
| A+B q 0.9, K 5, gain 1 | 14.23 / 319.4 / 0.0131 0.1831 / 62.8 690.8 | 16.68 / 16.3 / 0.0035 0.0382 / 13.8 166.7 | 14.41 / 22.1 / 0.0026 0.0088 / 10.9 28.5 |

- Floor fraction at gain 1, A, R1h (share / relief p50): q 0.6 14.64 / 64.1, 0.7 13.83 / 57.7, 0.8 12.62 / 50.0, 0.9 11.94 / 41.5 (depth p95 92, 92, 132, 308 m).
- With the band off the same windows keep 12.24 / 11.79 / 10.89 % (SPIKE-109): the bump variants leave 1 to 4 points of share above the band-off floor, the cap leaves 0.05 (R1h) and goes below it in R3 (9.90, 9.10 %; not traced), at depth p95 59 and 28 m.
- The relief kept: the cap collapses it (R1h 5x5 relief p50 22 % and p95 24 % of cap-off, slope p50 21 %); A keeps R1h at 73 % / 109 % (relief p50 / p95) and slope p50 61 % (gain 1.3) and R3 at 92 to 113 % (p50) and 100 % (p95) in R3a and 91 to 108 % (p50) in R3b of cap-off.
- Gain 1.3 on A raises R1h's relief p95 above cap-off (717 against 655 m) at depth p95 87 m. Gain 1.3 with K 5 or 9 (calibration made for K = 1) gave a relief p95 of 1.6 to 2.5 km and depth max up to 2 km (`raw/bump-r1h-c.txt`): the streak variants need their own gain (done: gain 1 with calibration for K).

## Result 2: with the tilt standing in for the other fixes (z = z0 + T x S, S the smooth land depth, T for a median gradient of 2 / 4 / 8 m/km; `spikes/start-surface-basins/results/tilt.md`)
The tilt of that note adds 0.5 to 3 km of height at the box medians and was refused as a fix; it is used here only to read the bars with the shallow basins of the coast plain drained. The plateau bowl of R3a (454 m) is not touched by it (R3a depth max 330 to 345 m remains in every row).
Share % / depth p95 (m) at tilt 0 / 2 / 4 / 8 m/km:
| Variant | R1h | R3a | R3b |
|---|---|---|---|
| cap off (SPIKE-109 follow-up) | 32.25 / 25.31 / 21.77 / 16.92 (p95 810 to 829 m) | 21.97 / 10.78 / 6.81 / 3.53 | 22.52 / 4.91 / 1.52 / 0.29 |
| cap c = 1 | 12.29 / 7.17 / 5.36 / 2.86; p95 94 / 155 / 175 / 200 | 9.90 / 6.71 / 4.92 / 3.20; p95 59 / 119 / 91 / 115 | 9.10 / 1.10 / 0.35 / 0.12; p95 28 / 27 / 32 / 9 |
| A q 0.7, gain 1.3 | 14.03 / 8.31 / 5.97 / 3.07; p95 87 / 114 / 158 / 163 | 16.10 / 9.19 / 6.44 / 3.67; p95 17 / 57 / 56 / 70 | 13.46 / 1.80 / 0.52 / 0.15; p95 20 / 15 / 15 / 13 |
| A+B q 0.85, K 3, gain 1 | 13.66 / 8.73 / 7.03 / 3.53; p95 190 / 178 / 199 / 241 | 17.17 / 9.57 / 6.39 / 3.46; p95 15 / 53 / 67 / 80 | 14.01 / 1.63 / 0.39 / 0.08; p95 24 / 17 / 15 / 10 |
| A+B q 0.9, K 5, gain 1 | 14.23 / 9.05 / 7.07 / 3.80; p95 319 / 304 / 327 / 333 | 16.68 / 9.46 / 6.30 / 3.34; p95 16 / 49 / 63 / 107 | 14.41 / 1.84 / 0.52 / 0.11; p95 22 / 16 / 12 / 12 |

Relief kept at the 4 m/km tilt (slope p50 p95 / relief p50 p95 in m; the tilt itself adds slope and relief: R3a cap-off 0.0037 0.0383 / 14.3 166.9, R3b 0.0029 0.0081 / 11.7 28.4, R1h 0.0226 0.183 / 94.1 654.8):
| Variant | R1h | R3a | R3b |
|---|---|---|---|
| cap c = 1 | 0.0080 0.0435 / 35.3 176.7 | 0.0060 0.0404 / 27.5 197.0 | 0.0046 0.0114 / 19.8 47.2 |
| A q 0.7, gain 1.3 | 0.0157 0.1920 / 77.6 717.8 | 0.0066 0.0406 / 29.1 197.5 | 0.0054 0.0132 / 22.8 50.1 |
| A+B q 0.85, K 3 | 0.0141 0.1665 / 67.4 619.7 | 0.0063 0.0403 / 27.9 197.4 | 0.0051 0.0119 / 21.4 47.0 |
| A+B q 0.9, K 5 | 0.0148 0.1895 / 71.4 692.0 | 0.0064 0.0404 / 28.1 197.5 | 0.0052 0.0122 / 21.8 46.9 |

## Reading (no verdict)
- Shaping the band to drain (A, A+B) takes the band's basins out about as completely as the cap does and keeps most of the relief where the cap keeps about a fifth to a quarter: R1h at 4 m/km: share 5.4 % (cap), 6.0 % (A), 7.0 % (A+B) with relief p50 / p95 of 35 / 177, 78 / 718, 67 / 620 m against 94 / 655 m cap-off.
- The bars: with the tilt at 4 m/km all three variants are at or under 7 % in R3a and R3b and at 5.4 to 7.1 % in R1h (A+B 7.03 and 7.07: on the line); depth p95 stays under 270 m except A+B q 0.9, K 5 in R1h (327 m, 333 m at 8 m/km); at tilt 0 none reaches 7 % (the floor of the coast and plateau terms: 12 to 17 %).
- The look: `images/` (top-down shaded relief from the north-west, basins outlined, 3 x crop of 200 x 200 cells = 163 km a side at 271 m per pixel, 20 km bar; no camera pose: maps). A gives isolated rounded hills on a flat floor; A+B gives spurs along the slope (ridge-like); the cap gives a nearly flat plain with a little relief. The look is the owner's call.
- Not covered: the plateau fix (R3a's 454 m bowl), a bounded tilt (the explorer's form), the class weights (read from `e_smooth`, not moved), the hypsometry (the bump floor lowers the median land height by the plain band's mean, about 0.5 x amp: 15 to 90 m), the effect on the erosion bake (the bake reads z0 as a start surface; no bake was run), tile bounds of the band (the min/max pyramid), and a bound check of the new signal in the LOD plan (the bump signal is in [0, 1] like the plain one).
- Calibration to repeat in a build: the gains hold the plain band's standard deviation, not its relief; relief matches cap-off at gain 1 to 1.3 in R1h.

## Reproduction
- Local branch `spike/spike-110-probe`; `cargo build --release -j 4 -p planet-generators --example shapes_check`; `PLANET_WIN=<window> PLANET_MARGIN=200 PLANET_BUMPS="none;cap:1;bump:0.7:1.3:1;bump:0.85:1:3" shapes_check z0bump 8` (spec `bump:<floor fraction>:<gain>:<K>`), `PLANET_SPEC=<spec> PLANET_TILTS=0,2,4,8 shapes_check z0tilt 8`, images with `PLANET_IMG_DIR` and `PLANET_CROP=x0,y0,size`. A z0 sample is 30 to 40 s a window (A+B somewhat more: 4 more e0 evaluations a cell).
- Raw: `raw/bump-*.txt` (`SPEC`, `BASINRAW`, `RELIEF` lines), `raw/tilt3-*` (cap, A), `raw/tilt4-*` (A+B).
