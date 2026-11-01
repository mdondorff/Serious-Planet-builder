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
Every authored item SHALL record the generator version, the stage version and the hash of the base definition (`WorldDefinition::base_hash`) it was authored against, and checking an item against other active versions (`Item::check_versions`) SHALL fail with a message that names every difference (item value and active value) and the options: pin the definition to the item's generator version, freeze the region, or re-author the item. (Pinning and freezing themselves are AUTH-003.) An item authored against an OLDER stage version than the active one is not a difference: it loads, and the editor's load (`Editor::load_report`) SHALL re-key it under the active stage version (its recorded stage version becomes the active one, so its item hash changes and the stamp it makes does not), write it back to its file and name it in the report; an item of a NEWER stage version than the build's is a difference and is refused. A stage-version bump therefore never makes authored work unloadable.

Verify: A
Status: active
Source: CON-09, ADR 0007, opus-planner (2026-10-20)

#### Scenario: Version mismatch
- **WHEN** an item authored against generator version 4, stage version 2 and one base hash is checked under generator version 5, under another base hash, under stage version 1, and under all of them
- **THEN** each check fails naming the item, the recorded and the active values and the three options, the stage version 1 check naming it as newer, and the last names three differences; the item's own versions pass

#### Scenario: An older stage loads
- **WHEN** an item authored against stage version 2 is checked under stage version 3, and the files of two such items are loaded by an editor of stage version 3
- **THEN** the check passes, the editor loads both, re-keyed under version 3 and written back, names them in the report, the item hash changes and the stamp's content hash does not, and a second load restages nothing; an editor of stage version 1 refuses the re-keyed files naming the newer stage

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
An item file SHALL be TOML with the keys `schema_version` (1; 2 for an item with a points list; 3 for an item with strokes), `id` (the canonical 26-character upper-case Crockford ULID, first character at most `7`), `body`, `surface`, `layer`, `priority` (integer), `kind`, `reach_m`, `max_delta_m`, `base_hash` (16 lower-case hex digits), `generator_version`, `stage_version`, the optional `name` and `notes`, the optional `group` (a canonical ULID: the line, region or brush group the item is a part of) and `part` (the integer index of the item in its group), the optional `mask_layer` (a layer name: the layer a region clips, WDEF-012; only on the kind `region`, with the same name rule as `layer`), and the tables `[anchor]` (`latitude`, `longitude`, `height`, `heading`) and `[params]` (the kind's parameters, finite floats by name), then, for an item with points, the array of tables `[[points]]` (ADR 0069: `latitude`, `longitude`, in order); a brush item (ADR 0070) has, instead of points, the array of tables `[[strokes]]` (`kind`, `radius_m`, `strength_m`, for a flatten `target_m`, then the stroke's own `[[strokes.points]]` tables with `latitude`, `longitude`, in order). The writer SHALL emit exactly that key order, one key per line, floats in shortest round-trip form with `-0.0` written `0.0`, and write only a valid item: finite numbers; the surface `sphere` (the only reference surface, CON-15; the direction of the anchor is its conversion of latitude and longitude at height 0); latitude in -90..90, longitude in (-180, 180] and 0 at a pole, so a place has one spelling, for the anchor and every point; heading in 0..360; reach in 0..one cell edge; `max_delta_m` in 0..100,000 m; layer, kind and parameter names of letters, digits, `_` and `-`; points only on a kind that takes them (the kinds `ridge`, `channel` and `region` need them: 2 to 64 for a line, 3 to 64 for a region), the first equal to the anchor's latitude and longitude, no two consecutive points equal; strokes only on the kind `brush` (1 to 64 strokes, 1 to 64 points each and 4,096 in all, no points beside them, no two consecutive points of a stroke equal, the first point of the first stroke equal to the anchor's latitude and longitude, `kind` one of `raise`, `lower`, `smooth` and `flatten`, `radius_m` and `strength_m` finite and above 0, `target_m` present on a flatten and only there; WDEF-013); `group` and `part` only together and only with points or strokes. The writer emits schema 1 for an item without points or strokes, so existing files and their hashes do not change, schema 2 for an item with points and schema 3 for an item with strokes. The index takes an item (`Item::put_into`) only when it is valid. The loader SHALL refuse a missing, non-integer or other `schema_version` (a newer one with the reason; 1 with points or strokes, 2 without points or with strokes, 3 without strokes), an unknown key at the top, in `[anchor]`, in a `[[points]]`, `[[strokes]]` or `[[strokes.points]]` table or as a table, a missing key (`[params]` may be missing: the item has no parameters), a value of the wrong type, a non-finite number, an integer that is not exact as a float, an id or group that is not the canonical ULID and what the writer refuses, each naming the key, and SHALL accept an integer literal for a float when its magnitude is at most 2^53 (a larger one is refused: write it with a decimal point) and read `-0.0` as `0.0`. The ULID of a new item is made by the command from an injected clock (48 bits of milliseconds) and injected entropy (80 bits), never by the generator. The content hash SHALL be BLAKE3-128 under the context `planet-builder item hash v1` over the parsed form: the schema version, the 16-byte id, the body, the surface, the layer, the priority, the kind, the four anchor numbers, the reach, the height bound, the parameters in name order, the base hash and the two versions, numbers as bit patterns with `-0.0` made `0.0` and strings length-prefixed, and not the name or the notes (layout, in this order, little-endian unless said: schema version `u32`; id 16 bytes big-endian; body `u32`; surface, layer as `u32` length and UTF-8 bytes; priority `i32`; kind as length and bytes; latitude, longitude, height, heading, reach, height bound as `u64` bit patterns; parameter count `u32`, then each name as length and bytes and its value bits; base hash `u64`; generator version `u32`; stage version `u32`; and, for schema 2 and 3, the group as a flag byte (1, then 16 bytes big-endian, or 0 and no bytes), the part as a flag byte (1, then `u32`, or 0), the mask layer as length and bytes (length 0 when absent), the point count `u32` and each point's latitude and longitude as `u64` bit patterns, and, for schema 3 only, after those, the stroke count `u32` and per stroke the kind code `u8` (0 raise, 1 lower, 2 smooth, 3 flatten), the radius, the strength and the target as `u64` bit patterns (target 0 bits when absent), the point count `u32` and each point's latitude and longitude as `u64` bit patterns; the digest is `blake3::derive_key(context, material)` truncated to its first 16 bytes and read as a big-endian `u128`), so formatting, key order, comments and merges never change it (WDEF-003). An item goes into the definition (`Item::put_into`) with its content hash, the unit direction of its anchor and its layer, priority, reach and height bound, and with its points or strokes as unit directions, so putting back an item restores the exact definition hash.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0069, ADR 0070, ADR 0007, WDEF-003

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
- **AND** a file with points and schema 1, without points and schema 2 (with strokes and schema 2 likewise), a group without a part, a first point that is not the anchor, a repeated consecutive point, 65 points or points on a kind that takes none is refused naming the key

