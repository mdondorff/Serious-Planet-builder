# SPIKE-121: E3 variants B and C (explorer)

Opened: 2026-11-15
Question: Does one round of the regional kernel at L drain and incise correctly once its K dt is scaled to the L cell (B), and with controlled hillslope diffusion added (C)?
Why it matters: P3 E3, the controlled comparison for D-1 (how drainage becomes valid at L). The variants are A (crusader), B and C (explorer), D to F (spikes). The exit needs one variant that passes G-DRAIN and G-INCISION separately, with acceptable cross-sections, on all five windows.
Time box: 1 day of agent work after the E1-rest PR (#1527).
Success criteria:
- B and C each change one thing against A: the same base, windows (R1h, R3a, R3b, m1, f1), reader build and binary (`bake_eroded`, this branch, cut from crusader's abb3e57d).
- Calibration on R1h only.
- One store per variant and window, baked under the heavy lock.
- Every number from `cargo xtask river-gate --table`.
- Cross-sections on spikes' 15 transects.
Fallback if it fails: report B and C's numbers against A as they are. If no variant passes, the architecture is revisited (P3 E3), not the least bad variant taken.
Code location: spikes/e3-variants-bc/ (results) and crates/cache/examples/bake_eroded.rs on spike/e3-variants-bc (throwaway; never merged)

## The binary
`bake_eroded --out <dir> [--window W] [--k-scale S] [--diffusion KAPPA] [--start <file.f64>] [--rounds 0|1]`
- The defaults are A on R1h, the same code path as crusader's SPIKE-120 bake. Check: the commit id equals crusader's 39038c59df52b635.
- `--start` and `--rounds 0` are spikes' hooks for D to F (agreed 2026-11-15).
- Any non-default option mixes into the inputs identity. The gate then reads those stores with `--no-key-check`.

## The kernel (crates/generators/src/bake/carve.rs:311-317)
- One step, a cell c to its receiver r, downstream first: `z_c <- (z_c + u + F z_r) / (1 + F)`, `F = k_dt * sqrt(A_c) / L_cr`, uplift `u = u_over_k * k_dt`.
- This is the implicit (Braun-Willett) step of `dz/dt = U - K A^m S^n`, m = 0.5, n = 1, with `k_dt = K dt`.
- The regional solve: k_dt 0.05, 5 steps a round, 4 rounds, cells of about 814 m (`region_bake.rs:216`).
- A: the same k_dt 0.05, 5 steps, 1 round, cells of 101.8 m (`bake_eroded`, spike/verification abb3e57d).

## B: K dt scaled to the cell size
- n = 1 makes the stream-power equation a kinematic wave: a knickpoint moves upstream at `c = K A^m` (m/time).
- F is that wave's Courant number for one step: `F = c dt / L`.
- The kernel's k_dt 0.05 was set on 814 m cells. For the same river (same A), A's F at L is 814 / 101.8 = 8 times the regional F.
  - Example: A = 100 km2 gives sqrt(A) = 1e4 m, so F = 0.61 regional and 4.9 at L.
- The implicit step is stable for any F, but its error grows with F. Above F ~ 1 a step takes most of `z_c - z_r` at once, and the knickpoint jumps many cells along a profile in one sweep. The incision then concentrates on the channel cells: A's R1h store (39038c59) has 0 % burn but 1.26 % valley depth over 50 m (p99 52.8 m), narrowness p50 0.37 and width at 10 m p50 634 m (the start surface: 894 m).
- Keeping the per-step Courant number of the scale the kernel was set at: `k_dt_L = k_dt_R * L_L / L_R = 0.05 * 101.8 / 814 = 0.00625`, a scale s = 1/8. This is the CFL time step `dt ∝ dx` of the advective wave.
- The uplift per step `u_over_k * k_dt` scales with it (the same dt). Nothing else changes.
- Consequence: 5 steps at s = 1/8 are 1/8 of A's model time. Matching A's time takes 40 steps; that changes a second thing and is reported as a note, not as variant B.
- Calibration on R1h only: s in {1/16, 1/8, 1/4, 1/2}, plus A (s = 1). Pick s on G-DRAIN (basins) and G-INCISION (valley depth and burn shares) separately, then run all five windows at that s.

## C: B plus hillslope diffusion
- `dz/dt = U - K A^m S + D lap(z)`, operator-split: after the round's 5 carve steps, 5 explicit sweeps of `z <- z + kappa * lap5(z)` (one per carve step), `kappa = D dt / L^2` (dimensionless), stable for kappa <= 1/4. The sea and the outlets keep their heights. The split keeps the carve's routing schedule as in B: rerouting between sweeps would change a second thing.
- One parameter: kappa.
- Derived value: with dt from B's step, `dt = k_dt_L / K`. With K ~ 2e-6 1/yr for m = 0.5 (Stock and Montgomery 1999 range) and D ~ 0.01 m2/yr (soil-mantled hillslopes), dt ~ 3,100 yr and kappa ~ 3e-3.
  - At that value diffusion barely touches 1 to 2 cell features in 5 steps: the 2-cell mode decays by (1 - 8 kappa)^5 ~ 0.89.
- What C must remove: the V that is 1 to 2 cells narrow (A's R1h store: narrowness p50 0.37, valley depth 1.26 % over 50 m).
- Calibration on R1h only: kappa in {3e-3 (derived), 1e-2, 3e-2, 0.1}. Pick on G-INCISION and the valley width at 10 m without lowering G-DRAIN, then all five windows.

## Bars (lead #155, set before the results)
| Gate | Bar |
|---|---|
| G-DRAIN | basins at most 10 % of river length |
| G-INCISION | valley depth share over 50 m: R1h 1.80 %, R3a 0.10 %, R3b 0.10 %, M1 40.80 %, F1 0.10 % |
| Cross-sections | judged on the T1 to T3 figures |

## B: calibration on R1h
Five stores of R1h from one binary (a07b404c): A (K dt x 1) and B at 1/2, 1/4, 1/8 (derived) and 1/16. All are read by `cargo xtask river-gate --table ... --no-key-check` on main 8c503df0. Full table: `table-b-calibration-r1h.csv` and `.md`.

| K dt scale | 1 (A) | 1/2 | 1/4 | 1/8 (derived) | 1/16 |
|---|---|---|---|---|---|
| basins % (G-DRAIN) | 0.58 | 0.59 | 0.47 | 0.37 | 0.32 |
| valley depth over 50 m, % (cells) (G-INCISION, bar 1.80) | 1.26 (2,225) | 1.33 (2,355) | 1.57 (2,791) | **2.03** (3,593) | **2.54** (4,480) |
| valley depth p99, m | 52.8 | 54.0 | 56.3 | 60.2 | 63.5 |
| burn over 50 m, % | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| ring 2-4 over 50 m, % | 4.82 | 3.89 | 3.76 | 4.22 | 4.77 |
| relief 10 x 10, m (start 54.9 in window mode) | 76.9 | 71.9 | 69.6 | 68.5 | 68.0 |
| width at 10 m p50, m | 634 | 668 | 700 | 710 | 705 |
| narrowness p50 | 0.365 | 0.391 | 0.412 | 0.433 | 0.463 |
| asymmetry p50 | 0.489 | 0.477 | 0.471 | 0.468 | 0.471 |
| slope-area theta | 0.347 | 0.340 | 0.340 | 0.348 | 0.357 |
| spectrum beta | -3.62 | -3.68 | -3.71 | -3.72 | -3.73 |
| trunk reaches over 50 m (of 5,297) | 539 | 444 | 434 | 446 | 459 |
| trunk vertices over 50 m | 3,815 | 3,340 | 3,420 | 3,716 | 3,988 |
| deepest trunk vertex, m | 357 | 378 | 361 | 347 | 340 |
| edge matched, % | 77.3 | 77.3 | 86.4 | 81.8 | 77.3 |

- G-DRAIN passes at every scale.
- G-INCISION gets worse as K dt shrinks. A weaker round at L leaves more relief for K7 to cut, and K7 cuts narrow: narrowness rises from 0.365 to 0.463, and the valley depth p99 from 52.8 to 63.5 m. The derived 1/8 fails R1h's bar (2.03 % against 1.80 %).
- The calibration picks the scale nearest the derived 1/8 that passes on R1h: **1/4** (1.57 %). It also has the lowest ring 2-4 share and the best edge agreement. B is K dt x 1/4 on every window.
- Finding: the incision the gate counts comes from K7's channel cut on top of the round, not from the round. The round's K dt sets how much of the valley K7 has to make.
- The trunks: about 8 % of H1's trunk reaches stand more than 50 m above their running minimum in every variant, A included, up to 378 m deep. No K dt scale carries the trunks without a burn on R1h.

## C: calibration on R1h
B (K dt x 1/4) and C at kappa 0.003 (derived), 0.01, 0.03 and 0.1 on R1h, from the same binary. Full table: `table-c-calibration-r1h.csv` and `.md`.

| kappa | 0 (B) | 0.003 (derived) | 0.01 | 0.03 | 0.1 |
|---|---|---|---|---|---|
| basins % (G-DRAIN, bar 10) | 0.47 | 1.03 | 2.23 | 6.23 | **13.75** |
| valley depth over 50 m, % (cells) (G-INCISION, bar 1.80) | 1.57 (2,791) | 1.42 (2,526) | 1.17 (2,090) | 0.96 (1,719) | 0.79 (1,407) |
| valley depth p99, m | 56.3 | 55.5 | 52.5 | 48.9 | 42.8 |
| ring 2-4 over 50 m, % | 3.76 | 3.54 | 3.11 | 2.65 | 2.15 |
| uphill river steps over 0.25 m, % | 0.10 | 0.14 | 0.29 | 0.75 | 1.53 |
| width at 10 m p50, m | 700 | 713 | 731 | 759 | 798 |
| narrowness p50 | 0.412 | 0.406 | 0.396 | 0.380 | 0.381 |
| relief 2 x 2 / 10 x 10, m | 8.06 / 69.6 | 8.03 / 69.4 | 7.98 / 69.1 | 7.86 / 68.5 | 7.61 / 66.9 |
| slope-area theta | 0.340 | 0.336 | 0.342 | 0.352 | 0.364 |
| trunk reaches over 50 m (of 5,297) | 434 | 430 | 424 | 403 | 349 |
| edge matched, % | 86.4 | 86.4 | 88.6 | 86.4 | 86.4 |

- Diffusion widens the valleys K7 cuts and lowers every incision number, monotonically.
- It also fills channel cells after the carve has routed, so closed basins and uphill steps grow just as fast. kappa 0.1 fails G-DRAIN.
- The derived kappa (3e-3) barely acts, as the derivation said: the 2-cell mode decays by about 0.89 over the 5 sweeps.
- The calibration picks the largest kappa that keeps G-DRAIN: **0.03** (basins 6.23 %, valley depth 0.96 %). C is K dt x 1/4 plus kappa 0.03 on every window.
- The basins come from the operator split. Re-routing between sweeps would drain them, but that changes a second thing against B; it is noted as a follow-up, not run.

## A, B and C on the five windows
15 stores from one binary (a07b404c): A, B (K dt x 1/4) and C (B + kappa 0.03), each on R1h, R3a, R3b, m1 and f1. One `river-gate --table` run read all of them (main 8c503df0 and the spike's `--xsec`, `--no-key-check`). Full table: `table-abc-five-windows.csv` and `.md`.

| Window | Variant | Basins % (G-DRAIN, bar 10) | Valley depth over 50 m, % (cells) | G-INCISION bar | Pass | Ring 2-4 % | Width 10 m p50, m | Narrowness p50 | Uphill % | Trunk reaches over 50 m (of) | Deepest trunk vertex, m | Edge % |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| R1h | A | 0.58 | 1.26 (2,225) | 1.80 | yes | 4.82 | 634 | 0.365 | 0.09 | 539 (5,297) | 357 | 77.3 |
| R1h | B | 0.47 | 1.57 (2,791) | 1.80 | yes | 3.76 | 700 | 0.412 | 0.10 | 434 (5,297) | 361 | 86.4 |
| R1h | C | 6.23 | 0.96 (1,719) | 1.80 | yes | 2.65 | 759 | 0.380 | 0.75 | 403 (5,297) | 367 | 86.4 |
| R3a | A | 0.05 | 0.08 (49) | 0.10 | yes | 0.42 | 1,571 | 0.385 | 0.02 | 17 (12,750) | 96 | 25.0 |
| R3a | B | 0.00 | 0.02 (12) | 0.10 | yes | 0.10 | 2,047 | 0.442 | 0.00 | 1 (12,750) | 52 | 26.2 |
| R3a | C | 1.13 | 0.00 (3) | 0.10 | yes | 0.06 | 2,213 | 0.422 | 0.23 | 0 (12,750) | 0 | 27.4 |
| R3b | A | 0.25 | 0.01 (25) | 0.10 | yes | 0.12 | 1,152 | 0.379 | 0.04 | 45 (8,797) | 129 | 68.8 |
| R3b | B | 0.03 | 0.01 (28) | 0.10 | yes | 0.04 | 1,377 | 0.417 | 0.01 | 31 (8,797) | 141 | 69.9 |
| R3b | C | 3.30 | 0.02 (39) | 0.10 | yes | 0.03 | 1,428 | 0.393 | 0.44 | 30 (8,797) | 129 | 72.0 |
| m1 | A | 3.50 | 16.49 (40,781) | 40.80 | yes | 33.25 | 271 | 0.442 | 0.52 | 3,218 (10,371) | 892 | 81.7 |
| m1 | B | 3.32 | 22.23 (55,429) | 40.80 | yes | 38.38 | 188 | 0.527 | 0.65 | 3,420 (10,371) | 923 | 79.3 |
| m1 | C | **11.59** | 16.98 (42,619) | 40.80 | **no (G-DRAIN)** | 31.92 | 257 | 0.483 | 1.23 | 3,332 (10,371) | 921 | 82.9 |
| f1 | A | 0.18 | 0.00 (1) | 0.10 | yes | 0.00 | 1,414 | 0.375 | 0.04 | 6 (10,587) | 112 | 34.5 |
| f1 | B | 0.00 | 0.00 (0) | 0.10 | yes | 0.00 | 1,527 | 0.396 | 0.00 | 6 (10,587) | 57 | 34.5 |
| f1 | C | 2.90 | 0.00 (0) | 0.10 | yes | 0.00 | 1,620 | 0.376 | 0.46 | 5 (10,587) | 56 | 34.5 |

- On the numbers, A and B pass G-DRAIN and G-INCISION separately on all five windows. C fails G-DRAIN on m1: the diffusion's basins (operator split, no re-route) grow with relief.
- B against A:
  - B drains better on every window but m1 (basins 0.00 to 0.47 % against 0.05 to 0.58 %).
  - B has fewer uphill steps and wider valleys at 10 m on the low-relief windows (R3a 2,047 against 1,571 m).
  - B carries the trunks better on R1h, R3a and R3b, and the same on f1.
  - B incises more than A on R1h and m1 (valley depth 1.57 against 1.26 %; 22.23 against 16.49 %), with narrower valleys there (m1 width 188 against 271 m, narrowness 0.527 against 0.442).
- C widens the valleys of R1h, R3a and R3b further and empties R3a's trunk deficit. It pays in basins and uphill steps on every window.
- The trunks: in the mountain window, about 31 to 33 % of H1's trunk reaches stand more than 50 m above their running minimum in every variant, up to 923 m. Elsewhere it is 0 to 10 %. No variant of A, B and C lets KR carry m1's trunks without a burn.
- Edge agreement is the window's property more than the variant's: R3a 25 to 27 %, f1 34.5 % in all three.
- Not yet judged: the cross-sections on the T1 to T3 figures (spikes' tool; the CSVs follow).
