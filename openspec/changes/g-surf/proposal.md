## Why
Briefing item 11 (inbox #104; A3 sections 3.1 to 3.3): the local 102 m pass in the new chain, on the base read (the store lattice's C1 read, stride-4 retired), with the channels burned into `L`, and gate G-SURF (the runtime's `B + L` against the bake at the lattice nodes). This is the hand-over that caused the original pits: the pit share and depth at 102 m with the breach and channels in are reported.

## What Changes
- `bake::k7`: `k7_tile` (the pass: `B` at every cell, the regional change by a C1 read, the burn-in, the breach of pass 1 at the local grid, the incision, thermal, deposition, pit spreading), `RegionField` (the C1 read of a window of the regional grid), `Channel` and `burn_channels`, `pit_stats` per stage, `surface_gate` (G-SURF at the lattice nodes and at the cells' centres).
- GEN-148 added (generator-pipeline).
- `examples/k7_region`: a tile of the prototype region at 102 m: the lattice (f32 nodes of `BakeInputs::base`, read with `read_elevation_c1`), the regional solve from the same read, H1's channels burned in, the pit shares and G-SURF.
- A3 section 3.1 gets the measured G-SURF.

## Impact
- Code: `generators::bake::{k7 (new), tile (two helpers visible to the crate), mod}`, `examples/k7_region`. No call site in a pass of the bake; nothing in today's output moves.
- Not in this change: the store's `L` (kind 13) and its i16 format; G-PROFILE; the channel continuity across tiles; a level profile from K6; creases and items in the surface read.
