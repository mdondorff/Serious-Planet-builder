## Context
- Pass 1 and pass 2 read the lithology erodibility K(x, z) (GEN-116) and pass 1 the uplift field U(x) (GEN-123); GEN-127 states the carve's inputs.
- The carve is a 20-step transient (`k_dt` 0.05, 20 steps): total uplift is `U` metres, at most 84 m; it does not reach steady state, so no slope law is claimed.

## Decisions
1. `Rock { par, k, uplift }`: K is evaluated for every cell at its height at the start of the step (parallel over 64-row blocks, f32 per cell), the uplift is an f32 array per cell; no fields is bit for bit the old carve.
2. A step does not lower a cell below the lower edge of its band by more than 0.01 m (`Layer`, `LAYER_SKIP_M`): bands (30 to 170 m) are thinner than a trunk's single step, so K read at the start height would skip a caprock.
3. `ROCK_K_SCALE` 1.16: a fixed constant of the default lithology (a softer or harder world erodes more or less), the `c` with mean `g(c K f)` = mean `g(f)` over land, `g(f) = f / (1 + f)`, `f = k_dt sqrt(A) / dx`, area distribution `P(A > a) = a^-0.43`; geometric mean of 12 seeds (0.88 to 1.78); seed 14's real face-2 areas give 1.82 against 1.78 parametric. Derived by `k_calib`.
4. The rock fields are part of the layer and tile keys (`rock` byte, refused on mismatch) and `BakeInputs::id` folds `LITHOLOGY_VERSION`, `UPLIFT_VERSION` and the constant. The lithology's province lattice is built on first use (`BakeInputs::lithology`), before the carve.

## Open finding (preview round, not tuned here)
Face 2 of seed 14, tile 2/12/10, unfilled, spike measure (`rock_stats`), K + U + band edge against today's carve:

| | 3 km relief p50 mountains / high / plain | p50 elevation mountains / high mountains |
|---|---|---|
| today | 534 / 695 / 59 m | 1841 / 2899 m |
| K, uniform U | 477 / 676 / 41 | 2028 / 3134 |
| K + U raw | 479 / 674 / 42 | 2037 / 3152 |
| K + U, mean-normalised (c about 1.46) | 487 / 669 / 43 | 1994 / 3081 |
| K + U, ROCK_K_SCALE 1.16 | 483 / 677 / 42 | 2020 / 3124 |

- U against the uniform value changes relief by under 3 m and p50 by 9 to 18 m (at most 84 m of offset).
- The K scale barely moves the result (effective mean K 0.69, 0.79, 1.0 give p50 2037, 2020, 1994), so the residual (relief -10 % on mountains at 3 km, p50 +180 to +225 m) is not the K mean.
- By elimination it is the band-edge cap (a trunk's lowering deferred at every band edge over 20 steps) and the K spread (the update removes `f / (1 + f)`, concave in K). Not separated: the next run is an arm without the edge.
- Seed 14 is the outlier of the constant (its own c is 1.78).
