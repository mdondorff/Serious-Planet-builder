## Why

M3 queue item 5, second part, ADR 0019 (accepted by the owner, 2026-10-11): the disk tier between the RAM LRU and the generators. It must serve a tile quickly, never block a frame, survive a crash without corrupting later reads, and let the editor and the headless `generate` mode share one store.

## What Changes

- `planet_cache::hilbert`: the standard 2D Hilbert curve (`xy_to_d`, `d_to_xy`) and the 11-byte `sort_key(kind, tile)`.
- `planet_cache::crc32c`: CRC-32C in-tree (table, no unsafe, no dependency), checked against the published values.
- `planet_cache::pack`: `Record` and its wire format, `PackWriter` (advisory `File::try_lock`, torn-tail truncation, atomic index writes, rollover at a size limit), `TileStore` (reads every pack of a directory, rebuilds a missing or stale index in memory, `get` by kind, tile and key, newest wins), `PackError` with actionable messages (corruption names file, offset and key; a locked pack names the holder).
- Modified: STRM-008 added (tier A). Tests: six in `hilbert_crc.rs` (the curve matches the standard 4x4 listing, bijection, consecutive cells adjacent, children contiguous, bounded neighbour distance; CRC published values) and fourteen in `pack.rs`.
- Proposed ADR 0024 records what the implementation had to settle in ADR 0019: the record carries its tile id, the header and index layouts (40-byte entries), staleness and corruption rules, duplicates, the size limit, and that the Windows lock is mandatory so the holder note is a separate file.
- Not here: compression (byte shuffle plus LZ4), compaction and the size cap, `cargo xtask cache-verify`, wiring the store into the streamer and the `generate` CLI.
