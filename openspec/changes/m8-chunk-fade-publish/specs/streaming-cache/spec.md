## ADDED Requirements

### Requirement: STRM-054 Landed chunks are published one fade at a time
With `ChunkConfig::publish_gate` on, a landed chunk SHALL be ready in the `ChunkManager` (and reported in `ChunkUpdate::landed`) but visible to its readers (`snapshot`, `is_ready`, `result`, `result_hash`, so the field, the keys, the horizon jobs and the weights) only after `publish`; an evicted chunk SHALL be unpublished, and a manager without the gate shows every ready chunk. The loop SHALL queue the landed chunks and publish all that wait as one epoch when no fade runs (`ChunkFade::idle`) and no stale tile that reads a chunk is drawn; a landing held back for `PUBLISH_TIMEOUT_MS` (5,000) SHALL be published anyway and counted (`ManagedLoop::forced_publications`). A tile new to the renderer that reads a chunk whose fade is running (`ChunkFade::is_fading`) SHALL NOT be uploaded until the fade ends, its parent staying drawn. A chunk that no resident tile reads SHALL start its fade when it is published.

Verify: A
Status: active
Source: STRM-052, STRM-053, numerics review of #711 (2026-10-26)

#### Scenario: Readers
- **WHEN** two chunks land in a gated manager, one is published, and one is evicted and lands again
- **THEN** the unpublished chunk is not ready, has no result or hash and is not in the snapshot, `unpublished` lists it, and the chunk that landed again is unpublished

#### Scenario: A stream of landings
- **WHEN** one chunk lands every 300 ms under tiles that read it, frames 100 ms apart
- **THEN** each is published within one fade and a rebuild (at most 1.9 s after it landed), every one is published, and no publication is forced

#### Scenario: A stale tile that never rebuilds
- **WHEN** the fade outlasts the timeout and a chunk waits
- **THEN** it is published 5 s after it landed and `forced_publications` is 1
