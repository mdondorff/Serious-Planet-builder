# SPIKE-111: band relief from a channel tree grown over e0 (candidate C), first cut

Measured 2026-11-12 by the spikes lane (BelowNormal, one process at a time). Measurement only, no verdict (the architect's). Order: the bulldozer, from the architect (D120 after SPIKE-110): the band relief as a function of the distance to a drainage network grown over e0 and the tree's order, noise only above the valley floor; bars of the start-surface fix (z0 basins over 30 cells at most 7 % of land with the tilt stand-in, depth p95 at most 270 m), the relief kept (5 x 5 p50 / p95 against the plain band), the cost per base sample, the top-down crops and one low-sun oblique with scale bar and pose. Time-box 1.5 days; this note is the first cut after about 2 h (one tree form and a small grid of its parameters, no look iteration beyond the crops and one oblique).
Probe: `probe-src.diff` (changes in `crates/generators/src` on main a74445d0: `set_probe_band_off`, `BakeInputs::band_amp`, plus the SPIKE-110 variants) and `shapes_check.rs.txt` (mode `z0tree`, an oblique renderer); the code stays on the local branch `spike/spike-111-probe`.

## What was built
- e0 = the base without the band (the probe switch `set_probe_band_off`); routed (D8, U rules, the canonical flood: `shapes_check`'s `route`) over the whole window (margin 200 cells); the upstream area of every cell in m2.
- The tree: three channel tiers, the cells whose area is at least `a0 x 4^t` km2 (t = 0, 1, 2; `a0` = 16 km2: 16, 64, 256 km2). The distance to the nearest channel cell of each tier (8-neighbour chamfer in metres).
- The relief: `relief = amp x gain x f x sum_t w_t x s(d_t / L_t)`; `s(x) = smoothstep(0, 1, min(x, 1))` (0 on a channel, 1 at the tier's hillslope length), `L_t = lfac x 1000 x sqrt(area_t in km2)` m, `w_t` = 0.2 / 0.3 / 0.5 (fine to coarse), `amp` = the plain band's amplitude of the cell (`hill_amplitude_for_k(k) x (1 + 3.5 x mountain share)`), `f = max(0.2, 1 + kn (2u - 1))` with `u` the plain band's normalised noise in [0, 1] ("noise only above the floor": it scales the rise, never lowers a channel below e0). z = e0 + relief on land; the tilt stand-in as in `spikes/start-surface-basins/results/tilt.md` (z + T x S for a median gradient of 2 / 4 / 8 m/km).
- The tree specs are written `tree:<a0>:<lfac>:<gain>:<kn>`; the plain band is `none` (the control: it reproduces the SPIKE-109 / SPIKE-110 baseline numbers).
- Windows R1h `1,12288,8056,8675,760,1379,14`, R3a `4,12288,168,1187,4520,5539,15`, R3b `2,12288,9768,10787,8104,9123,15`; metrics as in `spikes/band-drain/results/results.md` (slope: rise over run; relief: range in 5 x 5 cells; basins: interior depressions over 30 cells).

## Result 1: the parameter grid on R1h (tilt 0 and 4 m/km): share % / depth p95 (m) / slope p50 p95 / relief p50 p95 (m)
| Spec | Tilt 0 | Tilt 4 |
|---|---|---|
| plain band (cap off) | 32.25 / 852 / 0.0226 0.183 / 94.1 654.8 | 21.77 / 817 / 0.0241 0.187 / 101.8 656.1 |
| tree 4 km2, L 0.5, gain 0.6, kn 0.5 | 12.21 / 164 / 0.0105 0.082 / 51.1 294.4 | 6.18 / 167 / 0.0127 0.085 / 60.1 311.8 |
| tree 4 km2, L 0.5, gain 0.6, kn 0 | 10.24 / 396 / 0.0068 0.106 / 35.0 373.2 | 4.85 / 302 / 0.0095 0.112 / 47.1 394.4 |
| tree 8 km2, L 0.8, gain 1.2, kn 0.5 | 11.76 / 325 / 0.0184 0.144 / 85.9 535.4 | 6.15 / 319 / 0.0202 0.148 / 94.6 541.2 |
| tree 32 km2, L 0.8, gain 1.2, kn 0.5 | 12.07 / 325 / 0.0203 0.148 / 92.3 550.4 | 6.56 / 330 / 0.0220 0.152 / 100.3 555.3 |
| **tree 16 km2, L 0.6, gain 1.0, kn 0.5** | 12.72 / 269 / 0.0181 0.134 / 83.5 488.5 | 6.78 / 271 / 0.0199 0.138 / 92.0 497.8 |
| tree 16 km2, L 1.0, gain 1.0, kn 0.5 | 12.90 / 264 / 0.0170 0.116 / 76.9 438.5 | 6.70 / 265 / 0.0187 0.120 / 84.9 447.6 |
- The first rows (a0 = 4 km2) have channels in 62 % of the window's cells: too dense, relief halves (gain 0.6) and the depth tail follows kn (the noise term: kn 0.5 164 m against kn 0 396 m).
- Channel density of the chosen tiers (share of the window's cells at or over 16 / 64 / 256 km2): R1h 36 / 14 / 4.3 %, R3a 18.5 / 5.3 %, R3b 33.5 / 12.4 % (`raw/`).

## Result 2: the chosen spec (tree 16 km2, L 0.6, gain 1, kn 0.5) on the three windows, share % / depth p95 (m) / slope p50 p95 / relief p50 p95 (m) at the tilts 0 / 2 / 4 / 8 m/km
| Window | Surface | Tilt 0 | Tilt 2 | Tilt 4 | Tilt 8 |
|---|---|---|---|---|---|
| R1h | plain band | 32.25 / 852 / 0.0226 0.183 / 94.1 654.8 | 25.31 / 819 / 0.0234 0.184 / 98.5 655.5 | 21.77 / 817 / 0.0241 0.187 / 101.8 656.1 | 16.92 / 829 / 0.0261 0.188 / 110.5 677.1 |
| R1h | tree | 12.72 / 269 / 0.0181 0.134 / 83.5 488.5 | 8.51 / 268 / 0.0190 0.136 / 88.3 489.9 | **6.78 / 271** / 0.0199 0.138 / 92.0 497.8 | 5.49 / 273 / 0.0222 0.138 / 101.5 517.7 |
| R3a | plain band | 21.97 / 16 / 0.0037 0.0383 / 14.3 166.9 | 10.78 / 17 / 0.0046 0.0400 / 19.4 190.3 | 6.81 / 39 / 0.0064 0.0404 / 28.2 197.0 | 3.53 / 60 / 0.0106 0.0422 / 47.8 207.1 |
| R3a | tree | 7.56 / 13 / 0.0044 0.0393 / 18.1 171.6 | 9.37 / 43 / 0.0051 0.0410 / 22.1 194.2 | **6.17 / 46** / 0.0068 0.0414 / 30.3 200.6 | 3.49 / 67 / 0.0108 0.0431 / 48.4 210.8 |
| R3b | plain band | 22.52 / 15 / 0.0029 0.0081 / 11.7 28.4 | 4.91 / 13 / 0.0037 0.0097 / 15.3 36.9 | 1.52 / 12 / 0.0052 0.0120 / 21.7 47.4 | 0.29 / 12 / 0.0088 0.0172 / 37.4 71.2 |
| R3b | tree | 10.26 / 11 / 0.0031 0.0090 / 12.7 32.5 | 2.83 / 13 / 0.0039 0.0103 / 16.5 39.9 | **1.10 / 11** / 0.0054 0.0124 / 22.7 50.0 | 0.39 / 10 / 0.0089 0.0175 / 37.7 73.3 |
- At the 4 m/km tilt stand-in the tree meets the bars in all three windows: share 6.78 / 6.17 / 1.10 % (at most 7 %), depth p95 271 / 46 / 11 m (R1h at the 270 m line).
- Relief kept in R1h at 4 m/km (plain band 101.8 / 656.1 m, slope 0.0241 / 0.187): the tree 92.0 / 497.8 m (90 % / 76 %), slope 0.0199 / 0.138 (83 % / 74 %); at tilt 0 against the plain band 94.1 / 654.8: 83.5 / 488.5 m (89 % / 75 %). R3a and R3b are at or above the plain band's relief (the tilt adds relief; at tilt 0 R3a 18.1 / 171.6 against 14.3 / 166.9).
- Without any tilt the tree already drains much more than the plain band in the plains: R3a 21.97 -> 7.56 %, R3b 22.52 -> 10.26 %, R1h 32.25 -> 12.72 % (the rise from a channel is itself a gradient toward it; the plateau and coast terms of e0 keep the remainder: SPIKE-109 measured 12.2 / 11.8 / 10.9 % with the band off).
- R3a at tilt 2 is 9.37 %, above the 7.56 % at tilt 0 (non-monotone in the tilt; not traced).

## Result 3: cost per base sample (the probe is window-based)
- Routing the e0 surface of a window (D8, U rules, canonical flood): 1.1 s for 1.04 million cells (R1h), 2.4 and 2.5 s for 1.97 and 2.02 million cells (R3a, R3b): about 1.1 to 1.2 us a cell; the three chamfer distances 0.03 s each for 1.04 million cells (0.06 to 0.07 s for 2 million), about 0.03 us a cell each. The tree adds about 1.3 us a cell, window-based, against the three base evaluations a cell of the probe (`band_amp`, `base`, S) that took 18.8 s (R1h, run alone), 31.8 s (R3a) and 28.8 s (R3b): about 18, 16 and 14 us a cell for the three, so about 5 to 6 us each: the tree is a fifth to a quarter of one base evaluation.
- The window form is not a pointwise function (Result 4 reads the tree from a coarse lattice instead): it needs a neighbourhood (the routed upstream area of the window). In the bake the base is sampled by tile and by LOD with no neighbourhood; a window-based tree changes with the window (CON-30: partitioning changes nothing; the same cell in two tilings would differ where its upstream path leaves a window, as the K7 note 2026-11-12 measured for the incision). What C needs in production is a drainage structure fixed by (seed, point): the regional solve (12,288 cells a face edge) is the only structure of that scale; a tree grown from e0 on a coarse global lattice (SPIKE-109's e0 is band-free and smooth, so it can be routed once per face at 3 to 6 km) and read like the class lattice is the form to cost. Not done in this first cut.


## Result 4: the pointwise form, the tree on a coarse lattice (share % / depth p95 (m) / slope p50 p95 / relief p50 p95 (m) at tilt 0 and 4 m/km)
The e0 of the window is averaged over `cf` x `cf` cells, routed there (the same area thresholds in km2), the distance to the channels measured there, and the distance read back at the fine cells bilinearly (what a stored per-face lattice read at a point would give; `tree:16:0.6:1.0:0.5:<cf>`). cf 1 is the window form of Result 2; cf 2, 4, 8 are lattices of 1.6, 3.3 and 6.5 km.
| Window | cf | Tilt 0 | Tilt 4 |
|---|---|---|---|
| R1h | 1 | 12.72 / 269 / 0.0181 0.134 / 83.5 488.5 | 6.78 / 271 / 0.0199 0.138 / 92.0 497.8 |
| R1h | 2 | 13.66 / 271 / 0.0148 0.111 / 68.1 419.7 | 7.34 / 268 / 0.0167 0.115 / 76.7 430.2 |
| R1h | 4 | 15.02 / 237 / 0.0113 0.091 / 52.3 354.1 | 6.99 / 236 / 0.0134 0.094 / 62.0 367.8 |
| R1h | 8 | 17.38 / 235 / 0.0072 0.068 / 32.2 270.5 | 8.20 / 234 / 0.0097 0.071 / 44.2 291.7 |
| R3a | 1 | 7.56 / 13 / 0.0044 0.0393 / 18.1 171.6 | 6.17 / 46 / 0.0068 0.0414 / 30.3 200.6 |
| R3a | 2 | 11.20 / 13 / 0.0041 0.0396 / 17.4 173.2 | 6.26 / 63 / 0.0067 0.0417 / 30.0 202.2 |
| R3a | 4 | 16.35 / 18 / 0.0037 0.0398 / 16.3 173.8 | 6.62 / 91 / 0.0065 0.0419 / 29.8 203.0 |
| R3a | 8 | 20.41 / 21 / 0.0031 0.0396 / 13.6 173.2 | 6.39 / 105 / 0.0062 0.0417 / 28.2 202.7 |
| R3b | 1 | 10.26 / 11 / 0.0031 0.0090 / 12.7 32.5 | 1.10 / 11 / 0.0054 0.0124 / 22.7 50.0 |
| R3b | 2 | 10.81 / 14 / 0.0029 0.0086 / 12.0 31.8 | 0.97 / 14 / 0.0053 0.0122 / 22.2 49.8 |
| R3b | 4 | 12.11 / 18 / 0.0026 0.0081 / 10.8 30.6 | 1.01 / 17 / 0.0051 0.0118 / 21.5 48.9 |
| R3b | 8 | 15.15 / 27 / 0.0022 0.0074 / 9.2 29.1 | 1.17 / 18 / 0.0049 0.0113 / 20.7 47.8 |
- At the 4 m/km tilt stand-in the bars hold for cf 2 and 4 on all three windows (share 7.34 / 6.26 / 0.97 % and 6.99 / 6.62 / 1.01 %, a hair over 7 % at cf 2 in R1h) and the share is 8.2 % at cf 8 in R1h; depth p95 stays under 270 m.
- What the coarse lattice costs is relief in R1h: p50 / p95 76.7 / 430 (cf 2), 62.0 / 368 (cf 4), 44.2 / 292 m (cf 8) against 92.0 / 498 m (cf 1), the plain band 101.8 / 656 m: a smoother distance field gives lower slopes; a larger gain would restore it (not tried).
- Without the tilt the plains lose the drainage as the lattice coarsens (R3a 7.56 -> 11.2 / 16.4 / 20.4 %, R3b 10.3 -> 10.8 / 12.1 / 15.2 %): the fine tree's gradient toward the channels needs the fine distance.
- Storage of the lattice (an estimate, not built): a cube face of 98,304 local cells a side is 10,000 km; at cf 4 (3.3 km) 3,070 cells a side, 9.4 million cells a face, 57 million cells for six faces, three tiers of a distance in u16: about 340 MB; at cf 2 four times that. The tier distances could be one field each of a coarse order-weighted distance (1 value a cell: 115 MB at cf 4). Not measured; the lattice would be computed once per world from e0 (a band-free smooth surface) by a routing pass over the 57 million cells: about 60 seconds at the probe's 1.1 us a cell.

## Result 5: the gain on the coarse lattice (R1h; share % / depth p95 (m) / relief p50 p95 (m); spec `tree:16:0.6:<gain>:0.5:<cf>`)
Bars of the order: share at most 7 %, depth p95 at most 270 m, 5 x 5 relief at least 90 % / 75 % of the plain band at the same tilt (tilt 4: plain 101.8 / 656.1 m, so at least 91.6 / 492; tilt 0: plain 94.1 / 654.8, so at least 84.7 / 491).
| cf | gain | Tilt 4: share / depth p95 / relief | Tilt 0: share / depth p95 / relief |
|---|---|---|---|
| 4 | 1.0 | 6.99 / 236 / 62.0 368 (61 %, 56 %) | 15.02 / 237 / 52.3 354 |
| 4 | 1.3 | 7.33 / 299 / 70.9 457 (70 %, 70 %) | 15.48 / 287 / 60.8 447 |
| 4 | 1.5 | 7.50 / 337 / 76.2 518 (75 %, 79 %) | 15.73 / 320 / 66.0 510 |
| 4 | 1.7 | 7.69 / 349 / 81.5 576 (80 %, 88 %) | 15.98 / 352 / 70.8 573 |
| 4 | 2.0 | 7.87 / 411 / 89.0 668 (87 %, 102 %) | 16.31 / 403 / 77.7 668 (83 %, 102 %) |
| 2 | 1.0 | 7.34 / 268 / 76.7 430 (75 %, 66 %) | 13.66 / 271 / 68.1 420 |
| 2 | 1.5 | 7.76 / 373 / 98.6 617 (97 %, 94 %) | 14.15 / 368 / 89.2 615 |
| 2 | 2.0 | 8.05 / 492 / 118.4 811 (116 %, 124 %) | 14.47 / 477 / 108.4 810 |
- No gain at cf 4 meets the bars together. Relief p95 reaches 75 % at gain 1.5 and p50 90 % at about gain 2.1, but depth p95 passes the 270 m bar already at gain 1.3 (299 m) and share at tilt 4 passes 7 % at gain 1.3 (7.33 %). At gain 1.0 the share and depth bars hold (6.99 %, 236 m) and the relief is 61 % / 56 % of the plain band.
- The same holds at cf 2 (relief 90 % at about gain 1.35, where depth p95 is about 330 m).
- Depth p95 grows with the gain at about 100 m a unit of gain at cf 4 (236 -> 299 -> 337 -> 349 -> 411 m): the depressions that remain (the plateau and belt terms of e0 and the closed floors) get deeper as the relief on them grows. This is a reading of the numbers, not traced.
- On this form relief and depth p95 move together; a route to relief that leaves the residual basins as deep as at gain 1.0 was not found in this time-box.

## Result 6: the plains without a tilt (the lattice alone, D126's reading; share % / depth p95 (m) / relief p50 p95 (m); tilt 0)
| Window | Plain band | cf 2, gain 1.0 | cf 4, gain 1.0 | cf 2, gain 2.0 | cf 4, gain 2.0 |
|---|---|---|---|---|---|
| R3a | 21.97 / 16 / 14.3 167 | 11.20 / 13 / 17.4 173 | 16.35 / 18 / 16.3 174 | 12.07 / 21 / 30.2 181 | 18.50 / 24 / 27.9 182 |
| R3b | 22.52 / 15 / 11.7 28.4 | 10.81 / 14 / 12.0 31.8 | 12.11 / 18 / 10.8 30.6 | 12.43 / 20 / 20.2 47.1 | 13.91 / 21 / 17.6 43.0 |
- Without a tilt the lattice carries neither plain window to the 7 % bar: R3a 11.2 % (cf 2) and 16.4 % (cf 4), R3b 10.8 % and 12.1 %, at gain 1.0; gain 2.0 changes the share by +1 to +2 points. Depth p95 holds everywhere (at most 24 m). The plain band is 22 %.
- The lattice drains about as far as the band-off floor of SPIKE-109 (R3a 12.2 %, R3b 11.8 %, R1h 10.9 %): the remainder is the plateau and coast terms of e0, (a reading, not traced: a tree read from e0 does not drain the pits of e0 itself). The finding for the owner's brief: the lattice alone does not fix the plains; the plateau terms of e0 do. The images show the residual basins as broad flats (`images/R3b-lattice-cf4-gain1.0-tilt0-crop.png`).
- cf 2 holds the plains better than cf 4 (R3a 11.2 against 16.4 %) (a reading, not traced: the distance read from a 3.3 km lattice is smoother than the fine one, so it carries less gradient toward the channels at 800 m cells).

## Face edges
The build of the lattice across the six cube faces as one routed graph is written in `lattice-build.md` (n = 3,072 cells a face edge at cf 4, 56.6 million cells, about 2 GB while building, 3 to 5 minutes on one thread; no code).

## Look
- Gain 1.0 and 2.0 at cf 4 (new in this pass; the crops of R1h at tilt 0 and 4, the oblique of the same pose, R3b crops at tilt 0): `images/R1h-lattice-cf4-gain{1.0,2.0}-tilt{0,4}-{crop,obl}.png`, `images/R3b-lattice-cf4-gain{1.0,2.0}-tilt0-crop.png`. In the R1h crop at tilt 0 the flat valley floors are large and level, a straight-edged rectangular strip at the middle right and another at the lower right (the straight-run flats of e0 again; the artefact of the first cut is not removed), the ridge relief is on the edges of the floors; gain 2.0 shows the same layout with steeper ridges (the red closed floors look similar). The oblique at gain 2.0 reads as smooth rolling dunes-like ground with a long shadowed flank; one faint straight line across the plain at the left (the same straight edge).
- Top-down crops (3 x magnified, 200 x 200 cells = 163 km a side, 20 km bar; NW light): `images/R1h-plain-band-tilt4-crop.png` against `images/R1h-tree-tilt4-crop.png`; boxes `R1h-*-box.png` (505 km, 100 km bar); R3b crops. Basins outlined in red.
- The plain band is ridge-rings everywhere (every crest closes a basin); the tree surface has broad valley floors with fine dendritic ridges on the slopes, much as a drained mountain land looks; the residual basins are the plateau and belt terms of e0 and a few closed floors. Artefacts: a straight horizontal edge at the left of the R1h crop (a flat of e0 routed along a row: the flats' straight-run flow of SPIKE-108 is inherited by the tree), and valley floors at the channels' flat width.
- Low-sun oblique, R1h: `images/R1h-plain-band-tilt4-oblique.png` and `R1h-tree-tilt4-oblique.png`: pinhole camera at face 1 cell (8526, 960) (window cell (670, 400)), 1,500 m above the ground, azimuth 350 deg (north-north-west, clockwise from up the image), pitch -7 deg, horizontal field of view 70 deg, sun from 80 deg (east) at 14 deg elevation with cast shadows; 26.0 m (plain band) and 19.5 m (tree) per pixel at the screen centre (18.6 and 13.9 km away); the scale bar is the bar at the lower left at the centre's scale. A simple height-field ray marcher over the 814 m cells (no texture; hard grid-scale shadow edges): relief is modest at this scale (R1h's 5 x 5 relief p95 is 0.5 to 0.7 km), the tree surface reads as broad valleys with smooth slopes, the plain band as lumpy ridges.

## Not done (the time-box closed with Results 5 and 6; per the lead's order of 2026-11-12 nothing further on the starting surface until the owner's decision)
- The order of a channel (Strahler) instead of the area tiers, tributary junction smoothing, a valley-floor width law, the straight-edge artefact of flats, more seeds on R1h; the lattice across face edges is written (`lattice-build.md`), not coded.
- The plateau fix and the bounded tilt (the bars are read with my S-tilt stand-in at 4 m/km, +1.0 to +1.5 km of height at the medians; refused as a fix); the effect on the erosion bake; hypsometry (the relief is positive on a floor: the median height falls by the plain band's mean, like SPIKE-110's variants); class weights.
- More windows and seeds (three fixtures only), a second look pass.

## Reproduction
- Local branch `spike/spike-111-probe`; `cargo build --release -j 4 -p planet-generators --example shapes_check`; `PLANET_WIN=<window> PLANET_MARGIN=200 PLANET_TREES="none;tree:16:0.6:1.0:0.5" PLANET_TILTS=0,2,4,8 shapes_check z0tree 8`; images with `PLANET_IMG_DIR`, `PLANET_CROP=x0,y0,size`, `PLANET_OBL=x,y,height_m,azimuth,pitch,fov,sun_az,sun_el` (x, y in box cells). A window run is 1 to 2 minutes.
- Raw: `raw/tree-f-*.txt` (the three windows, the chosen spec and the 1.0 length variant at four tilts, `BASINRAW`, `RELIEF`, tier and timing lines), `raw/tree-r1h-a.txt`, `raw/tree-r1h-b.txt` (the grid).
