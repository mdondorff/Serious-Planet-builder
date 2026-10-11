## Why

M3 queue item 5, ADR 0018 (accepted by the owner, 2026-10-11): persistent and shared keys must be wide, collision resistant, specified byte for byte and identical on every platform. The in-memory 64-bit keys stay for maps and hot paths; nothing is stored under them.

## What Changes

- `planet_cache::key`: `CacheKey` (16 bytes, 32 hex characters), `KeyInputs` and `encode` (the canonical encoding of ADR 0018), `cache_key` (BLAKE3 `derive_key`, first 16 bytes), the 128-bit hashes that feed a key with their own contexts (`node_hash128`, `body_params_hash128`, `query_result_hash128`; an empty result is the hash of the empty input), `read_entry`, and `height_key` / `mesh_key` for a height field (the mesh key chains its height key).
- `world-def`: `ReadRecord` and `GenContext::read_records()` keep the typed values behind the 64-bit read set (node values, cell queries with their items, whole-body reads); `single_planet_seed_records`.
- `generators`: `HeightField::read_records` / `read_records_for` (typed twins of `inputs_hash` / `inputs_hash_for`); `Bound` keeps its context's records.
- Dependency: `blake3` `=1.8.7` with `default-features = false, features = ["std", "pure"]` (CC0-1.0 or Apache-2.0; `cargo deny` clean; `cc` is a build-dependency only and nothing is compiled from C). A test checks the official hash of the empty input.
- Modified: STRM-003 (now active). Tests: nine in `cache/tests/keys.rs` (every input changes the key on its own, orders and duplicates, boundary ambiguity and a generated sample of 1,152 distinct inputs, domains, empty queries, body and chaining, pinned known answers for the encoding and each hash).
- Not here: the tile pack and index (ADR 0019, next), `cache-verify`, wiring persistent keys into the streamer. The item hashes in query results include the item directions (as the 64-bit cell hash does), which ADR 0018 does not spell out; 64-bit item ids and content hashes widen with ADR 0021.
