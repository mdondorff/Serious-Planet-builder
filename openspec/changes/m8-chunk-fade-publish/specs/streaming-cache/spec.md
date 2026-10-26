## ADDED Requirements

### Requirement: STRM-054 Landed chunks are published one fade at a time
With `ChunkConfig::publish_gate` on, a landed chunk SHALL be ready in the `ChunkManager` (and reported in `ChunkUpdate::landed`) but visible to its readers (`snapshot`, `is_ready`, `result`, `result_hash`, so the field, the keys, the horizon jobs and the weights) only after `publish`; an evicted chunk SHALL be unpublished, and a manager without the gate shows every ready chunk. The loop SHALL queue the landed chunks and publish all that wait as one epoch when no fade runs (`ChunkFade::idle`), every resident tile that reads the previous epoch's chunks has been rebuilt and no tile new to the renderer is waiting for a fade; a landing held back for `PUBLISH_TIMEOUT_MS` (5,000) SHALL be published anyway and counted (`ManagedLoop::forced_publications`). A gated view without a fade SHALL publish what lands at once, and a chunk evicted while it waits SHALL NOT be published. A tile new to the renderer that reads a chunk whose fade is running (`ChunkFade::is_fading`) SHALL NOT be uploaded until the fade ends, its parent staying drawn. A chunk that no resident tile reads SHALL start its fade when it is published.

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

## MODIFIED Requirements

### Requirement: STRM-052 The fade clock of a landed chunk
The fade of a landed chunk's change (STRM-047) SHALL start when the change is first drawn: when the last resident tile that reads the chunk, as it was when the chunk was published, has been rebuilt and uploaded (a chunk no resident tile reads starts it at its publication; one whose readers are not all rebuilt within `PUBLISH_TIMEOUT_MS` starts it anyway, counted in `forced_publications`). Until the clock starts the chunk SHALL weigh 0, so a rebuild slower than `chunk_fade_ms` still fades. Every tile of the chunk SHALL share that one clock. A chunk that lands again without having left RAM (a new version) SHALL keep its state, so the tiles under it neither drop to the base nor fade again.

Verify: A
Status: active
Source: STRM-047, STRM-038, M8 plan criterion 3

#### Scenario: A slow rebuild and a new version
- **WHEN** a chunk lands and its rebuilt tiles reach the renderer 2 s later (the fade is 1.5 s), and later the chunk and a neighbour land again
- **THEN** the tiles under the chunk weigh 0 until the rebuilt tiles arrive, 0.5 at 750 ms after, and 1 after 1.5 s, and the later landings leave the weights at 1
