## MODIFIED Requirements

### Requirement: STRM-003 Content-addressed keys
A persistent tile cache key SHALL be the first 16 bytes of BLAKE3 (`derive_key` mode, context `planet-builder cache key v1`) over a canonical encoding of everything the artefact depends on (ADR 0018, clarified by ADR 0023): the format tag, the kind, the hash version and generator version, the implementation id, the face mapping id, the body id and the 128-bit body parameter hash (id, seed, radius, reference surface), the tile id, the key of its source artefact with a presence byte (a mesh from its heights), its output parameters (resolution, grid size, skirt fraction, mesh version) in ascending tag order, and the read set as entries (a node entry with the 128-bit content hash of the node read, and a query entry with the kind, the parameters and the 128-bit result hash of each spatial query, an empty result included), nodes sorted by path then queries by kind and parameters, duplicates collapsed. A cell or body query's result hash covers the matching items' ids, content hashes and direction bits in id order; a footprint query (kind 3, WDEF-007) covers the items in application order (priority, then id big-endian), each as a 16-byte id and a 128-bit item hash that contains its anchor, reach, bound, layer and priority. The format tag is 2 for a key that carries no footprint (kind 3) or region summary (kind 4) entry, byte for byte as before, and 3 for a key that carries one, so no stored entry is invalidated and the two layouts never alias (ADR 0066 point 2). Every variable-length part SHALL be length-prefixed and every list preceded by its count, every hash that feeds a key SHALL have its own `derive_key` context, a change of any input SHALL change the key, equal keys SHALL imply equal tile bytes (checked by STRM-004, `cache-verify`), and the encoding SHALL have a version tag and pinned known answers that are the same on every platform.

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
Every record of a world SHALL carry a node set id: the first 16 bytes of BLAKE3 `derive_key` (context `planet-builder node set v1`) over the paths of the field's field-wide reads and of the body's radius node (`body.0.radius_m`), deduplicated, sorted by UTF-8 byte order and concatenated, each as its byte length (u32 little endian) followed by its UTF-8 bytes, with no count prefix (ADR 0025 point 3). It SHALL depend on which nodes are read and not on their values, SHALL be equal for the records of one world, and its encoding SHALL have a pinned known answer that is the same on every platform. It is per world for a field that reads no items; for a field that reads items (`reads_cells`) the id function (`node_set_id_of_tile`) SHALL give a per-tile id, which a record writer of such a field SHALL use (the writers are wired with the first stored item-reading stage, E2; no production field reads items before then), and it also SHALL cover the identities of the tile's own item queries (kind, body, layer, tile id and halo bits, never the result), so it depends on which reads the tile makes and not on what they found, and equal tiles of a world have equal ids (ADR 0025 point 3, ADR 0066).

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025, ADR 0066

#### Scenario: Pinned encoding
- **WHEN** the node set id of the default planet is computed by hand from its sorted paths and compared with the library's
- **THEN** they are equal and equal to the pinned known answer, another seed gives the same id, and every stored record carries it
