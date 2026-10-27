## Why
Owner test 2026-10-27: one editing hour landed 1192 chunks and evicted 1150 (stall report counters: 390 computed, 802 disk hits). Eviction is by time (`evict_ms` 5 s unwanted), not by the byte cap (512 MiB, about 250 chunks, with about 42 resident): a chunk the camera leaves for 5 s is dropped and a return decodes it from the disk cache (or recomputes it), work on the chunk pool during an edit session that revisits the same few chunks. Planner (2026-10-27): raise it to 30 to 60 s, state the RAM cost under ADR 0083's memory share.

## What Changes
1. `chunk_evict_s` defaults to 30 (was 5): `ChunkConfig::default().evict_ms` is 30 000.

## Impact
- RAM: none above the cap. The chunk pool has its own byte cap (STRM-041: 512 MiB by default, the least recently wanted leave first beyond it); a longer time only lets the pool fill to that cap sooner. The terrain share of ADR 0083 is unchanged. The horizon maps (STRM-048) leave with their chunk, so each stays up to 30 s longer: they are not in the chunk pool's byte count, but there is at most one per ready chunk (about 250 at the cap) and a map is small next to a chunk; its size is not measured here. At 8 m cells a chunk is about 2 MiB, so 512 MiB holds about 250; a flight at 300 m/s crosses a 4.9 km chunk in 16 s, so 30 s keeps the chunks of the last two crossings.
- Spec: STRM-041 (default 30). Code: `streaming/chunks.rs`. Test: `streaming/tests/chunks.rs` (the eviction test, 32 s against 30 s).
