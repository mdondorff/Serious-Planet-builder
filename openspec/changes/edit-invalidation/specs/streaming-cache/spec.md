## MODIFIED Requirements

### Requirement: STRM-010 Node set id
Every record of a world SHALL carry a node set id: the first 16 bytes of BLAKE3 `derive_key` (context `planet-builder node set v1`) over the paths of the field's field-wide reads and of the body's radius node (`body.0.radius_m`), deduplicated, sorted by UTF-8 byte order and concatenated, each as its byte length (u32 little endian) followed by its UTF-8 bytes, with no count prefix (ADR 0025 point 3). It SHALL depend on which nodes are read and not on their values, SHALL be equal for the records of one world, and its encoding SHALL have a pinned known answer that is the same on every platform. It is per world for a field that reads no items; for a field that reads items (`reads_cells`) it SHALL be per tile and also cover the identities of the tile's own item queries (kind, body, layer, tile id and halo bits, never the result), so it depends on which reads the tile makes and not on what they found, and equal tiles of a world have equal ids (ADR 0025 point 3, ADR 0066).

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025, ADR 0066

#### Scenario: Pinned encoding
- **WHEN** the node set id of the default planet is computed by hand from its sorted paths and compared with the library's
- **THEN** they are equal and equal to the pinned known answer, another seed gives the same id, and every stored record carries it
