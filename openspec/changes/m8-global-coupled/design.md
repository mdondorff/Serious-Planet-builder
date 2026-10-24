# Design: m8-global-coupled, the run's lattice independent of the bake's (GEN-056)

## Why
- The production bake has 16384 cells per face edge; a run at that size is out of reach (about 1.6 G cells), so the run takes 2048 and starts from the bake reduced onto its lattice (planner, 2026-10-24).
- Reading the bake at the cell centres (the earlier input) samples one point of an `r x r` block of cells: it aliases the relief the run is meant to carve.

## Decisions
1. Run lattice `n = min(res, 2048)`; both are powers of two, so `n` divides `res` and the cells nest exactly (both lattices are uniform in the face coordinates).
2. Reduction = box filter of the bake's cell means: a run cell is the mean of the `r x r` bake cells (`r = res / n`) it covers; a bake cell is the mean of its four corner vertices. The vertex weights are 1 at the corners of the run cell, 2 along its edges, 4 inside, over `4 r^2`. `f64` sum in row, then column order, one rounding to `f32`.
3. Threads take bands of output rows aligned to the source's block rows (`block_cells`), results are joined in band order: the thread count changes the speed only.
4. Key: `CoupledSpec` already holds `n`, the steps, the parameters and `bake_res`, and the bake's stage key is the source of every block key. Two changes make it honest: `bake_res` is `src.res()` (the caller no longer states it) and the inputs enter by their declared identity (`declared_inputs_hash`: reduction version, radius, uplift, erodibility) instead of a hash of the sampled heights. The heights are a function of the bake (CON-05, its stage key) and of the reduction, so a stored run is found without reading the bake: at 16384 cells the reduction reads about 1.6 G vertices, the old content hash would have repeated that on every start.
5. A change to the reduction bumps `REDUCTION_VERSION`, which re-keys every stored run. Runs stored before this change are made again once (the old inputs read the cell centres).
6. Per-cell driver maps for uplift and erodibility (ADR 0076) will carry their own identity in the same hash.

## Not measured
- The reduction time at 16384 cells (about 1.6 G vertex reads from the store, 8 threads at most) needs the architect's window; it is a one-time cost per bake and run.
