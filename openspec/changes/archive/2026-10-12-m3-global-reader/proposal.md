## Why

ADR 0026 point 5 was left open until the first regional field reads the bake, and the store review (m3-global-store) found a layout gap: the bake's stage key covers `res`, block keys cover `B`, so one bake could be stored twice in different layouts. ADR 0027 (proposed) decides both; this change implements it before task 4's first regional field.

## What Changes

- Canonical layout (ADR 0027 point 1): `GlobalWorld::layout(res)` gives `(level, cells)` with `B = min(res, limit)` (limit 1024, a store policy settable only through `with_block_limit` for tests); `build_global(dir, world, res, threads, max_pack)` takes `res`, not `(level, cells)`; `block_record` refuses another layout; `verify_global` reports a block of another layout as unreadable. CLI: `--global-dir DIR --global-res N` replaces `--global-level` and `--global-cells`.
- Reader (ADR 0027 points 2 to 5): `generators::global::read_lattice` and `read_tile_sample` read the bake at integer lattice positions with the edge-symmetric blend `a (1 - w) + b w`, halo hold beyond a face edge, `LatticeSource` (`GridSource` for memory), `ReadError`, `READER_VERSION` 1.
- `planet_cache::global_store::StoreSource`: the stored bake as a lattice source (finds each block through the layout, decodes it once, keeps 16, names a missing or damaged block with its key).
- GEN-015 added; STRM-012 modified (layout, source, CLI). Tests: `generators/tests/global_reader.rs` (6: lattice points at every target resolution, equal values on shared face edges between bake samples, partition invariance, blend and convergence, halo and errors, pinned known answers) and store tests (the stored bake reads like the in-memory one at every vertex and between them, missing block error, canonical layout, refusals).
- Decisions for the architect in ADR 0027 (proposed): canonical layout, integer positions, edge-symmetric blend, halo hold (a limitation across face edges for stencils), land by interpolated continentalness, one upstream key.
- Not here: the first regional field that uses the reader (task 4), a reader that crosses a face edge for halos.
