## MODIFIED Requirements

### Requirement: STRM-026 Refresh of resident tiles
When an item edit changes what some tiles read (the footprint find of the edit, WDEF-007), the streamer SHALL request each resident tile of that set again without dropping it (`Streamer::refresh`): the tile stays resident and served from its slot until the new version arrives, the new version takes a second slot, replaces the old one when its job completes (the loop is told with a new `Added` change) and gives the old slot back. The request SHALL be of the tile's class (draw, residency or prefetch) and, in the residency and prefetch classes, ahead of the other requests of the class (rank 0 instead of the level: an exception to the order of STRM-005, "coarse before fine", for edited tiles only), so edited tiles are not queued behind the rest of R; a draw request has no rank to go ahead of. A resident tile in no class SHALL be evicted instead of refreshed. A tile whose job is running SHALL be told to stop and its result SHALL be dropped. A resident tile that has not been replaced yet SHALL stay marked stale and SHALL be requested again whenever nothing is running for it and something wants it (when its job ended without replacing it, after the retry delay of a failed job, or when it is wanted again after it lost every class), so no tile ever holds a version made from the field before the edit and a failed refresh is never forgotten. A refresh request that is still queued when its tile leaves every class SHALL be dropped and counted as cancelled (the class it was queued under no longer lists the tile); the tile stays resident and stale. While a tile is refreshed it holds two slots (an exception to STRM-006 and STRM-007 that lasts until the new version replaces it, the old slot then returning to the pool); a refresh that finds no free slot waits, and the old version is served meanwhile. The frame loop's edit event (`ManagedLoop::edit`) SHALL forget the plan cache entries (origin height, mesh key, volume) of the found tiles, refresh them and ask for a new plan at the next frame although the camera did not move.

Verify: A
Status: active
Source: SPIKE-13 variant D, ADR 0066, ADR 0021 point 2, M6 plan row E1, window 13 (2026-10-19)

#### Scenario: The old version is served
- **WHEN** a resident tile is refreshed and the job has not finished
- **THEN** the tile is resident in its old slot with its old payload, a second slot is in use, and when the job completes the payload is the new one, in the second slot, and the old slot is back after the frames in flight retire

#### Scenario: Nothing to refresh
- **WHEN** a tile that is resident but in no class, or was never loaded, is named by a refresh, also while a job of the resident one runs
- **THEN** the first is evicted (its job dropped) and the second is left alone, and no request is queued

#### Scenario: A running job
- **WHEN** a refresh names a tile whose job is running, once for a first load and once for a refresh job
- **THEN** the job is told to stop, its result is dropped, and the tile ends with the version of the last field, served all the time if it was resident

#### Scenario: Ahead of the rest of the class
- **WHEN** a refresh of a fine tile and a request of a coarser tile of the residency class are both waiting
- **THEN** the refresh starts first

#### Scenario: A failed refresh
- **WHEN** the job of a refresh fails
- **THEN** the old version is served meanwhile and the refresh is requested again after the retry delay, until the new version replaces the old

#### Scenario: Quiet
- **WHEN** nothing is stale and the wants did not change
- **THEN** a frame scans and updates nothing, also after a refresh has completed, and a refresh counts one unit of work per request it queues (STRM-019)

#### Scenario: The tile leaves every class
- **WHEN** a refresh is queued for a resident tile of a residency set made from a selection (closed under its ancestors) and the wants then change so that the tile is in no class before the request starts
- **THEN** no job starts, the request is dropped and counted as cancelled, the tile stays resident with its old payload, and when it is wanted again the new version replaces it
