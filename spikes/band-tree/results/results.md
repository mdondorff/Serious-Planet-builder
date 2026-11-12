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
- Not a pointwise function: the tree needs a neighbourhood (the routed upstream area of the window). In the bake the base is sampled by tile and by LOD with no neighbourhood; a window-based tree changes with the window (CON-30: partitioning changes nothing; the same cell in two tilings would differ where its upstream path leaves a window, as the K7 note 2026-11-12 measured for the incision). What C needs in production is a drainage structure fixed by (seed, point): the regional solve (12,288 cells a face edge) is the only structure of that scale; a tree grown from e0 on a coarse global lattice (SPIKE-109's e0 is band-free and smooth, so it can be routed once per face at 3 to 6 km) and read like the class lattice is the form to cost. Not done in this first cut.

## Look
- Top-down crops (3 x magnified, 200 x 200 cells = 163 km a side, 20 km bar; NW light): `images/R1h-plain-band-tilt4-crop.png` against `images/R1h-tree-tilt4-crop.png`; boxes `R1h-*-box.png` (505 km, 100 km bar); R3b crops. Basins outlined in red.
- The plain band is ridge-rings everywhere (every crest closes a basin); the tree surface has broad valley floors with fine dendritic ridges on the slopes, much as a drained mountain land looks; the residual basins are the plateau and belt terms of e0 and a few closed floors. Artefacts: a straight horizontal edge at the left of the R1h crop (a flat of e0 routed along a row: the flats' straight-run flow of SPIKE-108 is inherited by the tree), and valley floors at the channels' flat width.
- Low-sun oblique, R1h: `images/R1h-plain-band-tilt4-oblique.png` and `R1h-tree-tilt4-oblique.png`: pinhole camera at face 1 cell (8526, 960) (window cell (670, 400)), 1,500 m above the ground, azimuth 350 deg (north-north-west, clockwise from up the image), pitch -7 deg, horizontal field of view 70 deg, sun from 80 deg (east) at 14 deg elevation with cast shadows; 26.0 m (plain band) and 19.5 m (tree) per pixel at the screen centre (18.6 and 13.9 km away); the scale bar is the bar at the lower left at the centre's scale. A simple height-field ray marcher over the 814 m cells (no texture; hard grid-scale shadow edges): relief is modest at this scale (R1h's 5 x 5 relief p95 is 0.5 to 0.7 km), the tree surface reads as broad valleys with smooth slopes, the plain band as lumpy ridges.

## Not done in this cut (the time-box has 1.4 days left)
- The tree as a function of (seed, point): a coarse global tree lattice (above), the order of a channel (Strahler) instead of the area tiers, tributary junction smoothing, a valley-floor width law.
- The plateau fix and the bounded tilt (the bars are read with my S-tilt stand-in at 4 m/km, +1.0 to +1.5 km of height at the medians; refused as a fix); the effect on the erosion bake; hypsometry (the relief is positive on a floor: the median height falls by the plain band's mean, like SPIKE-110's variants); class weights.
- More windows and seeds (three fixtures only), a second look pass.

## Reproduction
- Local branch `spike/spike-111-probe`; `cargo build --release -j 4 -p planet-generators --example shapes_check`; `PLANET_WIN=<window> PLANET_MARGIN=200 PLANET_TREES="none;tree:16:0.6:1.0:0.5" PLANET_TILTS=0,2,4,8 shapes_check z0tree 8`; images with `PLANET_IMG_DIR`, `PLANET_CROP=x0,y0,size`, `PLANET_OBL=x,y,height_m,azimuth,pitch,fov,sun_az,sun_el` (x, y in box cells). A window run is 1 to 2 minutes.
- Raw: `raw/tree-f-*.txt` (the three windows, the chosen spec and the 1.0 length variant at four tilts, `BASINRAW`, `RELIEF`, tier and timing lines), `raw/tree-r1h-a.txt`, `raw/tree-r1h-b.txt` (the grid).
