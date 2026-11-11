## Why
Architect, 2026-11-10: item 3's stored-field read stays open after G-STORE: read H0 from stored blocks on both sides of a cube edge (at least one edge and one corner), compare it with the in-memory H0 at the same cells, allow at most the store's quantisation step. Tier A or B on a fixture; not a gate; no preview capability.

## What Changes
- `tests/h0_stored_read` (cache) only: a 16-cell bake in blocks of 4 stored and read through `StoreSource` against `GridSource`; H0's cell values (the C1 height and the bilinear rainfall and temperature at the cell centre, the reads H0's base uses) of 12 cells a face edge: 864 cells, 6 x 44 edge cells, 24 corner cells. Every value equal bit for bit (the stored codec is lossless: step 0); every edge cell's stencil reaches over its face edge; a read from an empty store is an error.
- GEN-164 added (generator-pipeline), worded as the architect asked: no new preview capability is promised. An earlier draft added `PreviewBase::from_field` and a routing comparison; both are dropped (routing equality is G-PART and G-DET's, a mismatch shows as an H0 difference first).

## Impact
- Code: a test and its aggregator entry only. No production code changes.
- The read is an interpolant of the lattice, so it differs from the generator's own sample at the cell centres; the store adds nothing to that difference (the two reads in the test are equal bit for bit). Not measured here: that difference.
