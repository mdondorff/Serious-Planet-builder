## Why
Two tests approved at the M3 gate were never written (Tracked item "M3 gate condition 2", trigger "before the first regional field is stored"): the declared-versus-recorded reads contract test (ADR 0021 point 7) and a tile-level GEN-017 test.

## What changes
- `reads_contract.rs` (generators, tier A): over a stack of stamp, line and brush stages and tiles from level 5 to 17, near the items and far, a recording reader shows that every item a stage reads is in the footprint its tile declares (`footprint_items` with `STAGE_HALO`), that removing the undeclared items changes no height and no fine key, and that removing a read item re-keys the tile. The tiles are sampled with the production lattice and ring (`lattice_directions` at 4, 8 and 32 cells) and a sweep of small stamps puts reach ends on both sides of the declared cap. The test was checked to fail, all three of its tests, when `STAGE_HALO` is cut from 0.25 to 0.05.
- `erosion.rs` (tier A): a direction has one height whichever tile asks for it (the tile, a neighbour, parent, child, a far tile; the production batches at 8 and 16 cells bit for bit in f64, and the f32 mesh vertices), and the points two neighbours share, matched by the bits of their direction (COORD-007), also across face edges and the cube corner, get the same height from either tile's batch.
- Both tests passed on the first run: no violation was found.
- The Tracked line is deleted from `docs/status.md`.

## Impact
- Spec: GEN-017 and GEN-026 gain one scenario each (modified).
- Code: tests only (`crates/generators/tests`).
