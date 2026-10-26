## MODIFIED Requirements

### Requirement: STRM-040 Landing refreshes and invalidates
When a chunk lands, the tiles that read it (the chunk under them and the neighbours within their read reach, STRM-038) SHALL be refreshed through the stale-tile machinery (the old tile keeps drawing while the refresh runs, REND-032), and the origin heights and mesh keys the plan cache holds for them, resident or not, SHALL be invalidated before the next plan reads the cache (STRM-058). The viewer's use of this (building the field with the landed chunk, calling it each frame) arrives with the chunk stage in the viewer (STRM-046). Heights sampled by the plan SHALL never request a chunk.

Verify: A
Status: active
Source: SPIKE-34 sections 1 and 4, REND-032, LOD-009

#### Scenario: Landing
- **WHEN** a chunk lands during a flight
- **THEN** exactly the tiles at or above the apply level under it are refreshed and their cached origin heights dropped, and no chunk is requested by sampling

## ADDED Requirements

### Requirement: STRM-058 The main thread never takes the plan cache's lock
The plan job holds the plan cache's lock for the whole plan, which can take a second while chunk and horizon jobs hold the cores, so the main thread SHALL NOT take it. Everything the main thread asks of the cache (the tiles of a landed chunk, the tiles of an edit, a change of the item revision) SHALL wait in one ordered queue that the plan job applies, under the lock, before it plans, landed chunks that follow one another in one pass. The queue SHALL stay bounded: a clear replaces what was queued before it, tile drops merge without duplicates, a chunk is queued once, and a queue that outgrows the cache becomes a clear.

Verify: A
Status: active
Source: SPIKE-44 step 2, CON-22, STRM-040

#### Scenario: The queue is bounded
- **WHEN** one chunk lands a thousand times, two thousand overlapping tile drops are asked, six thousand distinct chunks land, and a clear follows
- **THEN** the queue holds one entry for the chunk, one merged tile drop with no tile twice past 4,096 tiles, at most 4,097 entries in the storm, and after the clear one entry that makes the tile drops and landings after it moot while an item change after it is still queued in order

#### Scenario: A landing waits for the plan job
- **WHEN** a chunk lands while the plan cache holds the origin heights of tiles that read it and of tiles that do not, and two landings come together
- **THEN** the landing runs nothing on the calling thread, and a plan that starts after it finds no origin height of a tile that reads a landed chunk and all the others
