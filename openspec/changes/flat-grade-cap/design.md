## The cap
- `regrade_one` writes `min(base + epsilon * rank, chessboard level)`; the chessboard level is the ungraded fill, the input of the grade.
- Every cell keeps a strictly lower neighbour:
  - **cap not binding:** the Euclidean parent's written level is at most its rank, and that rank is at least `FLAT_STEP` below the cell's rank;
  - **cap binding:** the chessboard parent's written level is at most its chessboard level, one epsilon below the cell's;
  - **seed of the grade:** a rim cell at or below the base, while every graded rank is at least 1/sqrt 2.
- No cell outside a flat sees a neighbour rise. A routing built on the chessboard surface keeps its lower neighbours.
- A graded cell stays strictly above its ground, because both terms are above `z` (`own` and the lift).
- The tiling rule holds: the cap reads only the flat's own chessboard levels, which are a function of the flat and its rim.

## Rejected
- Dropping `own`: a ramp inside a flat would be graded below its own height (the numerics review of GEN-173).
- Ordering the Dijkstra by `max(d / sqrt 2, own)`: it changes the grade of every flat with a ramp. The cap changes only cells that stood above their count.
