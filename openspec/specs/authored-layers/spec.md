# authored-layers Specification

## Purpose
Hand-authored content (placed items and edits) kept apart from the procedural base: geo-anchored text files with content-addressed binary blobs, recorded versions, version pinning with freeze/bake, and commands with undo. Governed by CON-09 and ADR 0007 (authored-layer data model); the item index is ADR 0021. Planned for the editor milestones.

## Requirements

### Requirement: AUTH-001 Separate, geo-anchored storage
Authored content SHALL be stored separately from the procedural base, anchored in geo-coordinates (latitude, longitude, height, heading), as one TOML file per item named `<ULID>.toml` in the item directory of its body, so that adding an item adds a file and two additions never touch the same file (git merges the directories without a conflict); binary payloads belong in content-addressed blobs (not built yet). Storage SHALL sit behind an item store interface (`ItemStore`: `put`, `delete`, `get` and `load_all` in id order, ADR 0066 point 6) with two backends, memory and loose files (`LooseFileStore`, `<root>/<body>/<ULID>.toml`, each file written whole beside its place and moved over it); an id and a content hash do not depend on the backend. The loose store SHALL refuse, naming the file, a name that is not `<ULID>.toml` (hidden files are not items), an `id` or a body that is not its name's or its directory's, and a file that is not a valid item. A write that fails leaves no temporary file behind (a temporary file has a hidden name no other writer uses and is flushed before it is moved into place; keep `.*.tmp` in the ignore list of the repository). Revisit trigger (ADR 0066 point 6): more than 20,000 items in one body, or an item load over 2 s on Rasierklinge, makes the choice of a packed or database backend, from measurements; nothing counts or times them yet (tracked in `docs/status.md`).

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

### Requirement: AUTH-003 Version pinning and freeze/bake
A region SHALL be freezable (baked to authoritative tiles at all levels with a blending ring at the border), and a definition SHALL be pinnable to a generator version.

Verify: A
Status: planned
Source: CON-09, ADR 0007

#### Scenario: Frozen region
- **WHEN** the base generator version changes
- **THEN** tiles inside a frozen region are unchanged and tiles in the blending ring are continuous with both sides

### Requirement: AUTH-004 Commands and undo
Edits SHALL be commands over the WorldDefinition (`planet_editor::commands::Editor`): put (a new item whose ULID the command makes from an injected clock and entropy, authored against the active versions and the base hash of the definition now), change (a copy of the item edited, the id and the body kept; a change that leaves the item as it was is not a command) and delete. Every command SHALL write its item files through the item store, undo and redo too, and git is the history: the command log is only the in-memory stack of the session (not persisted, `UNDO_LIMIT` = 10,000 commands, the oldest dropped), cleared of its redo half by a new command. Undo and redo SHALL restore exactly the item before or after the command, so the definition hash is exactly the previous (next) one, for at least 1,000 operations. A drag (`begin_edit`, `edit` steps, `end_edit`) SHALL be one command: its steps change the definition at once and no file, its end writes the file once and records the one command (none when the item is back as it was), `cancel_edit` restores the item and records nothing, and no other command, undo, redo or load runs while a drag is open (each is an error). A command that fails (an invalid item, a body the index refuses, an unknown id, a store that cannot write or remove) SHALL change nothing: not the definition, not the files, not the stack. Each command SHALL return the places to refresh (`Affected`: the body, the unit direction of the anchor and the reach) of the item before and after it (a put has only the after, a delete only the before), for the footprint find of the tiles. Two additions made on two branches SHALL merge in git without a conflict (one file per item) and load as one definition, whatever the merge order.

Verify: A
Status: active
Source: ADR 0007, ADR 0066 points 6 and 7

#### Scenario: Undo all
- **WHEN** 1,000 random commands (puts, changes, deletes and drags) are applied, then undone one by one, then redone one by one, over memory and over loose files
- **THEN** after each undo the definition hash equals the one before that command, after the 1,000th the hash equals the starting hash and no item or file is left, each redo reaches the hash the command left, and the files always equal the items

#### Scenario: A drag
- **WHEN** an item is dragged in 50 steps and the drag is ended, cancelled, or ends where it began
- **THEN** the steps change the definition and no file, the end writes one file and records one command that one undo takes back whole, a cancel records nothing, and a drag with no net change records nothing and writes nothing

#### Scenario: Failures change nothing
- **WHEN** a put is invalid or refused by the index, a change is invalid, an id is unknown, or the store fails on a put, a delete, an undo or the end of a drag
- **THEN** the definition, the files and the stack are as they were, a failed undo can be tried again and a failed end keeps the drag open

#### Scenario: Merging in git
- **WHEN** two people each add an item, change a different item and one deletes a third on two branches of one repository, and the branches are merged either way
- **THEN** the merge has no conflict and the merged files load to one definition with both additions, both changes and the deletion, with the same hash whichever way it was merged

