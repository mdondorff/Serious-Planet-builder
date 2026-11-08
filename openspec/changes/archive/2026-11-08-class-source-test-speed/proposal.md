# class-source-test-speed

## Why
- Since #1225 every class source that reads a sea point builds the coast lattice (GEN-136), about 4.6 s on 16 threads and about 5 s on 2.
- `class_source_store` (2 of 5 tests) and the viewer/class tests built the same world's lattice up to three times per process and hit the 180 s timeout in a full local `test fast` (12-41 s alone).

## What changes
- `ClassSource` keeps its coast lattice in a process-wide memo (cap 4, about 8 MB each) under the key seed, radius bits and the full `ShelfProfile`; the lattice is a pure function of that key (plates follow from the seed, the threads do not change the bytes).
- No lattice code, field value, version or golden changes; no test is weakened.

## Evidence
- Tier A: GEN-136, `class_sources_of_one_world_share_the_lattice_and_another_shelf_does_not` (two thread counts read the same bits; another shelf is another lattice).
- Full local `cargo xtask test fast`: 1965 passed, no timeout.
- `class_source_store` alone: 5 tests, 11-26 s before; builds per test process 1, 1, 2, 2, 3 reduced to one per distinct seed.
