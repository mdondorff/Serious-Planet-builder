## MODIFIED Requirements

### Requirement: STRM-039 Chunk jobs never block a frame
Chunk jobs SHALL run on the injected spawner (CON-04), ordered by distance to the camera, at most `chunk_workers` at once, and no frame SHALL wait for one. A tile whose chunk is not ready SHALL draw the base. No frame fades a landed chunk in and none counts a share of tiles on the base since ADR 0086.

Verify: A, B
Status: active
Source: ADR 0077 point 7, CON-22, CON-04, ADR 0086

#### Scenario: Flight without waiting
- **WHEN** a scripted flight from 30 km to 1.8 m and a 300 m pan run on the fake clock with a stand-in chunk cost of 0.25 s per km2 and 6 workers
- **THEN** no frame waits for a chunk and every chunk lands (the first within 10 s at the pan)

### Requirement: STRM-042 Chunk prefetch below the wanted set
Chunks passed to `ChunkManager::update_with` as prefetch SHALL be started only on workers no wanted chunk needs, and a wanted chunk submitted after them SHALL run before them. No caller passes a prefetch set since ADR 0086 (the frame's track and the app's request are removed); the requirement goes with the manager.

Verify: A
Status: active
Source: STRM-018, SPIKE-34 section 7, D7, ADR 0086

#### Scenario: A wanted chunk after a prefetch
- **WHEN** two chunks are prefetched while nothing is wanted and a chunk is then wanted
- **THEN** the prefetched chunks start as prefetch jobs and the pool runs the wanted chunk before either of them

### Requirement: STRM-047 The compact tile carries the chunk change
The compact tile builder SHALL store the chunk change of a tile at the tile's own level beside its heights (`HeightField::level_change`, forwarded by every wrapper that forwards `height_at_level`), and the CPU mirror `CompactTile::shade_vertex_faded` SHALL take `fade x change` off a vertex. The renderer and the plan do not read the change or a weight since ADR 0086; the requirement goes with the generators' chunk field.

Verify: A
Status: active
Source: STRM-038 (the chunks a tile reads), GEN-025, ADR 0086

#### Scenario: Change
- **WHEN** a tile at the pyramid top over a ready chunk is built
- **THEN** the stored change equals the field's height at the tile's level minus the base's, and the mirror's vertices at fade `f` are the final ones minus `f` times the change

### Requirement: STRM-055 The job log tells a refresh and its waits
The job log (STRM-022) SHALL record for each job the milliseconds from the request of its tile to its dispatch (the wait in the streamer's queue) and whether it rebuilds a stale resident tile (a refresh, STRM-026). `refresh_summary` SHALL put the refresh jobs of a slice in one line: their count and the median, 90th percentile and largest of the streamer-queue wait, the spawner-queue wait and the load, in milliseconds; none without a refresh. The viewer SHALL write that line every 10 s while there are refreshes. The readings decide nothing (CON-14).

Verify: A
Status: active
Source: m8-refresh-priority, STRM-022, STRM-026

#### Scenario: A refresh after a wait
- **WHEN** a resident tile is refreshed and 250 ms of the clock pass before the step that dispatches it
- **THEN** its sample is a refresh that is done with a streamer queue wait of 250 ms, the samples of the missing tiles before it are not refreshes with a wait of 0, and the summary counts the one refresh

### Requirement: STRM-056 A stale chunk keeps drawing and is re-solved when read
An edit SHALL mark the ready chunks whose read set (GEN-081) it touches stale. A stale chunk SHALL keep drawing its old version until the new version lands, and SHALL be re-solved only when the plan reads it: nearest to the camera first, ahead of prefetch, with at most two of the chunk workers on stale chunks. The new version SHALL land as a second landing. No frame waits for it (CON-22).

Verify: A
Status: active
Source: m8-chunk-version, STRM-042

#### Scenario: An edit over a drawn chunk
- **WHEN** a world with a stand-in chunk stage on a fake clock has a chunk ready, gets an edit inside its halo, and the plan keeps reading the chunk
- **THEN** the chunk draws its old version until the new one lands, one job runs for it, a chunk the plan does not read stays stale and unsolved, and at most two stale jobs run at once

### Requirement: STRM-058 The main thread never takes the plan cache's lock
The plan job holds the plan cache's lock for the whole plan, which can take a second while chunk and horizon jobs hold the cores, so the main thread SHALL NOT take it. Everything the main thread asks of the cache (the tiles of an edit, a change of the item revision) SHALL wait in one ordered queue that the plan job applies, under the lock, before it plans. The queue SHALL stay bounded: a clear replaces what was queued before it, tile drops merge without duplicates, and a queue that outgrows the cache becomes a clear.

Verify: A
Status: active
Source: SPIKE-44 step 2, CON-22

#### Scenario: The queue is bounded
- **WHEN** two thousand overlapping tile drops are asked, more item changes than the cache keeps follow, and a clear follows
- **THEN** the queue holds one merged tile drop with no tile twice past 4,096 tiles, the item changes the cache keeps, and after the clear one entry that makes the tile drops after it moot while an item change after it is still queued in order

#### Scenario: A landing waits for the plan job
- **WHEN** a chunk lands while the plan cache holds the origin heights of tiles that read it and of tiles that do not, and two landings come together
- **THEN** the landing runs nothing on the calling thread, and a plan that starts after it finds no origin height of a tile that reads a landed chunk and all the others

## REMOVED Requirements

### Requirement: STRM-040 Landing refreshes and invalidates

### Requirement: STRM-052 The fade clock of a landed chunk

### Requirement: STRM-053 A rebuilt tile carries the increment over the tile it replaces

### Requirement: STRM-054 A replaced tile keeps its weight until the swap
