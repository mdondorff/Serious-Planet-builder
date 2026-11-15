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
