## Why

M3 task 3, third slice and ADR 0026 follow-up (l): the bake had a field, a tiled bake and a stage key but no stored form. Persisting it needs the block record (kind 3), its payload layout, its content key and node set id, and a command that builds, verifies and times it (the 1 km clause of GEN-014 needs a measurable command).

## What Changes

- `planet_cache::global_store`: the kind-3 block payload codec (`encode_block`, `decode_block`, `block_cells`: four shuffled `f32` planes in an LZ4 block, hostile-header safe), `GlobalWorld` (the seed through the recording context, the radius read directly so it is in the body parameter hash but not the read set, the bake's stage key, block keys, the node set id, block records), `build_global` (reuses stored blocks without computing them, holds at most one batch) and `verify_global` (STRM-004 rules for blocks; reuses `VerifyReport`).
- Keys per ADR 0026 point 6: `Kind::GlobalBlock = 3`; block content key = the ADR 0018 encoding with the block's tile, the bake's stage key as source, `B` under tag 1, `res` under the new `PARAM_GRID_RES` tag 4, an empty read set. No new stage format tag.
- `generators::global::bake_blocks_where`: `bake_blocks` for the blocks a predicate accepts (a rebuild computes only missing blocks).
- CLI: `generate --global-dir DIR [--global-level L --global-cells B --threads N]` (prints blocks written and reused, peak blocks in memory, seconds and microseconds per sample per core), `--verify-global DIR [--sample N] [--allow-stale]`, `--global-products DIR [--global-res N]` (cube nets and maps of an in-memory bake); options they ignore and combinations are refused.
- STRM-012 added (tier A). Tests: `cache/tests/global_store.rs` (8: round trips, hand-built layout, hostile payloads, keys and known answers, node set id by hand, build/reuse/verify, bad blocks, bodies and radius) and three CLI tests.
- Interpretation to confirm (ADR 0026 consequences say "over the body's seed node path only, `world.seed` for body 0, `derived.body.N.seed` otherwise"): the node set id covers the paths the field actually reads and records, which are both `world.seed` and `derived.body.0.seed` (the derived seed is a recorded read), and not the radius. Easy to change before anything is stored for real; a pinned known answer would move.
- Measured ad hoc on this machine (20 threads, release build, other work running; NOT a perf-window result): 100 M samples (4096 cells per face edge) in 21.5 s, about 4.7 M samples per second. At that rate 16384 cells per face edge (1.6 G samples) takes about 345 s and 8192 about 87 s. The architect's perf window decides GEN-014.
- Not here: the ADR 0026 point 5 reader ADR (before the first regional field reads the bake), compaction of kind-3 blocks, per-tile node set ids.