#### Scenario: Strokes
- **WHEN** a brush item with three strokes (a raise of three points, a smooth of two, a flatten with a target) and a group and a part is written, read and written again, and the schema-1 and schema-2 items of the earlier scenarios are written
- **THEN** the brush is schema 3 with the keys in the fixed order, its `[[strokes]]` in order and each stroke's `[[strokes.points]]` after its keys, reading it gives the item and the same text, the earlier items are byte-identical to what the writer made before schema 3 existed, a stroke's kind, radius, strength, target or a point, and the order of two strokes, change the hash, and the hash equals BLAKE3-128 of the layout above computed independently in the test
- **AND** a file with strokes and schema 2, a flatten without a target, a raise with a target, 65 strokes, 65 points in a stroke, 4,097 points in all, strokes beside points, strokes on another kind, a radius or strength of 0 or less, an unknown stroke kind or an unknown key in a stroke is refused naming the key

### Requirement: AUTH-006 The item store's revisit trigger is counted and timed
The editor SHALL count the items of each body and time each load of a body's items, and show both in its diagnostics. A body holding more than 20,000 items, or a load over 2 s (ADR 0066 point 6), SHALL log one warning per crossing naming the count or the time, the threshold and "ADR 0066 point 6: revisit the backend". The count SHALL follow every addition and removal, and a fall back under the limit SHALL re-arm the warning. The time of the store's read alone SHALL come from an injected timer; it is a signal, and only loads on Rasierklinge decide, which the warning says.

Verify: A
Status: active
Source: ADR 0066 point 6, planner (2026-11-01, item-store-trigger)

#### Scenario: The count
- **WHEN** stores of 19,999, 20,000 and 20,001 items are loaded
- **THEN** only the last warns, once, naming 20,001 items and the limit 20,000

#### Scenario: The time
- **WHEN** a load is timed at 2,000 ms and at 2,100 ms by an injected timer
- **THEN** only the second warns, naming 2,100 ms and the limit 2,000 ms

#### Scenario: Adding past the limit
- **WHEN** the 20,001st item is put, then removed by undo, then put back by redo
- **THEN** the first crossing warns, the fall back clears the mark and the second crossing warns again

#### Scenario: The diagnostics
- **WHEN** the status bar of an editor with a store is drawn
- **THEN** it shows the largest body's count and the last load time with their limits, marked when one is over
