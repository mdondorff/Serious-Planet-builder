## Why

Perf windows 1 and 2 showed that the gated dive descent cannot meet its lag budget (mean at most 1 level, worst at most 3) from an empty tile cache: a level-0 tile alone reads about 320 bake blocks (6.6 s on one thread), levels 1-4 take 2.1, 0.74, 0.35 and 0.21 s a tile. The owner decided on 2026-10-16 (option A, ADR 0045 point 3) that the gated dive starts with coarse levels 0-4 pre-filled on disk by the bake step, and that the cold dive stays as a tracked, report-only case.

Today the streamer reads no tile from the tile pack at all (it generates every tile on demand), so "pre-filled in the tile pack" needs a stored tile form, a writer, a reader in the tile source, and a way for the perf logs to say which of the two starts they measured.

## What Changes

- cache: `meshpack` (STRM-021): the payload codec of a render tile (vertex or compact form, every bit), the key (the existing ADR 0018 mesh key plus the form), `prefill` (levels 0..=N, any thread count, fixed append order, idempotent, skips present tiles, rewrites an undecodable record), `TileSpec::lookup` (a miss for anything not current).
- app: `generate --global-dir` pre-fills after the bake (`--prefill-levels N`, default 4, 0 disables); `generate --prefill-dir DIR --global-res N` pre-fills an existing bake in place; both print tiles, payload bytes, pack growth and wall time. `MeshSource` takes a tile of level 8 or coarser from the pack when it holds it, and generates it otherwise; `WorldTerrain::cold()` never reads the pack.
- perf scenes: `dive` and `dive-fast` refuse to start (before the readiness gate, naming the count and the command) unless levels 0-4 are present in the pack; every scene logs `protocol.source` as `<place>; <marker>` (`prefilled coarse pack (levels 0-N present at start)`, `no pre-filled coarse pack (P of T ...)` or `cold: ...`) and `prefill_tiles_used`. New scene `dive-cold` (the dive script over a reader that ignores the pack's coarse tiles; tracked, report-only), in `perf stream all` as a seventh child process. `--prefill-levels N` on a streaming scene changes the levels it requires.
- `accept M4`: the gated `dive` is judged only from a log whose source says pre-filled to level 4 or deeper (a cold log, an incomplete pack, a shallower pre-fill and an unmarked log from before the pre-fill are refused with the commands); `dive-fast` likewise (unusable, not a refusal); `dive-cold` is reported in its own section, never gated, and must say cold; logs of the other four scenes from before the pre-fill still read and are reported as `not stated`.
- spec: STRM-021 (new); TEST-018, TEST-019, TEST-021 modified.

Not in this change: the pre-fill of the 16384 store of the perf window (a one-off run after the merge, logged in docs/log.md), a block-ordered pass for the pre-fill, a readiness report (ADR 0045 point 5), eviction. No golden, digest or `MESH_VERSION` change; no existing key, record format or pack file changes.
