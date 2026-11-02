# SPIKE-92: continent candidates for M9 generator round 2

Status: closed 2026-11-02. Prototype on `spike/m9-continents` (7e68ebb8, never merged); patch in `proto/spike92.patch`.

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
