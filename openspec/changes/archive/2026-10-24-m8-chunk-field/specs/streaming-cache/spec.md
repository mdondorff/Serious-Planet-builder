## ADDED Requirements

### Requirement: STRM-038 Chunk results in tile keys
A tile key SHALL chain the content hash of the chunk results the tile reads, its own chunk and every neighbouring chunk within its read reach (a tile near a chunk border reads cells of the neighbour); a chunk that is not ready SHALL chain word 0, so a stand-in tile and an exact tile never share a key and a neighbour landing re-keys exactly the tiles it can change, and a tile above the pyramid's top level SHALL have the base's key whatever the chunks hold.

Verify: A
Status: active
Source: SPIKE-34 section 6, STRM-027, STRM-029, ADR 0022

#### Scenario: Keys
- **WHEN** a tile at the pyramid top is keyed with its chunk not ready, then ready, a tile one level above the top in both states, and a tile at the chunk border with its neighbour not ready, then ready
- **THEN** the first two keys differ, the two keys of the coarser tile are equal and equal the base's, and the border tile's keys differ while a tile in the middle of the chunk keeps its key

### Requirement: STRM-039 Chunk jobs never block a frame
Chunk jobs SHALL run on the injected spawner (CON-04), ordered by distance to the camera, at most `chunk_workers` at once, and no frame SHALL wait for one. A tile whose chunk is not ready SHALL draw the base; on landing, the chunk's change SHALL fade in per tile over `chunk_fade_ms` (default 500) on the injected clock, through a plain f32 weight (CON-11; the weight is the CPU side, drawing with it is REND-039). The per-frame log SHALL count the plan's nodes that read chunk detail (at or finer than the pyramid's top level, column `chunk_nodes`) and those among them whose own chunk is not ready (column `chunk_stand_in`): the stand-in share, for the speed-envelope scene.

Verify: A, B
Status: active
Source: ADR 0077 point 7, CON-22, CON-04, CON-11, M8 plan criteria 3 and 7

#### Scenario: Flight without waiting
- **WHEN** a scripted flight from 30 km to 1.8 m and a 300 m pan run on the fake clock with a stand-in chunk cost of 0.25 s per km2 and 6 workers
- **THEN** no frame waits for a chunk, every chunk lands (the first within 10 s at the pan), and the log shows the stand-in share falling to zero

#### Scenario: Fade
- **WHEN** a chunk lands under visible tiles
- **THEN** each tile's weight rises linearly from 0 to 1 over `chunk_fade_ms`, so the drawn height `base + w x change` steps by at most the weight step times the change per frame; for a change of at most 7 m that is within 0.25 m per frame at 60 frames per second over 500 ms, and a change of 30 m needs 2 s

### Requirement: STRM-040 Landing refreshes and invalidates
When a chunk lands, the tiles that read it (the chunk under them and the neighbours within their read reach, STRM-038) SHALL be refreshed through the stale-tile machinery (the old tile keeps drawing while the refresh runs, REND-032), and the origin heights and mesh keys the plan cache holds for them, resident or not, SHALL be invalidated. The viewer's use of this (building the field with the landed chunk, calling it each frame) arrives with the chunk stage in the viewer (STRM-046). Heights sampled by the plan SHALL never request a chunk.

Verify: A
Status: active
Source: SPIKE-34 sections 1 and 4, REND-032, LOD-009

#### Scenario: Landing
- **WHEN** a chunk lands during a flight
- **THEN** exactly the tiles at or above the apply level under it are refreshed and their cached origin heights dropped, and no chunk is requested by sampling

### Requirement: STRM-041 Chunk residency, eviction and disk cache
A chunk not wanted for `chunk_evict_s` (default 5) on the injected clock SHALL be evicted from RAM, and its bytes SHALL count toward the RAM caps as a pool of their own, 512 MiB by default, the least recently wanted leaving first beyond it. A landed chunk SHALL be stored in the pack store under a key that covers the body, the process hash, the chunk grid, the pyramid top level, the radius, the key of the field below the chunk stage and the chunk, and verified on read (CON-05); a verification failure SHALL be a miss and the chunk recomputed, with the failure named in the log.

Verify: A
Status: active
Source: ADR 0077, D7 (runtime generation, disk cache), CON-05, STRM-033

#### Scenario: Evict and reload
- **WHEN** a chunk is evicted and wanted again, with its disk entry intact, then with a corrupted entry
- **THEN** the first reload reads the disk entry without computing, the second recomputes and logs the failure, and both give the identical result

### Requirement: STRM-042 Chunk prefetch along travel
The chunks needed by the camera predicted by STRM-018's velocity, one chunk ahead along the direction of travel, SHALL be requested below the wanted set in priority, and a camera that only rotates SHALL prefetch no chunk.

Verify: A, B
Status: active
Source: STRM-018, SPIKE-34 section 7, D7

#### Scenario: Travel and rotation
- **WHEN** the camera travels at 100 m/s over a fixed script, then turns 360 degrees in place
- **THEN** the next chunk along the travel is requested before it is wanted, and the rotation requests none
