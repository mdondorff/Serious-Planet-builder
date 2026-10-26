## MODIFIED Requirements

### Requirement: GEN-052 A-net chunk time
An A-net chunk of 24 km2 of core SHALL take at most 4 s at 8 m cells with the 3 km halo (1.8 million cells) on the reference machine with the stage's production thread setting (the viewer's chunk workers), measured by the performance harness through the architect on the production geometry (the stored bake) and never in a test (CON-20); the one-thread time and the times at 4 and 10 m are recorded with the run (4 m was about 55 s per chunk on one thread: not met, ADR 0077 allows 4 to 10 m).

Verify: C
Status: planned
Source: SPIKE-33, M8 plan risks, CON-22, planner decision 2026-10-24 (8 m default), planner decision 2026-10-26 (production threads; the one-thread window gave 7.0 s with the base heights 4.46 s)

#### Scenario: Chunk time
- **WHEN** a chunk of 24 km2 at 8 m cells runs on the reference machine in a perf window on the production threads
- **THEN** it finishes within 4 s at 8 m cells and the log holds the one-thread time and the times at 4 and 10 m

## ADDED Requirements

### Requirement: GEN-079 A compact batch takes its footprint from a hint
A batch of directions that lies on one face at least `reach + 2` cells from its edges and within 4,194,304 cells of bounding box, evaluated by a field whose reads stay within `reach` cells of a direction's own cell (the eroded field: `GRADIENT_REACH_CELLS`, 8), SHALL take the cells to gather from the box of `reach` cells before and `reach + 1` after the cell of each direction instead of from a pass that makes the reads of every direction; any other batch SHALL take the exact footprint. The gathered cells are the source's own samples and the evaluation is unchanged, so every value is bit for bit the single `height`, at any thread count and with the tile's climate (GEN-021) joined to the footprint.

Verify: A
Status: active
Source: opus-planner (2026-10-26), GEN-020, perf window 2026-10-26

#### Scenario: Bits and reads
- **WHEN** compact batches (across a block edge, inside a block, near a face edge, at a cube corner) are evaluated on 1 and 8 threads
- **THEN** every value equals the single `height`, no read fails, and a batch of about 12 cells reads fewer than 2,000 cells and decodes the four blocks round its centre once
