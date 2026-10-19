## MODIFIED Requirements

### Requirement: AUTH-001 Separate, geo-anchored storage
Authored content SHALL be stored separately from the procedural base, anchored in geo-coordinates (latitude, longitude, height, heading), as one TOML file per item named `<ULID>.toml` in the item directory of its body, so that adding an item adds a file and two additions never touch the same file (git merges the directories without a conflict); binary payloads belong in content-addressed blobs (not built yet). Storage SHALL sit behind an item store interface (`ItemStore`: `put`, `delete`, `get` and `load_all` in id order, ADR 0066 point 6) with two backends, memory and loose files (`LooseFileStore`, `<root>/<body>/<ULID>.toml`, each file written whole beside its place and moved over it); an id and a content hash do not depend on the backend. The loose store SHALL refuse, naming the file, a name that is not `<ULID>.toml` (hidden files are not items), an `id` or a body that is not its name's or its directory's, and a file that is not a valid item.

Verify: A
Status: active
Source: CON-09, ADR 0007, ADR 0066 point 6

#### Scenario: Anchor round trip
- **WHEN** an authored item is saved and loaded
- **THEN** its anchor position is unchanged within 1 mm

#### Scenario: One file per item
- **WHEN** five items are put into a loose store, one is replaced, one deleted, and items of another body are put
- **THEN** each is a file `<ULID>.toml` in its body's directory holding the canonical text, nothing else is left behind, the items load in id order, and a memory store behaves the same

#### Scenario: What the loose store refuses
- **WHEN** the item directory holds a file not named for a ULID, a copy named for another id, a copy in another body's directory, or a file that is not an item
- **THEN** loading fails naming the file and the reason, and a hidden file is ignored

### Requirement: AUTH-002 Recorded versions
Every authored item SHALL record the generator version, the stage version and the hash of the base definition (`WorldDefinition::base_hash`) it was authored against, and checking an item against other active versions (`Item::check_versions`) SHALL fail with a message that names every difference (item value and active value) and the options: pin the definition to the item's generator version, freeze the region, or re-author the item. (Pinning and freezing themselves are AUTH-003.)

Verify: A
Status: active
Source: CON-09, ADR 0007

#### Scenario: Version mismatch
- **WHEN** an item authored against generator version 4, stage version 2 and one base hash is checked under generator version 5, under stage version 3, under another base hash, and under all three
- **THEN** each check fails naming the item, the recorded and the active values and the three options, and the last names three differences; the item's own versions pass

## ADDED Requirements

### Requirement: AUTH-005 The item file
An item file SHALL be TOML with the keys `schema_version` (1), `id` (the canonical 26-character upper-case Crockford ULID, first character at most `7`), `body`, `surface`, `layer`, `priority` (integer), `kind`, `reach_m`, `max_delta_m`, `base_hash` (16 lower-case hex digits), `generator_version`, `stage_version`, the optional `name` and `notes`, and the tables `[anchor]` (`latitude`, `longitude`, `height`, `heading`) and `[params]` (the kind's parameters, finite floats by name). The writer SHALL emit exactly that key order, one key per line, floats in shortest round-trip form with `-0.0` written `0.0`, and write only a valid item (finite numbers, latitude in -90..90, longitude in -180..180, reach in 0..one cell edge, height bound at least 0, names of letters, digits, `_`, `-`). The loader SHALL refuse a missing, non-integer or other `schema_version` (a newer one with the reason), an unknown key at the top, in `[anchor]` or as a table, a missing key, a value of the wrong type, a non-finite number, an integer that is not exact as a float, an id that is not the canonical ULID and what the writer refuses, each naming the key, and SHALL accept an exact integer literal for a float and read `-0.0` as `0.0`. The ULID of a new item is made by the command from an injected clock (48 bits of milliseconds) and injected entropy (80 bits), never by the generator. The content hash SHALL be BLAKE3-128 under the context `planet-builder item hash v1` over the parsed form: the schema version, the 16-byte id, the body, the surface, the layer, the priority, the kind, the four anchor numbers, the reach, the height bound, the parameters in name order, the base hash and the two versions, numbers as bit patterns with `-0.0` made `0.0` and strings length-prefixed, and not the name or the notes, so formatting, key order, comments and merges never change it (WDEF-003). An item goes into the definition (`Item::put_into`) with its content hash, the unit direction of its anchor and its layer, priority, reach and height bound, so putting back an item restores the exact definition hash.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0007, WDEF-003

#### Scenario: Canonical text
- **WHEN** an item is written, read and written again, with floats at the edge of the range, a name with quotes, a backslash, a newline and non-ASCII text, and `-0.0` anywhere
- **THEN** the text is the one fixed by the key order above, reading it gives the item (bit for bit, `-0.0` as `0.0`) and writing that gives the same text

#### Scenario: Other spelling
- **WHEN** a hand-written file has another key order, comments, spacing and integer literals for floats
- **THEN** it parses to the same item, the same hash and, written, the canonical text

#### Scenario: Refusals
- **WHEN** a file has an unknown key, a missing key, a wrong type, nan or inf, a bad version, a lower-case or short id, a latitude out of range, a reach over a cell edge, or a bad name
- **THEN** it is refused naming the key, and the writer refuses the same values

#### Scenario: The hash
- **WHEN** each field of an item changes in turn, and the name and the notes change
- **THEN** every field changes the hash, differently from the others, the name and the notes do not, and the hash equals BLAKE3-128 of the layout above computed independently in the test
