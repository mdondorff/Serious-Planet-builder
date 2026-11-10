## Why
Decision D11 (architect, 2026-11-10; inbox #123): item 3's stored-field read stays open after G-STORE: read H0 from stored blocks on both sides of a cube edge (at least one edge and one corner), compare it with the in-memory H0 at the same cells, allow at most the store's quantisation step. Tier A or B on a fixture; not a gate.

## What Changes
- `bake::preview::PreviewBase::from_field(n, src, map, word)`: the H0 base from a `LatticeSource` (a stored bake or an in-memory grid): C1 read of the height at each cell centre, bilinear read of rainfall and temperature. It is the means for D11's read, not a new feature path: nothing in the app calls it yet; `from_class_source` is unchanged.
- `tests/h0_stored_read` (cache): a 16-cell bake in blocks of 4 stored and read through `StoreSource` against `GridSource`: H0 of 12 cells a face edge (864 cells, 6 x 44 edge cells, 24 corner cells); every height and rainfall equal bit for bit (the stored codec is lossless: step 0), receivers, areas and outlets of the two routings equal, an empty store is an error.
- GEN-164 added (generator-pipeline).

## Impact
- Code: `generators::bake::preview`, `cache/tests/h0_stored_read`, `cache/tests/it.rs`. No bit of any existing path moves.
- Reported, not gated: the read is an interpolant of the lattice, so against the generator's own sample at the cell centres the 16-cell lattice differs by up to 2,404 m (a coarse fixture); the store adds nothing to it.
- Not in this change: an app path that builds the preview base from the stored bake (it would need the field resident at the preview's resolution).
