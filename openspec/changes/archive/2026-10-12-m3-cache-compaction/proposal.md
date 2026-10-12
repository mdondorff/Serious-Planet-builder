## Why

M3 task 7: ADR 0019 describes space reclamation and a size cap, ADR 0024 lists them as not built, and STRM-008 only describes the pack. A hash-version or generator bump leaves dead records, and the cache needs an upper bound on disk use.

## What Changes

- STRM-013 added: compaction under the writer lock, byte-deterministic and idempotent output, dead-pack removal, damage rules (ADR 0024 point 8), the size cap by rank. It uses ADR 0019's design as accepted; the one open piece is the last-hit sidecar for LRU, which is an input (`rank`) here with the pack age as default, so adding it later changes no mechanism.
- `planet_cache::pack::compact` (`crates/cache/src/pack/compact.rs`), `rank_by_pack_age`, `CompactPolicy`, `CompactReport`; the writer's lock code is a shared helper (`take_dir_lock`).
- `World::is_live` and `GlobalWorld::is_live` (versions, hash version, node set id).
- `planet generate --compact-cache DIR [--cap-bytes N]` and `cargo xtask cache-compact`.
- Tier-A tests: `cache/tests/compact.rs` (8: dead and superseded, byte determinism across write orders and pack splits, idempotence, the cap and eviction order, hash-version packs and unusable files, corrupt newest copy, lock, wrong body), an app test through `run`, xtask argument tests.
- Not here: the last-hit sidecar, a per-tile node set id (a global block's liveness is the seed node's until a cell-reading field is stored), opening-time automatic compaction.
