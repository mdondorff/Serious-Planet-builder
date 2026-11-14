## Why
Item 15 PR 5a (architect, 2026-11-10): the runtime reads the new chain's L through the commit its bake wrote, as a `LayerSource`, with no fill depth added back; the drawn commit changes only when every part of the new one is ready (A3 section 7.4, CON-22).

## What Changes
- `cache::l_layer::CommitLayer` (STRM-083): a `LayerSource` over a commit (`open(dir, world, capacity)`, `with_commit`): tiles decoded once in a bounded cache with their pyramids, a tile the commit does not name is no land, a named tile the store lacks or a key other than the spec's is an error, the layer's identity is the commit id.
- `cache::l_layer::PairedLayer`: the drawn commit and `try_swap(next, needed)` (the new commit's needed tiles and its basin tables must be in the store).
- `l_writer::Commit` records the L spec (`CommitSpec`: `res`, grid, seed, sea level, flood, width law, base and inputs ids; commit layout version 2) so that a reader forms the keys of the tiles; `read_l_tile` refuses a tile the commit names and the store lacks.
- Tests T1-T4 and the commit pairing test (TEST-111). The old `StoreLayer` and the fill path stay until PR 5b (waits for the owner's acceptance of ADR 0110).

## Impact
- Code: `cache::l_layer` (new), `cache::l_writer` (commit spec, version 2: no store of version 1 exists), tests `l_layer`, `l_writer`. `app::worldfield::with_erosion` reads a folder that holds a commit record through `CommitLayer` (no fill, `--channels on` refused: the new chain stores no drainage channels yet); a folder without one takes the old path.
- STRM-083 and TEST-111 added.
