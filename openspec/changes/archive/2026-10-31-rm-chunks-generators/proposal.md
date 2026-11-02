## Why
B6 PR 5 of `remove-chunk-stage` (owner, 2026-10-29; ADR 0086): after PRs 1 to 4 nothing in the app, the frame, the renderer, the streamer or the cache calls the chunk stage, so the generator side that only it used goes.

## What changes
- Removed in `generators`: `anet.rs` (the A-net incision, routing and hillslope stage), `route.rs`, the chunk field, its stage trait, its pyramid and its stand-in in `chunkfield.rs`; the chunk drivers of `drivers.rs`; `CompactTile.change`, `change_replacing`, `shade_vertex_faded`, `shade_all_faded`; `HeightField::height_at_level`, `level_aware`, `level_drivers`, `level_change`, `heights_threaded_at_level` and every wrapper of them (`chain`, `stamp`, `water`, `Bound`, the boxed field, the cancel guard and timing wrappers of `app`); `field_chain_with` and the chain with a chunk layer (`with_stamps_where`).
- Kept in `chunkfield.rs`: the lattice (`Cell`, `ChunkLattice`), `ChunkKey`, `chunks_read_by`, `PYRAMID_TOP_LEVEL`; in `chunk.rs`: `ChunkId`, the grid level and the apply level (GEN-034, GEN-035). The horizon maps are keyed per chunk on this lattice.
- Kept in `drivers.rs`: the driver block of the tile (neutral codes, the 8-bit coding, `NeutralInsolation`, `tile_drivers_at`, `blend_driver`); the block stays in the slot (GEN-025) and the shader's blend `1.0 + fade_w (d - 1.0)` with `fade_w` 1.0 is not edited, because removing it could change the last bit of a driver value.
- Removed elsewhere: `ReadRecord::Chunk`, `Kind::Chunk`, `QUERY_CHUNK`, `PARAM_CHUNK` (nothing produces them; the numbers are not reused); `frame::look::materials::driver_inputs`; the editor's `pick_level` and the level of the hover pick; `ChunkSource::result` and the `ready` argument of `HorizonBaker::bake` (the real baker never read it).
- Requirements removed: GEN-036, GEN-037, GEN-038, GEN-047 to GEN-051, GEN-053, GEN-054, GEN-057 to GEN-060, GEN-066 to GEN-070, GEN-075 to GEN-078, GEN-083, GEN-084, STRM-047. Reworded: GEN-035 (the lattice and the chunks a tile reads), GEN-039 (the setting acts on no build), GEN-071, GEN-072, GEN-074 (the driver block holds the neutral values), VIEW-059 (no pick level; tier B), WDEF-018 (the hash stays).
- Tests removed with the code: `anet_incision.rs`, `anet_network.rs`, `chunkfield.rs`, `level_height.rs`, the chunk cases of `bounds_monotone.rs`, the GEN-070 to GEN-076 cases of `drivers.rs`, the pick-level case of `editwin.rs`. Moved: the lattice case to `chunk.rs` (GEN-035), the neutral block of a tile to `compact.rs` (GEN-074), the cancel guard case to STRM-005, the receiver case of `coupled.rs` to GEN-040.

## Kept (decision 3: the definition hash must not change)
- The `chunk_stage` setting of the definition stays parseable and hashes as before, with `CHAIN_ORDER_ERA`; `world-def/tests/chunk_stage.rs` is unchanged. It acts on no build.
- The world state of a horizon map still hashes the numbers 10 (A-net version) and 1 (chunk stage version), now constants of `horizonworld.rs`, so no stored map is lost.

## Default path
No tile key, golden, slot byte or definition hash changes: no default node read a chunk, level-aware heights equalled `height` for every field without chunk detail, and the driver block was already the neutral one.
