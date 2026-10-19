## MODIFIED Requirements

### Requirement: STRM-003 Content-addressed keys
A persistent tile cache key SHALL be the first 16 bytes of BLAKE3 (`derive_key` mode, context `planet-builder cache key v1`) over a canonical encoding of everything the artefact depends on (ADR 0018, clarified by ADR 0023): the format tag, the kind, the hash version and generator version, the implementation id, the face mapping id, the body id and the 128-bit body parameter hash (id, seed, radius, reference surface), the tile id, the key of its source artefact with a presence byte (a mesh from its heights), its output parameters (resolution, grid size, skirt fraction, mesh version) in ascending tag order, and the read set as entries (a node entry with the 128-bit content hash of the node read, and a query entry with the kind, the parameters and the 128-bit result hash of each spatial query, an empty result included), nodes sorted by path then queries by kind and parameters, duplicates collapsed. A cell or body query's result hash covers the matching items' ids, content hashes and direction bits in id order; a footprint query (kind 3, WDEF-007) covers the items in application order (priority, then id big-endian), each as a 16-byte id and a 128-bit item hash that contains its anchor, reach, bound, layer and priority. A region summary entry (kind 4, WDEF-008) has the parameters `body u32 | layer (u32 length + UTF-8) | level u8 | node raw id u64` and the 16-byte node hash itself as its result, a cell hashed under `planet-builder cell hash v1` and an inner node under `planet-builder summary node v1`. The format tag is 2 for a key that carries no footprint (kind 3) or region summary (kind 4) entry, byte for byte as before, and 3 for a key that carries one, so no stored entry is invalidated and the two layouts never alias (ADR 0066 point 2). Every variable-length part SHALL be length-prefixed and every list preceded by its count, every hash that feeds a key SHALL have its own `derive_key` context, a change of any input SHALL change the key, equal keys SHALL imply equal tile bytes (checked by STRM-004, `cache-verify`), and the encoding SHALL have a version tag and pinned known answers that are the same on every platform.

Verify: A
Status: active
Source: CON-05, CON-06, CON-14, ADR 0006, ADR 0018, ADR 0066

#### Scenario: Version bump
- **WHEN** the generator version changes
- **THEN** every key changes and no stale tile is served

#### Scenario: Every input
- **WHEN** any single input of a key changes (kind, versions, implementation, mapping, body, body parameters, tile, source key, an output parameter, a node entry, a query entry)
- **THEN** the key changes, and a set of generated inputs gives distinct encodings and keys

#### Scenario: Orders
- **WHEN** the same output parameters or reads are given in another order or with duplicates
- **THEN** the key is the same

#### Scenario: Reads
- **WHEN** a field that reads cells is keyed
- **THEN** its persistent key follows the same per-tile reads as its 64-bit key (one source), and a field that says it reads cells but reports none cannot be keyed

#### Scenario: Empty query
- **WHEN** a spatial query found nothing and an item is then added to that cell, or is added elsewhere
- **THEN** the key changes in the first case and not in the second

#### Scenario: Format tag per key
- **WHEN** a key carries no footprint or summary entry, and when it carries one
- **THEN** the first has tag 2 and the bytes it always had (the pinned known answers are unchanged) and the second has tag 3

### Requirement: STRM-010 Node set id
Every record of a world SHALL carry a node set id: the first 16 bytes of BLAKE3 `derive_key` (context `planet-builder node set v1`) over the paths of the field's field-wide reads and of the body's radius node (`body.0.radius_m`), deduplicated, sorted by UTF-8 byte order and concatenated, each as its byte length (u32 little endian) followed by its UTF-8 bytes, with no count prefix (ADR 0025 point 3). It SHALL depend on which nodes are read and not on their values, SHALL be equal for the records of one world, and its encoding SHALL have a pinned known answer that is the same on every platform. It is per world for a field that reads no items; for a field that reads items (`reads_cells`) the id function (`node_set_id_of_tile`) SHALL give a per-tile id, which a record writer of such a field SHALL use (the writers are wired with the first stored item-reading stage, E2; no production field reads items before then), and it also SHALL cover the identities of the tile's own item queries (for a footprint query: kind, body, layer, tile id and halo bits; for a region summary node: kind, body, layer, level and node id; never a result or a hash), so it depends on which reads the tile makes and not on what they found, and equal tiles of a world have equal ids (ADR 0025 point 3, ADR 0066).

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025, ADR 0066

#### Scenario: Pinned encoding
- **WHEN** the node set id of the default planet is computed by hand from its sorted paths and compared with the library's
- **THEN** they are equal and equal to the pinned known answer, another seed gives the same id, and every stored record carries it

## ADDED Requirements

### Requirement: STRM-026 Refresh of resident tiles
When an item edit changes what some tiles read (the footprint find of the edit, WDEF-007), the streamer SHALL request each resident tile of that set again without dropping it (`Streamer::refresh`): the tile stays resident and served from its slot until the new version arrives, the new version takes a second slot, replaces the old one when its job completes (the loop is told with a new `Added` change) and gives the old slot back. The request SHALL be of the tile's class (draw, residency or prefetch) and, in the residency and prefetch classes, ahead of the other requests of the class (rank 0 instead of the level: an exception to the order of STRM-005, "coarse before fine", for edited tiles only), so edited tiles are not queued behind the rest of R; a draw request has no rank to go ahead of. A resident tile in no class SHALL be evicted instead of refreshed. A tile whose job is running SHALL be told to stop and its result SHALL be dropped. A resident tile that has not been replaced yet SHALL stay marked stale and SHALL be requested again whenever nothing is running for it and something wants it (when its job ended without replacing it, after the retry delay of a failed job, or when it is wanted again after it lost every class), so no tile ever holds a version made from the field before the edit and a failed refresh is never forgotten. While a tile is refreshed it holds two slots (an exception to STRM-006 and STRM-007 that lasts until the new version replaces it, the old slot then returning to the pool); a refresh that finds no free slot waits, and the old version is served meanwhile. The frame loop's edit event (`ManagedLoop::edit`) SHALL forget the plan cache entries (origin height, mesh key, volume) of the found tiles, refresh them and ask for a new plan at the next frame although the camera did not move.

Verify: A
Status: active
Source: SPIKE-13 variant D, ADR 0066, ADR 0021 point 2, M6 plan row E1

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

#### Scenario: The edit event
- **WHEN** an edit event names the resident tiles it changed while the camera stands still
- **THEN** a new plan is delivered although the request did not change, the old tiles are drawn until their refreshes finish, and the frame is the synchronous frame of the new field again afterwards, with every requested refresh counted as a replacement