#### Scenario: Ids
- **WHEN** ids are made from a clock that stands still, goes back, moves on, and from entropy that is all ones
- **THEN** they increase strictly (the same millisecond adds one to the entropy, an overflow takes the next millisecond), the same sources give the same ids and other entropy gives other ids

### Requirement: AUTH-005 The item file
An item file SHALL be TOML with the keys `schema_version` (1, or 2 for an item with a points list), `id` (the canonical 26-character upper-case Crockford ULID, first character at most `7`), `body`, `surface`, `layer`, `priority` (integer), `kind`, `reach_m`, `max_delta_m`, `base_hash` (16 lower-case hex digits), `generator_version`, `stage_version`, the optional `name` and `notes`, the optional `group` (a canonical ULID: the line or region the item is a part of) and `part` (the integer index of the item in its group), the optional `mask_layer` (a layer name: the layer a region clips, WDEF-012; only with points and parsed with the same name rule as `layer`), and the tables `[anchor]` (`latitude`, `longitude`, `height`, `heading`) and `[params]` (the kind's parameters, finite floats by name), then, for an item with points, the array of tables `[[points]]` (ADR 0069: `latitude`, `longitude`, in order). The writer SHALL emit exactly that key order, one key per line, floats in shortest round-trip form with `-0.0` written `0.0`, and write only a valid item: finite numbers; the surface `sphere` (the only reference surface, CON-15; the direction of the anchor is its conversion of latitude and longitude at height 0); latitude in -90..90, longitude in (-180, 180] and 0 at a pole, so a place has one spelling, for the anchor and every point; heading in 0..360; reach in 0..one cell edge; `max_delta_m` in 0..100,000 m; layer, kind and parameter names of letters, digits, `_` and `-`; points only on a kind that takes them, at most 64, the first equal to the anchor's latitude and longitude, no two consecutive points equal; `group` and `part` only together and only with points. The writer emits schema 1 for an item without points, so existing files and their hashes do not change, and schema 2 for an item with points. The index takes an item (`Item::put_into`) only when it is valid. The loader SHALL refuse a missing, non-integer or other `schema_version` (a newer one with the reason; 1 with points, 2 without), an unknown key at the top, in `[anchor]`, in a `[[points]]` table or as a table, a missing key (`[params]` may be missing: the item has no parameters), a value of the wrong type, a non-finite number, an integer that is not exact as a float, an id or group that is not the canonical ULID and what the writer refuses, each naming the key, and SHALL accept an integer literal for a float when its magnitude is at most 2^53 (a larger one is refused: write it with a decimal point) and read `-0.0` as `0.0`. The ULID of a new item is made by the command from an injected clock (48 bits of milliseconds) and injected entropy (80 bits), never by the generator. The content hash SHALL be BLAKE3-128 under the context `planet-builder item hash v1` over the parsed form: the schema version, the 16-byte id, the body, the surface, the layer, the priority, the kind, the four anchor numbers, the reach, the height bound, the parameters in name order, the base hash and the two versions, numbers as bit patterns with `-0.0` made `0.0` and strings length-prefixed, and not the name or the notes (layout, in this order, little-endian unless said: schema version `u32`; id 16 bytes big-endian; body `u32`; surface, layer as `u32` length and UTF-8 bytes; priority `i32`; kind as length and bytes; latitude, longitude, height, heading, reach, height bound as `u64` bit patterns; parameter count `u32`, then each name as length and bytes and its value bits; base hash `u64`; generator version `u32`; stage version `u32`; and, for schema 2 only, the group as a flag byte (1, then 16 bytes big-endian, or 0 and no bytes), the part as a flag byte (1, then `u32`, or 0), the mask layer as length and bytes (length 0 when absent), the point count `u32` and each point's latitude and longitude as `u64` bit patterns; the digest is `blake3::derive_key(context, material)` truncated to its first 16 bytes and read as a big-endian `u128`), so formatting, key order, comments and merges never change it (WDEF-003). An item goes into the definition (`Item::put_into`) with its content hash, the unit direction of its anchor and its layer, priority, reach and height bound, and with its points as unit directions, so putting back an item restores the exact definition hash.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0069, ADR 0007, WDEF-003

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
- **THEN** every field changes the hash, differently from the others, the name and the notes do not, and the hash equals BLAKE3-128 of the layout above computed independently in the test, and a pinned item has a pinned hash and a pinned canonical text

#### Scenario: Points
- **WHEN** a line item with points, a group and a part is written, read and written again, and a schema-1 item is written
- **THEN** the line is schema 2 with the keys in the fixed order and its `[[points]]` in order, reading it gives the item and the same text, the schema-1 item is byte-identical to what the writer made before schema 2 existed, and a point or the group or part changes the hash
- **AND** a file with points and schema 1, without points and schema 2, a group without a part, a first point that is not the anchor, a repeated consecutive point, 65 points or points on a kind that takes none is refused naming the key
