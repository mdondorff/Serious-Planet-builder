# SPIKE-92: continent candidates for M9 generator round 2

Status: round 1 closed 2026-11-02 (7e68ebb8); round 2 (inbox #65, the hybrid and S9/S10) closed 2026-11-02 (efec4475); round 3 (inbox #66, islands, polar land, 3D noise) closed 2026-11-02 (79c9d663 on `spike/m9-continents`, never merged); patch of all rounds in `proto/spike92.patch`. Winner: H2r3 (round 3).
Planner review of round 1: B2 passes S1-S3 but is round blobs with noisy rims (S3 met by rim noise); A looks Earth-like but fails on sizes. Round 2 answers that.

## Question
Which continent stage passes S1-S3 (SPIKE-91) and about 29 % land on seeds 1-5: a tuned fBm (A) or constrained multi-scale noise (B)?

## Gates (SPIKE-91, Earth from ETOPO 2022 at 0.25°)

| Metric | Earth | Gate |
|---|---|---|
| S1 land masses ≥ 1 Mkm² | 5 | 4-7 |
| S2 largest share of land | 54 % | 45-62 % |
| S3 coastline box dimension (2-32 cells) | 1.22 | ≥ 1.18 |
| Land fraction | 29.1 % | 27-31 % |

## Method
- Only the continent value of `global::terrain` changes (`crate::spike92::continent`, switched by `S92_*` environment variables); belts, classes and the rest of the pipeline are today's.
- Each candidate is shifted per seed so that 29 % of the sphere is land: the offset is the 71 % quantile of the raw value over a fixed Fibonacci grid of 20,000 directions.
- Maps: the merged preview example at 1440 × 720 (0.25°), seeds 1-5, plus a `land.u8` mask (tooling only, in the example). Scores: `proto/s92_score.py` (SPIKE-91 definitions, wrap-around labelling, cell areas by latitude).
- Deterministic and libm only (CON-14): splitmix hashing, memoized per seed, no thread-dependent state.

## Candidates

| Candidate | Raw value |
|---|---|
| base | today's: warped fBm, frequency 1.1, 5 octaves, gain 0.5, cut 0.1 |
| A | warped fBm with a per-octave gain: frequency `S92_FREQ`, octaves `S92_OCT` (7), warp `S92_WARP` (0.6), gain `S92_GAIN` |
| B | max over n random cratons of `1 - angle/r` (log-normal radii) + mid fBm + high fBm |
| B2 | ranked cratons: each cap holds its share (`S92_SIZES`) of 29 % land, centres at least r_i + r_j + 0.15 rad apart; `max(r_i - angle)` + `mid`·fBm(3.0, 3 oct) + `high`·fBm(6.0, 7 oct, gain); warp 0.2 |

- B2 sizes: default `0.54,0.26,0.09,0.05,0.02,0.015,0.01,0.01`; `B2e` = `0.55,0.27,0.09,0.05,0.015,0.005,0.004,0.003` (fewer mid-size cratons, so small islands stay under 1 Mkm²).
- Config names: `f` frequency, `g` gain, `w` warp, `m` mid, `h` high, `r`/`s`/`c` craton radius, spread, count.

## Results (seeds 1-5, left to right)

| Config | land % | S1 | S2 % | S3 | all pass |
|---|---|---|---|---|---|
| base | 30 38 54 34 63 | 2 2 3 4 1 | 79 98 47 83 100 | 1.11 1.09 1.08 1.07 1.05 | 0/5 |
| A0 (f1.8 g.50) | 29 29 29 29 29 | 5 7 6 8 4 | 61 39 40 37 84 | 1.13 1.10 1.13 1.12 1.15 | 0/5 |
| A-f0.9-g.60-w.8 | 29 29 29 29 29 | 5 6 9 5 5 | 65 33 41 54 60 | 1.20 1.22 1.24 1.21 1.19 | 2/5 |
| A-f1.0-g.60 | 29 29 29 29 29 | 5 6 8 6 4 | 68 47 32 54 61 | 1.21 1.21 1.22 1.19 1.23 | 3/5 |
| A-f1.2-g.60 | 29 29 29 29 29 | 3 5 8 9 7 | 95 43 78 50 56 | 1.19 1.21 1.26 1.20 1.26 | 1/5 |
| A-f1.2-g.60-w.3 | 29 29 29 29 29 | 4 5 8 9 3 | 94 40 59 51 94 | 1.22 1.22 1.25 1.20 1.24 | 0/5 |
| A-f1.2-g.65 | 29 29 29 29 29 | 4 7 9 9 7 | 93 40 67 49 43 | 1.25 1.28 1.34 1.24 1.30 | 0/5 |
| A-f1.5-g.60 | 29 29 29 29 29 | 8 6 7 9 8 | 83 35 34 40 44 | 1.22 1.23 1.25 1.23 1.23 | 0/5 |
| A-f1.5-g.65 | 29 29 29 29 29 | 9 6 7 8 9 | 80 35 33 43 43 | 1.27 1.28 1.31 1.27 1.30 | 0/5 |
| B0 | 29 29 29 29 29 | 3 5 3 4 4 | 94 58 87 33 42 | 1.03 1.13 1.14 1.10 1.02 | 0/5 |
| B-r.35-s.6-h.25 | 29 29 29 29 29 | 3 5 3 4 4 | 95 59 88 34 44 | 1.09 1.13 1.15 1.16 1.06 | 0/5 |
| B-r.30-s.8-h.2 | 29 29 29 29 29 | 2 5 3 4 4 | 97 67 93 36 47 | 1.09 1.14 1.15 1.16 1.05 | 0/5 |
| B-r.25-s.9-h.25-c9 | 29 29 29 29 29 | 4 5 4 6 4 | 95 70 87 28 51 | 1.07 1.14 1.14 1.05 1.04 | 0/5 |
| B2-m.08-h.08 | 29 29 29 29 29 | 8 7 7 8 8 | 53 58 54 51 54 | 1.05 1.15 1.05 1.04 1.06 | 0/5 |
| B2-m.12-h.12 | 29 29 29 29 29 | 7 7 7 8 8 | 53 58 53 51 54 | 1.08 1.17 1.09 1.07 1.11 | 0/5 |
| B2-m.15-h.20 | 29 29 29 29 29 | 7 8 6 8 8 | 52 58 53 51 53 | 1.16 1.20 1.18 1.12 1.19 | 0/5 |
| B2e-m.15-h.25-g.65 | 29 29 29 29 29 | 4 6 6 7 7 | 54 59 54 52 55 | 1.21 1.26 1.21 1.17 1.23 | 4/5 |
| B2e-m.12-h.35-g.68 | 29 29 29 29 29 | 4 5 6 7 8 | 54 59 53 51 54 | 1.29 1.29 1.29 1.24 1.29 | 4/5 |
| **B2e-m.20-h.30-g.65** | 29 29 29 29 29 | 4 6 6 7 7 | 54 58 53 51 54 | 1.24 1.26 1.25 1.19 1.24 | **5/5** |
| **B2e-m.15-h.30-g.70** | 29 29 29 29 29 | 4 5 6 7 7 | 54 59 53 51 54 | 1.29 1.31 1.29 1.24 1.30 | **5/5** |

Raw per-seed scores, including the largest seven areas: `raw/scores.json`.

## Findings
1. The land offset alone fixes the land fraction for every candidate (29 % on all seeds; today's ranges 30-63 %).
2. Coast roughness is the fBm gain: 0.5 gives a dimension of 1.03-1.17 in every candidate; 0.60-0.70 gives 1.19-1.34.
3. A (tuned fBm) cannot hold the size distribution: at any frequency, S2 ranges over 32-95 % across seeds and S1 over 3-9. Best A: f1.0 g.60, 3/5.
4. B with random craton radii inherits the same spread (S2 28-97 %).
5. B2 (ranked cratons) sets S2 by construction (51-59 % on every seed); S1 follows the share list and the mid/high amplitudes; the high-frequency gain sets S3.
6. Best fit to Earth's S3 (1.22): B2e-m.20-h.30-g.65 (1.19-1.26). B2e-m.15-h.30-g.70 has more margin on S3 (1.24-1.31) and is rougher than Earth.
7. Seed 4 is the weakest in every B2 row (S3 lowest, S1 = 7 at the band's edge).

## Best two

| Rank | Config | Environment |
|---|---|---|
| 1 | B2e-m.20-h.30-g.65 | `S92_CANDIDATE=B2 S92_SIZES=0.55,0.27,0.09,0.05,0.015,0.005,0.004,0.003 S92_MID=0.20 S92_HIGH=0.30 S92_GAIN=0.65` |
| 2 | B2e-m.15-h.30-g.70 | same sizes, `S92_MID=0.15 S92_HIGH=0.30 S92_GAIN=0.70` |

- Land-sea maps (committed): `images/<config>/land-sea-seed-N.png`; side by side with today and best A: `images/land-sea-today-A-B2.png`.
- Elevation maps (local, about 1 MB each, not committed): `target/s92/<config>/seed-N/elevation.png` in the `sonnet-spikes` worktree; reproduce with `proto/s92_sweep.py` on the spike branch.

## Visuals
- `images/land-sea-today-A-B2.png`: seeds 1-5 (rows) for today, A-f1.0-g.60 and B2e-m.20-h.30-g.65 (columns); green land, blue sea.
- `images/B2e-m.20-h.30-g.65/land-sea-seed-N.png` and `images/B2e-m.15-h.30-g.70/land-sea-seed-N.png`: the best two at 1440 × 720.

## Limits
- Craton centres are uniform random with a separation rule; there is no plate or supercontinent logic, so placement is not Earth-like (S4+ of SPIKE-91 not scored).
- Belts and classes still come from today's stage and do not follow the new coasts; round 1 (belts) and round 2 must be joined in M9.
- One resolution only (0.25°); S3 at other scales not checked.

# Round 2: the hybrid and two shape metrics (inbox #65)

## Shape metrics (new; `proto/s92_shape.py`, masses of at least 1 Mkm²)

| Metric | Definition | Earth (ETOPO 2022, 0.25°) | Disc | Gate |
|---|---|---|---|---|
| S9 compactness | area-weighted median of 4πA/P²; P the contour length (marching squares at 0.5) of the mass's land fraction on a 1° grid (about 111 km), E-W steps scaled by cos(latitude) | 0.096 (per mass 0.10-0.49) | 1.00 | ≤ 0.20 |
| S10 elongation | median over masses of √(λ1/λ2), the eigenvalues of the area-weighted second moment in the tangent plane at the mass's mean direction | 1.77 (per mass 1.19-3.10) | 1.0 | 1.5-2.5 |

- Earth's masses at 0.25° (ETOPO surface > 0; mask by `proto/s92_earthmask.py`): Afro-Eurasia 79.8 Mkm² (S9 0.10, S10 1.77), the Americas 38.9 (0.10, 3.10), Antarctica 13.7 (0.37, 1.19), Australia 7.7 (0.49, 1.51), Greenland 2.2 (0.39, 2.09).
- The disc reference scores 1.00-1.07 (2-40 Mkm²): the contour perimeter removes the staircase bias of a cell-edge perimeter (which gives a disc 0.65).
- The area-weighted S9 is dominated by the largest mass; the S9 gate is about twice Earth's value. The S10 band is the planner's, around Earth's 1.77.
- Round 1 against them: B2 scores S9 0.33-0.48 and S10 1.1-1.3 (fails both on every seed); A scores S9 0.06-0.15, S10 1.6-2.4 (passes both).

## Hybrid H (`S92_CANDIDATE=H`)
raw = A's warped fBm × margin weight + k × Σ lobe weights, then round 1's per-seed land offset.

| Part | Definition | Knob (final) |
|---|---|---|
| Shape | A's warped fBm, 8 octaves | `S92_FREQ` 1.6, `S92_GAIN` 0.72, `S92_WARP` 0.6 |
| Cratons | ranked area shares of the land | `S92_SIZES` 0.50,0.26,0.10,0.06,0.03,0.02,0.01,0.01 |
| Lobes | a craton of ≥ 20 % of the land is a chain of `S92_LOBES` lobes, of 4-20 % 1 to `S92_LOBES`, smaller 1; lobes split the craton's area at random; the spine turns up to `S92_TURN` rad per lobe; centres `S92_SPACING` × the two major radii apart | 3, 1.2, 0.6 |
| Lobe weight | exp(-ρ²), ρ the elliptical distance (radii r·√a and r/√a, aspect a uniform in 1.5..`S92_AMAX`), on the point warped by `S92_CWARP` | `S92_AMAX` 2.5 or 3.0, `S92_CWARP` 0.3 |
| Bias strength | k | `S92_K` 0.8-1.0 |
| Margin weight | the fBm's amplitude is 1 on craton margins (bias 0.5), `S92_DEEP` in the deep ocean and craton interiors: deep + (1 − deep)·4w(1 − w), w = min(bias, 1) | `S92_DEEP` 0.3 |
| Layout retry | the generator scores its own craton layout on a 2° grid (29 % land by area, 8-connected masses with wrap) and redraws it from a derived seed (up to `S92_RETRY` draws) until 4-7 masses ≥ 1 Mkm² and the largest 47-60 %; else it keeps the closest draw | `S92_RETRY` 12 |

- Deterministic and libm only (CON-14): the layout grid, the draw order and the tie-break are fixed per seed. The retry costs up to 12 × 16,200 evaluations per seed, once per process.

## Results (seeds 1-5)

| Config | S1 | S2 % | S3 | S9 | S10 | all |
|---|---|---|---|---|---|---|
| A-f1.0-g.60 (8 oct) | 5 6 8 6 4 | 68 47 32 54 61 | 1.21 1.21 1.22 1.19 1.23 | 0.15 0.12 0.06 0.15 0.06 | 1.8 1.6 2.4 1.8 1.6 | 3/5 |
| B2e-m.20-h.30-g.65 | 4 6 6 7 7 | 54 58 53 51 54 | 1.24 1.26 1.25 1.19 1.24 | 0.37 0.48 0.33 0.42 0.37 | 1.1 1.3 1.2 1.3 1.2 | 0/5 |
| H-k.3 (f1.0, single ellipses) | 6 6 10 4 7 | 64 44 22 69 45 | 1.18 1.19 1.18 1.14 1.22 | 0.21 0.10 0.15 0.10 0.09 | 1.5 2.0 2.6 2.3 1.9 | 1/5 |
| H-f1.6-k1.0 | 7 8 7 6 11 | 48 39 46 52 34 | 1.28 1.25 1.22 1.23 1.27 | 0.11 0.09 0.16 0.07 0.13 | 2.3 1.8 1.9 2.0 1.8 | 3/5 |
| H-f2.0-k.8 | 10 9 4 7 5 | 43 31 73 30 75 | 1.29 1.29 1.26 1.30 1.30 | 0.06 0.11 0.04 0.08 0.03 | 1.9 1.5 2.2 2.1 1.7 | 0/5 |
| H-k1.5-p2 (flat-top weights) | 8 6 6 5 7 | 42 50 53 51 47 | 1.18 1.19 1.15 1.18 1.19 | 0.37 0.18 0.36 0.30 0.40 | 1.7 2.2 2.2 2.2 2.0 | 1/5 |
| H-k2.0-p2-a2.2 | 5 7 6 9 6 | 45 53 52 56 49 | 1.11 1.12 1.08 1.15 1.15 | 0.54 0.50 0.75 0.46 0.55 | 1.6 1.9 1.5 2.6 1.6 | 0/5 |
| L3-k1.5 (3 lobes) | 6 4 10 5 3 | 39 95 43 62 59 | 1.24 1.23 1.19 1.19 1.23 | 0.16 0.05 0.19 0.10 0.15 | 1.6 1.9 2.3 2.1 2.0 | 1/5 |
| L3-k1.0-d.3 (+ margin weight) | 4 3 6 4 3 | 45 60 45 60 60 | 1.23 1.19 1.20 1.24 1.23 | 0.17 0.09 0.19 0.09 0.15 | 1.8 1.9 2.1 1.5 1.7 | 1/5 |
| L3z-k1.0-sp.5 (+ sizes, spacing) | 6 7 6 6 7 | 39 45 50 49 56 | 1.23 1.17 1.19 1.14 1.25 | 0.15 0.23 0.12 0.31 0.11 | 2.0 2.5 2.1 1.9 1.7 | 2/5 |
| R-k1.0-sp.6-g.7 (+ retry) | 7 4 6 5 5 | 55 59 56 52 52 | 1.22 1.22 1.27 1.23 1.27 | 0.40 0.13 0.16 0.19 0.12 | 1.4 1.8 2.1 2.0 2.3 | 4/5 |
| R3-k1.0-sp.6 (+ full lobes ≥ 20 %) | 4 5 6 5 5 | 56 56 52 51 50 | 1.20 1.18 1.24 1.23 1.24 | 0.14 0.16 0.16 0.16 0.12 | 1.6 2.3 2.4 1.6 1.6 | 4/5 |
| R4-k0.8-sp.6 (4 lobes) | 4 6 4 6 5 | 58 58 57 50 57 | 1.23 1.18 1.29 1.25 1.23 | 0.09 0.23 0.08 0.14 0.17 | 2.1 2.3 2.6 2.9 2.1 | 2/5 |
| R3-k0.8-a2.5 | 5 7 7 5 4 | 58 53 49 52 56 | 1.25 1.21 1.24 1.25 1.28 | 0.14 0.17 0.19 0.13 0.09 | 2.0 2.4 2.1 1.8 2.2 | 5/5 |
| **H1: R3-k1.0-g.72** | 4 5 6 5 5 | 56 56 52 51 51 | 1.22 1.20 1.27 1.25 1.25 | 0.13 0.15 0.14 0.15 0.11 | 1.6 2.3 2.4 1.6 1.6 | **5/5** |
| **H2: R3-k0.8-a2.5-g.72** | 5 7 7 5 4 | 58 52 50 52 56 | 1.27 1.23 1.26 1.28 1.29 | 0.12 0.15 0.18 0.11 0.09 | 2.0 2.4 2.1 1.8 2.2 | **5/5** |

- Land is 29 % on every seed of every row. Raw scores: `raw/scores-round2.json`.
- Rows above `R3-` ran on intermediate states of the spike code (the lobe draw order changed between them); the `R3-` rows, A and B2 reproduce on efec4475.

## Findings (round 2)
1. S9 and S10 separate the candidates: B2's discs fail both on every seed and A passes both; Earth sits at S9 0.10, S10 1.77.
2. Biasing A's fBm with single elongated ellipses trades shape for size control: a weak bias keeps A's unstable sizes (S2 22-69 %); a strong or flat-topped bias makes blobs (S9 0.30-0.75) and smooths coasts (S3 1.08-1.19).
3. Lobed cratons (chains of 2-3 ellipses along a bending spine) give continent-scale non-convexity, so a strong bias no longer makes blobs.
4. Damping the fBm in the deep ocean and craton interiors stops it creating extra masses or splitting cratons; coast detail stays on the margins.
5. Sizes still depend on each seed's layout; the deterministic layout retry on a 2° grid makes S1 and S2 pass on every seed in every retry row.
6. H1 is closest to Earth on S3 (1.20-1.27) and S10 (1.6-2.4); H2 has more S3 margin (1.23-1.29) and more masses (up to 7).

## Visuals (round 2)
- `images/land-sea-A-H1-H2.png`: seeds 1-5 (rows) for A-f1.0-g.60, H1 and H2 (columns).
- `images/R3-k1.0-g.72/land-sea-seed-N.png` and `images/R3-k0.8-a2.5-g.72/land-sea-seed-N.png`: H1 and H2 at 1440 × 720.
- Elevation maps (local, not committed): `target/s92/R3-k1.0-g.72/seed-N/elevation.png` and `target/s92/R3-k0.8-a2.5-g.72/seed-N/elevation.png`.

## Limits (round 2)
- By eye, H has fewer small islands, archipelagos and inland seas than A: the margin weight damps the fBm in the deep ocean. No metric scores islands below 1 Mkm²; an island-count metric (masses of 10^4-10^6 km² against Earth) would catch it.
- Lobe centres are random; several seeds put a large mass over a pole (exaggerated by the equirectangular map). Placement is still not plate-driven.
- Thin streaks along the polar rows come from the warp at the poles (present in A as well).
- Margins are thin: H1's S1 is 4 on seed 1 (band edge) and its S3 1.20 on seed 2; H2's S1 is 7 on seeds 2 and 3.
- The S9 gate (≤ 0.20) and the S10 band (1.5-2.5) rest on Earth's single sample; 5 seeds only.

# Round 3: islands, polar land, 3D noise (inbox #66)
Planner's pick after round 2: H2 (R3-k0.8-a2.5-g.72). Round 3 varies H2 only; spike code 79c9d663.

## New checks (`proto/s92_islands.py`, same 0.25° grid)

| Check | Definition | Earth (ETOPO 2022) | Gate |
|---|---|---|---|
| S11 islands | count of land masses of 10^4 to 10^6 km², and their share of all land | 50 islands, 3.9 % of land | 35-70 islands and 2-6 % of land |
| Polar masses | masses of at least 1 Mkm² whose area-weighted mean direction lies poleward of 70° | 2 (Greenland 73.1° N, Antarctica 85.8° S) | ≤ 1 (planner) |

- Earth itself fails the ≤ 1 polar rule (Greenland's centroid is at 73° N); the rule is the planner's, kept as given.
- H2 of round 2 already has ≤ 1 polar mass on every seed (0 or 1), so no latitude weighting of the craton placement was needed.
- H2 of round 2 has 21-40 islands holding 0.5-1.9 % of land: too few and too small.

## Islands term
- `S92_DEEP` (the fBm floor in the deep ocean and craton interiors) is raised from 0.3.
- `S92_ISL`, `S92_ISLF` add island-scale fBm (4 octaves, gain 0.6, on the warped 3D direction) just offshore of the craton margins: weight exp(-((w - 0.2)/0.15)²), w = min(bias, 1).
- Raising the floor alone helps on average but stays seed-dependent (seed 2: 19-37 islands at floors 0.45-0.75; higher floors overshoot to 82).
- Island noise at base frequency 6 makes many small islands (share 1-2 %); base frequency 3 makes islands of Earth's size (Earth's 50 islands average 0.08 Mkm²: Borneo, Madagascar, Baffin, Sumatra).

## Results (seeds 1-5; all on H2's knobs plus the column's)

| Config | S1 | S2 % | S3 | S9 | S10 | S11 | S11 % | polar | all |
|---|---|---|---|---|---|---|---|---|---|
| H2 (round 2, deep 0.3) | 5 7 7 5 4 | 58 52 50 52 56 | 1.27 1.23 1.26 1.28 1.29 | 0.12 0.15 0.18 0.11 0.09 | 2.0 2.4 2.1 1.8 2.2 | 26 21 24 40 36 | 0.9 1.1 0.5 0.8 1.9 | 1 1 1 0 1 | 0/5 |
| d.45 | 6 7 7 5 4 | 56 52 49 51 54 | 1.29 1.23 1.26 1.28 1.31 | 0.11 0.13 0.16 0.11 0.09 | 1.8 2.4 2.2 1.8 2.1 | 61 19 33 64 56 | 3.2 1.0 0.8 1.8 3.3 | 1 1 1 0 1 | 2/5 |
| d.6 | 6 7 7 7 4 | 53 52 48 49 53 | 1.32 1.24 1.27 1.29 1.30 | 0.10 0.12 0.15 0.10 0.08 | 1.7 2.3 2.4 1.7 2.0 | 77 23 52 66 67 | 3.9 1.2 1.7 2.2 3.3 | 0 1 1 0 1 | 2/5 |
| d.75 | 6 7 7 5 6 | 50 52 47 54 47 | 1.32 1.26 1.29 1.30 1.30 | 0.05 0.11 0.15 0.06 0.10 | 1.9 2.7 2.3 1.9 2.2 | 68 37 72 82 79 | 3.2 1.8 2.9 4.9 4.2 | 1 1 1 0 1 | 1/5 |
| d.45-i.35 (f6) | 6 7 7 5 4 | 56 52 49 52 55 | 1.28 1.23 1.28 1.28 1.30 | 0.12 0.15 0.13 0.09 0.08 | 2.0 2.3 2.2 1.9 2.1 | 82 40 45 36 44 | 3.1 1.9 1.2 1.3 2.8 | 1 1 1 0 1 | 1/5 |
| d.45-i.6-f3 | 5 7 7 6 4 | 55 53 48 50 56 | 1.26 1.27 1.28 1.27 1.30 | 0.11 0.10 0.14 0.11 0.09 | 1.8 2.3 2.2 2.1 2.4 | 57 31 41 36 51 | 2.9 1.9 1.9 2.6 3.5 | 1 1 1 0 1 | 3/5 |
| d.45-i.9-f2.5 | 5 7 7 7 6 | 55 55 48 51 57 | 1.25 1.26 1.27 1.26 1.30 | 0.09 0.08 0.14 0.11 0.07 | 1.8 2.3 2.2 1.6 2.3 | 38 23 30 37 37 | 1.4 1.3 0.9 1.2 1.6 | 1 1 1 0 1 | 0/5 |
| d.55-i.6-f3 | 7 7 7 6 5 | 54 52 47 50 54 | 1.29 1.27 1.28 1.29 1.30 | 0.09 0.10 0.14 0.08 0.09 | 1.8 2.3 2.2 2.1 2.2 | 64 41 56 52 66 | 2.7 2.2 2.0 2.8 3.6 | 1 1 1 0 1 | 4/5 |
| d.6-i.6-f3 | 6 7 7 6 6 | 53 52 47 50 54 | 1.30 1.27 1.28 1.28 1.30 | 0.09 0.09 0.14 0.08 0.09 | 1.8 2.2 2.3 2.1 2.3 | 78 41 65 56 57 | 3.3 2.4 2.1 3.4 3.2 | 1 1 1 0 1 | 4/5 |
| **H2r3: d.65-i.6-f3** | 6 7 7 7 7 | 53 52 47 49 53 | 1.30 1.27 1.28 1.29 1.30 | 0.08 0.08 0.13 0.07 0.09 | 1.8 2.2 2.3 2.2 2.2 | 68 40 65 54 56 | 3.4 2.2 2.3 2.8 2.8 | 1 1 1 0 1 | **5/5** |

- Land is 29 % on every seed. Every row of `raw/scores-round3.json` (17 configs) reproduces on 79c9d663 (`S92_ISL` defaults to 0, no island term).
- H2r3's environment: `S92_CANDIDATE=H S92_SIZES=0.50,0.26,0.10,0.06,0.03,0.02,0.01,0.01 S92_LOBES=3 S92_TURN=1.2 S92_SPACING=0.6 S92_K=0.8 S92_AMAX=2.5 S92_GAIN=0.72 S92_RETRY=12 S92_DEEP=0.65 S92_ISL=0.6 S92_ISLF=3`.

## 3D noise
- Every continent term is 3D value noise on the direction: `global::fbm` calls `height::layer_noise` (crates/generators/src/height.rs:51), trilinear on a Cartesian lattice at `p · freq`; the warp, the shape fBm, the craton weights and the island term all take the 3D direction. No term reads latitude or longitude.
- `images/polar-H2r3.png` shows seeds 1, 4 and 5 in azimuthal-equidistant views to 40° from each pole beside the equirectangular map: coasts keep their character at the poles; the edge streaks of the equirectangular map are its projection.
- Some island fields show parallel striations (seed 4's south view, seed 1's west coast): the domain warp (0.6) stretches the high-frequency noise along its gradient. It is direction-based and appears at all latitudes; a smaller warp on the island term would reduce it.

## Findings (round 3)
1. H2r3 passes S1-S3, S9, S10, S11 and the polar rule on all 5 seeds.
2. Islands need both a higher fBm floor (0.65) and an island-scale term at Earth's island size (base frequency 3); either alone is seed-dependent.
3. The islands cost S1 margin: S1 is 7 (band edge) on four seeds, as large islands near 1 Mkm² count as masses.

## Visuals (round 3)
- `images/land-sea-A-H2-H2r3.png`: seeds 1-5 (rows) for A-f1.0-g.60, H2 (round 2) and H2r3 (columns).
- `images/polar-H2r3.png`: polar views of H2r3, seeds 1, 4, 5.
- `images/H2-d.65-i.6-f3/land-sea-seed-N.png`: H2r3 at 1440 × 720.

## Limits (round 3)
- Margins are thin: S1 7 on seeds 2-5, S11 68 on seed 1 (band 35-70), S11 share 2.2 % on seed 2.
- The S11 band and the polar rule rest on Earth's single sample (Earth fails the polar rule).
- Warp striations in the island fields (above).
