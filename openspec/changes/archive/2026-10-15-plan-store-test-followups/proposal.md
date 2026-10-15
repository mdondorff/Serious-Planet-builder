## Why

Two test gaps the architect found after M4 items 7a (#120, LOD-009) and 7b (#121, STRM-014):

1. Removing the mesh-key memo from `plan_terrain_cached` is not caught: the reference and eroded keyers have no per-tile observable on the field.
2. The 7b rate requirement (20-thread tile rate over the store at least 80 % of in-memory) is only measured ad hoc.

## What Changes

- frame: `plan_terrain_cached` delegates to a `pub(crate)` `plan_terrain_keyed` that can take a replacement per-tile mesh keyer (tests only; production passes none and uses `MeshKeyer`). A tier A test counts key derivations: a warm plan derives zero, a moved camera derives only the new tiles, a changed cell count derives all again.
- cache: `SourceStats` gains `shared_lookups`, the number of shared-cache lock acquisitions on the read path (one relaxed increment next to the lock that is taken anyway). A tier A test is the deterministic proxy for the rate: at 20 threads, lock acquisitions per tile stay a small fraction of lattice reads, and decodes equal the distinct blocks. The measured-rate comparison stays ad hoc: `store_blocks --memory-rate N` prints the ratio and PASS/FAIL at 80 %, with the recorded figures in the test's comment (a synthetic ignored test on the 64-cell grid does not measure the product's shape: tiles change block too often there).
- Specs: scenarios added to LOD-009 and STRM-014. No behaviour, golden or pinned count changes.
