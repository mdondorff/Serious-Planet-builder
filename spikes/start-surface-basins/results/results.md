# SPIKE-109: where the start surface's closed basins come from

Measurement only; no verdict (the architect's). Measured 2026-11-12 by the spikes lane on the start surface z0 of the production base (`BakeInputs::base`) of three fixtures, against the frozen GLO-30 pool (`spikes/shapes-check/results/dem-pool-frozen.md`).
Probe: component switches in `classes.rs`, `global.rs`, `bake/inputs.rs`, `bake/round.rs` (`probe-src.diff`) and the example `shapes_check.rs.txt` (modes `z0basins`, `z0sweep`, `z0breach`); the probe lives on the local branch `spike/spike-109-probe`, nothing of it is in a PR.

## Definitions
- z0 = `BakeInputs::base(d)` on the generator window: `ClassSource::eval` (coast, belts, back-arc, old margin, plateau uplift) plus the class band (`amp x noise x smoothstep`; amp = `hill_amplitude_for_k(k)` x (1 + (MOUNTAIN_BAND_SCALE - 1) x mountain weight); ridged 5 octaves 16/8/4/2/1 km on the mountain weight, hill 3 octaves 8/4/2 km otherwise).
- Lifted cell: fill depth at least 0.5 mm under U routing with the canonical flood. Basin = 8-connected component of lifted cells. Interior = no cell on the window's outer ring (margin 200 cells).
- Share = interior basins over 30 cells, % of the box's land cells. Depth = a basin's largest fill depth; p50 / p95 over the basins over 30 cells.
- Windows (face, cells per face edge, box i0-i1, j0-j1, seed; 814 m cells; window margin 200): R1h `1,12288,8056,8675,760,1379,14` (the owner's region, box 620 x 620 cells = 505 km); R3a `4,12288,168,1187,4520,5539,15`; R3b `2,12288,9768,10787,8104,9123,15`.
- Breach = round 1 of `catchment_round` alone (`PROBE_BREACH_ONLY`, the sweeps off, one round under `RoundRules::ROUND`): for each donor c, receivers first, `z[rec] = max(z[c] - breach_slope x dist, floor)`, at most `breach_limit` = 500 m per round; floor = sea level + 0.001 m.

## (a) Size and depth against the DEM pool
| Set | land cells | share over 30 cells | basins over 30 cells | depth p50 / p95 / max (m) | cells p50 / p95 / max | share deeper than 100 m |
|---|---|---|---|---|---|---|
| R1h z0 | 267,717 | 32.25 % (interior) | 996 | 79.9 / 809.8 / 1,321 | 81 / 1,027 / 18,191 | 21.67 % |
| R3a z0 | 727,199 | 21.97 % | 1,227 | 7.9 / 16.0 / 453.8 | 64 / 550 / 18,641 | 2.55 % |
| R3b z0 | 883,645 | 22.52 % | 1,960 | 6.8 / 14.5 / 130.0 | 63 / 385 / 23,185 | 0.74 % |
| DEM high pool (8 tiles, slope p95 0.2 to 0.7) | 66,459 | 3.67 % (tile-edge basins included) | 36 | 45.3 / 134.3 / 170.1 | 48 / 185 / 480 | 0.32 % |
| DEM mid pool (4 tiles, slope p95 0.03 to 0.14) | 42,716 | 0.62 % | 6 | 10.1 / 17.1 / 42.1 | 40 / 62 / 97 | 0.00 % |

- Excess against the matching pool: R1h 8.8 x the high pool's share and 1.8 x its depth p50; R3a 35 x and R3b 36 x the mid pool's share, at a depth p50 of 0.8 and 0.7 x the pool's (the R3 excess is many shallow basins).
- Limit of the comparison: the DEM tiles are about 100 x 138 cells, so a basin much larger than about 100 cells in a side cannot appear in them (pool cells p95 185, max 480; generator cells p95 1,027 / 550 / 385 and max 18,191 / 18,641 / 23,185); the DEM share is a lower bound for the planet's scale, the generator's depth tail (R1h p95 810 m) has no DEM counterpart at this tile size.
- Raw: `raw/sweep-*.txt` (mask 0 = baseline, line `BASINRAW`), `raw/dem-high.txt`, `raw/dem-mid.txt` (line `DEMBASINS`, `pooled DEM, routed U`).

## (b) Component sweep
Masks are bits of `PLANET_BASE_OFF`. Share (%) and depth p50 / p95 (m) per window. In R3a and R3b only the coast term, the class band and the plateau are active (no belts, back-arc or old margin there), so those switches change nothing in the R3 columns.

| Mask | Switch | R1h share | R1h depth p50 / p95 | R3a share | R3a depth p50 / p95 | R3b share | R3b depth p50 / p95 |
|---|---|---|---|---|---|---|---|
| 0 | baseline (all components on) | 32.25 | 79.9 / 810 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 1 | ridged octave 16 km off | 31.26 | 88.7 / 418 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 2 | ridged octave 8 km off | 32.73 | 76.5 / 697 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 4 | ridged octave 4 km off | 32.25 | 79.7 / 768 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 8 | ridged octave 2 km off | 32.54 | 79.4 / 797 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 16 | ridged octave 1 km off | 32.33 | 79.3 / 807 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 32 | hill octave 8 km off | 27.72 | 47.0 / 817 | 17.37 | 4.7 / 8.4 | 15.62 | 4.3 / 8.7 |
| 64 | hill octave 4 km off | 33.47 | 70.5 / 801 | 23.02 | 6.9 / 13.4 | 22.92 | 6.1 / 13.2 |
| 128 | hill octave 2 km off | 33.11 | 72.6 / 797 | 22.60 | 7.0 / 14.6 | 23.42 | 6.0 / 13.6 |
| 256 | whole class band off | 12.24 | 9.5 / 343 | 11.79 | 2.9 / 132.3 | 10.89 | 4.7 / 58.9 |
| 512 | belt ridge factor to its mean | 32.08 | 79.3 / 808 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 1024 | belts off | 33.87 | 76.8 / 460 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 2048 | back-arc off | 32.19 | 79.6 / 810 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 4096 | old margin and flank off | 33.72 | 79.6 / 805 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 8192 | plateau uplift off | 33.52 | 79.0 / 760 | 29.81 | 8.1 / 15.2 | 30.45 | 6.8 / 15.0 |
| 16384 | mountain scale x4.5 to 1 | 25.31 | 51.5 / 167 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 32768 | class amplitude to 30 m | 17.49 | 13.5 / 51 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 65536 | class weights direct (no C1 lattice, no arid blend) | 32.08 | 79.3 / 816 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 131072 | ridged band replaced by the hill band | 25.87 | 79.8 / 199 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |
| 15616 | isolation: coast term only (band, belts, back-arc, old margin, plateau off) | 19.84 | 3.5 / 28 | 11.90 | 1.7 / 8.9 | 17.26 | 3.3 / 30.2 |
| 15360 | isolation: band only (belts, back-arc, old margin, plateau off) | 41.32 | 66.0 / 330 | 29.81 | 8.1 / 15.2 | 30.45 | 6.8 / 15.0 |
| 8448 | isolation: belts and arcs only (band and plateau off) | 7.67 | 5.4 / 68 | 11.90 | 1.7 / 8.9 | 17.26 | 3.3 / 30.2 |
| 98304 | isolation: flat k (amplitude 30 m and direct weights) | 17.49 | 13.5 / 51 | 21.97 | 7.9 / 16.0 | 22.52 | 6.8 / 14.5 |

Reading (the numbers above; no mechanism beyond what a switch shows):
- Class band (bit 8, whole band off): R1h 32.25 -> 12.24 % (depth p95 810 -> 343 m), R3a 21.97 -> 11.79 %, R3b 22.52 -> 10.89 %. It is the largest single component in all three windows.
- Inside the band, the amplitude: class amplitude to 30 m (bit 15) R1h 17.49 % and p95 51 m; mountain scale x4.5 to 1 (bit 14) R1h 25.31 % and p95 167 m; ridged to hill band (bit 17) R1h 25.87 % and p95 199 m (the basin count rises 996 -> 1,521, smaller ones). R3a and R3b have no mountain weight, so bits 14, 15 and 17 change nothing there (their amplitude is already at or under 30 m).
- Inside the band, the octaves: the hill octave of 8 km (bit 5) removes 4.5 / 4.6 / 6.9 points (R1h / R3a / R3b) and halves the depth p50 (79.9 -> 47.0, 7.9 -> 4.7, 6.8 -> 4.3 m). The hill octaves of 4 and 2 km raise the share by 0.4 to 1.2 points when removed (they break basins up). The ridged octave of 16 km (bit 0) sets the deep tail in R1h: its removal moves p95 810 -> 418 m and the maximum 1,321 -> 615 m at a share change of -1.0; the ridged octaves of 8, 4, 2, 1 km change the share by at most 0.5 points (the 8 km one moves p95 810 -> 697 m).
- Belts (bit 10, R1h only): off gives 33.87 % (+1.6) and p95 810 -> 460 m (maximum 1,321 -> 899 m): they make the deepest basins and drain shallow ones. Belts and arcs alone, band off (mask 8448): 7.67 %, p95 68 m.
- Plateau uplift (bit 13): the deep bowls of R3: off, the maximum depth falls 453.8 -> 38.3 m (R3a, the 16,475-cell basin) and 130.0 -> 64.4 m (R3b); the share rises 21.97 -> 29.81 and 22.52 -> 30.45 %. In R1h nothing visible (33.52 %, p95 760 m).
- Coast term alone (mask 15616): 19.84 / 11.90 / 17.26 % at a depth p50 of 3.5 / 1.7 / 3.3 m, p95 28 / 8.9 / 30 m, maximum 59 / 21 / 49 m. Not the band: flat plains of a few metres to 60 m of relief carry big shallow basins (R1h: 59 m / 32,914 cells at the top). With belts added (8448) R1h falls to 7.67 %: slope drains them.
- No visible effect on the deep tail (share within 0.2 points, except the old margin and flank at +1.5): back-arc (bit 11), old margin and flank (bit 12), the belt ridge factor (bit 9), the class weights from the source instead of the 256-cell C1 lattice and the arid blend (bit 16: R1h 32.08 %).
- R1h with the band off (mask 256) keeps 12.24 %, a few large basins (the band-off image in (d)); with the plateau off as well 7.67 % remains (coast, belts, arcs).

Components that make the excess, in order of share removed: the class band's amplitude in the mountain class (R1h) and its 8 km hill octave (R1h, R3a, R3b); the 16 km ridged octave and the belts for the depth tail (R1h); the plateau uplift for the single deepest bowls (R3a 454 m); the smooth coast plain for a floor of shallow basins in every window. The C1 lattice step, the back-arc and the old margin make none.

## (c) Candidate fix per component (nothing built)
Estimates: PRs and days of lane work; the check after each is this sweep (about 1.5 min per mask and window when the machine is quiet). Quota not estimated (the lane does not see it). Every change in the base changes every generated start surface, so the golden-producing views of generated terrain go through `golden-review`.

| Component | Candidate at the source | Estimate | What it does not do |
|---|---|---|---|
| Class band amplitude (k, mountain scale) | cap each band octave's amplitude by what the base surface's local slope can drain, amp(octave) <= c x slope(e0) x wavelength / 2, e0's slope from two extra `elevation_m` samples; or lower `MOUNTAIN_BAND_SCALE` / the k-amplitude table | 1 PR in `classes.rs` plus the band amplitude requirement; 2 days with re-bless | relief on flat plains drops to the cap; the owner's look call on mountain relief; the coast-plain floor stays |
| Hill octave 8 km | weight the 8 km octave down (one constant) or move it to the mountain class only | 1 PR, 0.5 day | takes 4.5 to 6.9 points of 22 to 32 |
| Ridged octave 16 km (depth tail) | amplitude lowered or its wavelength pushed past the belt spacing | 1 PR, 0.5 day, with the band PR | share barely moves (-1.0) |
| Belts | the same slope cap applied to the belt term's own relief; the belts are not separable from the band in R1h (band only: 41.3 %, belts and arcs only: 7.7 %) | within the band PR | not measured apart |
| Plateau uplift | make the uplift monotone from rim to interior (smoothstep of the distance to the plateau edge) so the interior cannot form a bowl | 1 PR, 1 day | one 454 m basin (R3a) and the 130 m basin (R3b); the share rises when the plateau is off (+7.8 points), so this is a depth fix |
| Coast plain (floor of shallow basins) | a regional tilt toward the sea added to e0 (an epsilon gradient per metre of distance to the coast) | 1 PR, 1 day; the tilt value needs a probe | basins of 1 to 60 m depth; the rounds already cut metres |
| Lattice, back-arc, old margin | none (no excess) | none | |
| Not a source fix | a pit-fill or breach pass inside the base is round 0's breach again (GEN-174), reported as shelved | n/a | |

## (d) The owner's region at z0 with the basins outlined
- `images/z0-R1h-basins.png`: top-down shaded relief from the north-west (gradient of z0, 0.7071 of the diagonal), R1h box = face 1 (12,288 cells per edge, seed 14), cells i 8056 to 8675, j 760 to 1379, 620 x 620 px at 814 m per pixel (505 km a side); sea dark blue; basins over 30 cells outlined red (interior) with a red tint inside; scale bar 100 km at the lower left. Window margin 200 cells (not drawn). No camera: the image is a map, not a view; there is no altitude, pitch or field of view.
- `images/z0-R1h-basins-band-off.png`: the same crop with the class band switched off (`PLANET_BASE_OFF=256`): 66 basins over 30 cells, share 12.24 %, depth p50 9.5 m, p95 343 m.
- In the first image the basins cover the rough north-east as many small ones between ridges, a large one at the top, and the plain in the west as a swarm of small ones; the band-off image keeps the large basins only.

## (e) Basin depth against the round-1 breach it causes
Method: z0 breached by round 1 alone, per basin over 30 cells the cells lowered by at least 0.5 m in its footprint (the basin plus 2 steps) and the lowering on its receiver path pit -> sill -> pit level (the stretch the breach cuts). The breach lowers the downstream path, not the basin: the deepest R1h basins have 0.0 m of lowering in their own footprint.

| Window | cells lowered >= 0.5 m | of them in a basin footprint | lowering volume in footprints | cells on the pit-sill-pit paths | volume on the paths | correlation ln(1 + fill) with ln(1 + max cut on path) | correlation with max lowering in footprint |
|---|---|---|---|---|---|---|---|
| R1h | 50,540 | 81.8 % | 93.3 % | 14.8 % | 31.0 % | 0.77 | -0.09 |
| R3a | 123,026 | 73.1 % | 87.3 % | 10.5 % | 31.6 % | 0.71 | 0.14 |
| R3b | 162,234 | 70.3 % | 81.0 % | 9.4 % | 36.3 % | 0.46 | 0.08 |

Per depth bin of fill depth (basins / max cut on the path p50 / p95 / max, m / basins with the cut at the 500 m round limit):

| Fill depth | R1h | R3a | R3b |
|---|---|---|---|
| under 1 m | 0 | 25 / 1.1 / 3.7 / 5.5 / 0 | 2 / 3.8 / 3.8 / 5.6 / 0 |
| 1 to 10 m | 60 / 14.7 / 376.5 / 500 / 2 | 827 / 15.8 / 57.4 / 263 / 0 | 1,532 / 19.7 / 223 / 433 / 0 |
| 10 to 100 m | 517 / 76.9 / 279 / 500 / 6 | 369 / 28.8 / 107 / 376 / 0 | 424 / 58.6 / 330 / 434 / 0 |
| 100 to 300 m | 205 / 237.9 / 444 / 500 / 4 | 5 / 185 / 231 / 262 / 0 | 2 / 177 / 177 / 262 / 0 |
| over 300 m | 214 / 500 / 500 / 500 / 140 | 1 / 476 / 476 / 476 / 0 | 0 |

- The eight deepest R1h basins (fill 1,321 to 1,123 m) all have the path cut at the 500 m round limit (paths of 25 to 376 cells, sill over pit equal to the fill depth).
- R3a, deepest basin: fill 454 m / 16,475 cells, cut 476 m on a 313-cell path, 2,096 cells lowered in its footprint; next: 205 m -> 261 m, 155 m -> 185 m, 148 m -> 154 m. R3b: 130 m -> 177 m, 101 m -> 262 m, 66 m -> 249 m, 56 m -> 371 m.
- After round 1 alone the interior share over 30 cells is 5.35 / 5.38 / 1.39 % (R1h / R3a / R3b; from 32.25 / 21.97 / 22.52 %); after the 4 rounds 1.48 / 6.34 / 2.70 %. Cells at the 0.001 m floor after the breach alone: 4,970 / 44,143 / 12,873.
- Per basin lines: `raw/breach-*.txt` (`BREACHBIN`, `PATHBIN`, `DIAG basin`, `PATHS`, `PATHCORR`).

## (f) Cells held at exactly 5.001 m (baseline)
Measured on the probe window (margin 200, the 4 rounds under production rules, GEN-174's round-0 breach off; probe branch based on main 70d6e7f6), per window (`FIVEBASE`):

| Window | cells at 5.001 m after 4 rounds | cells at 0.001 m after 4 rounds | at 5.001 m after the breach alone |
|---|---|---|---|
| R1h | 1,208 | 1,166 | 0 |
| R3a | 890 | 6,547 | 0 |
| R3b | 3,543 | 1,842 | 0 |

- The earlier baseline (K7 tile geometry, main e129414d, GEN-174 off): R3b 4,369, R3a 1,666, R1h 1,059. The two sets differ in geometry (the probe window runs the 4 rounds on the window, the K7 numbers on a halo-padded tile); compare each set only against itself after a fix.
- The plateau is not there after the breach alone (0 cells at 5.001 m): it forms in the rounds (pit sweep plus 5 steps of 1.0 m on the floor-cut cells, per `round.rs` lines 55 to 82 of that main).

## Reproduction
- Branch `spike/spike-109-probe` (local), build `cargo build --release -j 4 -p planet-generators --example shapes_check`; `shapes_check.rs.txt` is the example, `probe-src.diff` the changes in `crates/generators/src`.
- Env: `PLANET_WIN=<window>`, `PLANET_MARGIN=200`, `PLANET_TAG`, `PLANET_MASKS=0,1,2,...` (z0sweep), `PLANET_ROUNDS4=1` (z0breach: the 4-round table), `PLANET_BASE_OFF=<mask>` (z0basins), `PLANET_BASIN_IMG=<png>`; DEM: `PLANET_SLOPE=0.2,0.7` or `0.03,0.14` with `shapes_check dem`.
- Runs: BelowNormal, one at a time; `z0sweep` is 23 masks per window (80 to 97 s a mask when the machine is quiet, several minutes when loaded), the DEM pools about 1 min each, an image about 1 min.
