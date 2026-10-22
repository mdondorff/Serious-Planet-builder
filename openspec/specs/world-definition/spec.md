# world-definition Specification

## Purpose
The WorldDefinition as the single source of truth for all world content (CON-05): a versioned file format with tested migrations (CON-10), canonical hashing, recorded reads so a generator's keys cover exactly what it read (CON-06, ADR 0006), and incremental invalidation when the definition changes (ADR 0007).

## Requirements

### Requirement: WDEF-001 Single source of truth
The WorldDefinition SHALL be the only source of world content; every runtime representation (tiles, meshes, textures, instances) SHALL be regenerable from it and from nothing else.

Verify: A
Status: planned
Source: CON-05

#### Scenario: Regenerate
- **WHEN** all cached tiles are deleted and regenerated from the definition
- **THEN** the regenerated tiles are bit-identical to the deleted ones

### Requirement: WDEF-002 Schema version and migrations
The WorldDefinition SHALL be stored as TOML with a top-level `schema_version` (2 since the liquid table and the sea of a body, WDEF-006: version 2 only adds keys, so the migration from version 1 is the identity), independent of the generator version. Loading SHALL require the version, reject a newer one and a gap in the migration chain with an error that names what to do, apply the migrations from the file's version to the current one in order, and fail with the step's message when a migration fails. Files of every older schema version in the fixture set SHALL load through tested migrations and equal the current-layout definition of the same content. Values SHALL be non-negative integers (a `u64` above `i64::MAX`, and only that, as a canonical decimal string) and finite floats, typed by their literal except for the paths the program reads (`world.seed` an integer, `body.<n>.radius_m`, `body.<n>.sea.level_m`, `body.<n>.sea.min_area_fraction` and every `liquid.<k>.*` node a float, `body.<n>.sea.liquid` an integer, where an exact integer literal is accepted for the float and a float for the integer is an error); the one string value is `world.height_field`, the implementation id of the height field (letters, digits, `_` and `-`), which every other path rejects as a string and which is rejected as a number; any other value, a negative integer, a key outside letters, digits, `_` and `-`, or a reserved name (`derived`, `schema_version`) SHALL be an error naming the path. Writing SHALL be canonical (version first, sorted tables, shortest round-trip floats) and reading it back SHALL give the same nodes and height-field id bit for bit.

Verify: A
Status: active
Source: CON-10

#### Scenario: Old fixture
- **WHEN** a fixture written by an older schema version is loaded
- **THEN** it migrates and equals the current-version fixture of the same content (the version 1 fixture through the identity step to version 2; the engine is also tested with a synthetic three-version chain)

#### Scenario: Unloadable file
- **WHEN** the version is missing, newer than the build, or has no migration path, or a value cannot be stored exactly
- **THEN** loading fails with an error that names the version or the path

#### Scenario: Height field id
- **WHEN** a file sets `[world] height_field = "cpu-terrain-v1"`, and when it sets the same path to a number or any other path to a string
- **THEN** the first loads, equals the definition built in code and round-trips; the others fail naming the path

### Requirement: WDEF-003 Canonical hashing
Every definition node SHALL have a content hash over a canonical byte encoding (path, value type and exact value bits) that does not depend on field order, platform or insertion history, and the definition hash SHALL be a function of the set of nodes only, taken from the parsed nodes of a TOML file and never from its bytes, so comments, key order, whitespace and table style never change it. The height-field id of the definition (`world.height_field`, GEN-018), when set, SHALL enter the base hash as one more entry under that path, and a definition that names none SHALL hash exactly as it did before the id existed.

Verify: A
Status: active
Source: ADR 0006

#### Scenario: Formatting
- **WHEN** the same definition is written with other comments, key order, spacing, table style or numeric spelling of an exact value (`6.371e6`, `6371000`)
- **THEN** it loads to the same nodes and the same hash

#### Scenario: Reordered fields
- **WHEN** the same definition is built with its entries inserted in a different order
- **THEN** the node hashes and the definition hash are equal

#### Scenario: Height field in the hash
- **WHEN** a definition names a height field, another names a different one, and a third names none
- **THEN** the three hashes differ, and the third equals the hash of the same nodes without the id

### Requirement: WDEF-004 Read recording
Generators SHALL read definition inputs only through a context that records every read with the hash of the node read; the hash of the recorded read set SHALL be what enters cache keys in place of the seed, a body's id SHALL be part of that read set, and spatial queries record the hash of each cell read (WDEF-005), and a missing node SHALL be an error naming it. A field built directly from a seed (`Reference(seed)`, the single planet's seed) SHALL key exactly like one built through a context over a definition with that world seed for `BodyId(0)`; other bodies key by their derived seed and id, so their fields come from the generator.

Verify: A
Status: active
Source: CON-06, ADR 0006

#### Scenario: Unrecorded read
- **WHEN** a generator is given a definition whose unread node changes
- **THEN** its output and key are unchanged, and when a read node changes both change

#### Scenario: Bodies
- **WHEN** the same generator reads two bodies of one definition
- **THEN** their keys differ

### Requirement: WDEF-005 Incremental invalidation
Authored items SHALL be indexed by spatial cell, a `TileId` of the tangent-warp mapping at a level per body chosen so a cell is about 40 km wide (level 8 at Earth's radius, computed with `libm` from the body's radius), re-filed whenever the body's radius changes, and each item SHALL keep its unit direction so a move inside a cell is a change. The cell is the lookup index and no longer the invalidation unit: a generator SHALL read the items of a layer for a tile only through a footprint query (WDEF-007) that records the items whose reach touches the tile, so a read is never silently stale; a tile coarser than the cell level, or one that needs more than 16 rings of candidate cells, scans the items of the body, and the result is the same set whichever way the candidates are found (a tile three or more levels above the cell level records region summaries instead of the result, WDEF-008). A cell query that records the hash of one cell, an empty cell included, remains for readers of a whole cell. A field that queries items SHALL say so (`reads_cells`) and report the read set of each tile (`inputs_hash_for`, given the mapping, which must be the tangent warp), so that adding, moving, editing or removing an item changes the keys of exactly the tiles its reach touches and no others, and editing a node no generator read changes no key. The definition hash SHALL cover the items, the base hash (ADR 0007) the nodes only, and a definition with items SHALL NOT be written to a TOML file that cannot carry them.

Verify: A
Status: active
Source: report §5, ADR 0006, ADR 0007, ADR 0066

#### Scenario: Local edit
- **WHEN** an item in one cell is added, moved, edited or removed
- **THEN** only tiles whose footprint query returns the old or the new item (the tile's cap, its halo and the item's reach) change height and mesh key, and restoring the item restores every key exactly

#### Scenario: Empty cell
- **WHEN** a generator queried a cell or a tile footprint and found nothing, and an item is then added there
- **THEN** its read set and so the keys of the tiles that queried it change; an item out of every reach changes nothing

#### Scenario: Radius edit
- **WHEN** a body's radius changes across a cell-level boundary
- **THEN** its items follow to the cells of the new level and are still returned by the queries that cover them

#### Scenario: Hashes
- **WHEN** items are inserted in any order, or all removed
- **THEN** the definition hash is equal for equal content and equals the node hash when there are no items; the base hash never changes with items

### Requirement: WDEF-006 Liquid kinds and the sea of a body
Liquid kinds SHALL be a **parameter table in the world definition**, not a code enum (the owner's M5 constraint 3, ADR 0041 point 3): a row is a kind id `<k>` and nine float nodes `liquid.<k>.colour_r|colour_g|colour_b` (the liquid's colour in deep water, linear RGB, each in [0, 1]), `emission_r|emission_g|emission_b` (emitted radiance in units of the scene's sun illuminance, linear RGB, each at least 0; water 0, lava above), `viscosity_pa_s` (above 0), `wave_amplitude_m` (at least 0; 0 means no waves) and `wave_length_m` (above 0 when there are waves); a row with a missing, non-numeric, non-finite or meaningless node SHALL be an error naming it. A world that defines no `liquid.*` node SHALL have the Earth table, one liquid with id 0, water; a world that defines any SHALL have exactly the rows it defines (nothing merged), so a definition that never mentions liquids keeps the hash it had and a definition with a table is a different definition (WDEF-003). The **sea** of a body SHALL be three optional nodes with Earth defaults: `body.<n>.sea.level_m` (the water level W of the sea, a finite float, default 0), `body.<n>.sea.liquid` (the kind id, an integer up to `u32::MAX`, default 0) and `body.<n>.sea.min_area_fraction` (the area threshold of the ocean connectivity as a fraction of the body's surface, from 0 to 1, default `EARTH_OCEAN_MIN_AREA_FRACTION`, 1e6 km^2 of an Earth surface); a wrong type, a non-finite level or a fraction outside [0, 1] SHALL be an error naming the node. A generator reads the table and the sea through the recording context (`GenContext::liquids`, `GenContext::sea`), which records the nodes the definition sets (none when it sets none), so adding or changing a node changes the read set of exactly the generators that read it. Class of the attributes (ADR 0035): colour, emission, wave amplitude and wavelength are **presentation parameters held as definition nodes** (nothing in a generator or a CPU system reads them; bound: exact, the validated ranges above); viscosity is world data that a field may read later; the sea's level, liquid and threshold are field inputs (GEN-022, GEN-023). Because the nodes are in the definition, editing a liquid's colour changes the definition hash (CON-09 cost, ADR 0051 point 1); the look file (ADR 0042 point 6) overrides how a liquid is presented for display only, and where it sets a value it wins over the definition's. The sea's liquid SHALL name a row of the table, and a body whose sea names none SHALL be refused when its bake is opened, naming the node.

Verify: A
Status: active
Source: owner's M5 constraint 3, ADR 0041 points 1 to 3, ADR 0006, ADR 0035, CON-05, CON-10

#### Scenario: Defaults
- **WHEN** a definition sets no liquid and no sea
- **THEN** its table is the Earth table (water), its sea is level 0 of liquid 0 at the Earth threshold, its hash is the hash it had before the table existed, and a generator that reads them records nothing

#### Scenario: A table
- **WHEN** a world defines liquids 0 and 7 (water and lava) and a sea of kind 7
- **THEN** its table has exactly those rows, every field of every row is in the hash, the sea names kind 7, another body has the Earth sea, and a read of the table records its nodes

#### Scenario: A dangling kind
- **WHEN** a body's sea names liquid 5 and the table has no such row
- **THEN** opening its bake is refused naming `body.0.sea.liquid` and the kind

#### Scenario: Bad rows
- **WHEN** a row has a colour above 1, a negative emission, a zero viscosity, waves without a wavelength, a missing node or an integer where a float is read
- **THEN** it is refused naming the liquid and the field, and nothing is written by a refused row

#### Scenario: The file
- **WHEN** a hand-written file defines two liquids and a lava sea with integer literals for float nodes
- **THEN** it loads to the definition built in code and writes back canonically

#### Scenario: What the bake reads of the sea (ADR 0051 point 1)
- **WHEN** the bake reads the sea nodes
- **THEN** it reads them from the definition directly, outside the recording context, and the area threshold reaches the ocean block key as an output parameter (STRM-023)

### Requirement: WDEF-009 128-bit item ids and content hashes
The id of an authored item (`ItemId`) and its content hash (`ItemHash`) SHALL be 128 bits (`u128`; ADR 0066 point 6, ADR 0023 point 2), and every encoding of an id SHALL be 16 bytes big-endian, so the byte order is the numeric order (the application order of equal priorities, the order of `item_ids`, the order of a footprint result). A hash that was defined over 64-bit ids and content hashes SHALL be unchanged for values below 2^64: the definition hash, the cell hash and the footprint hash (64-bit words) write the low words where the 64-bit value stood and the high words of the rows that have any, behind a marker and a count, after the rows (the header fixes the rows' number and width, so the tail cannot be taken for rows; no tail when every value is below 2^64); the cache's query-result hash (bytes) has no header, so it writes 40-byte rows of the low 8 bytes and, when a row is wide, a tail read from the end (the wide rows' index and high words, the count of wide rows, the count of rows and a marker byte: `20 n + 9` bytes, never a multiple of 40); the item hash of the footprint writes the content hash as 8 bytes when it is below 2^64 and as 16 bytes otherwise. A value above 2^64 SHALL reach every hash it reaches below it, and two different sets of items SHALL NOT hash alike because of the tail. The ULID text of an id and the item file are AUTH-005.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0023 point 2, ADR 0018

#### Scenario: Wide values everywhere
- **WHEN** items with the largest id and content hash are put into a definition, found in a cell and in a footprint, and removed
- **THEN** they work as any item, the ids order numerically, and the definition hash is a function of the set of items whatever the insertion order

#### Scenario: Narrow values keep their hashes
- **WHEN** the pinned known answers of the footprint and summary hashes and a query result of ids and hashes below 2^64 (the old encoding written out in the test) are computed
- **THEN** they equal what a 64-bit build gave

#### Scenario: Wide never aliases narrow
- **WHEN** an id or a content hash differs from another only in its high 64 bits, in any row of a result
- **THEN** the definition, cell, footprint and query-result hashes all differ, and which row is wide matters

### Requirement: WDEF-007 Item reach and the footprint query
An authored item SHALL carry `reach_m` (the distance from its anchor beyond which it changes no output of any stage) and `max_delta_m` (a bound on the height change anywhere in its reach), both finite and at least 0, a layer name and an integer priority. An item whose `reach_m` is over one nominal cell edge (40 km, independent of the body's radius, so a radius edit cannot leave an item over it) or whose numbers are not finite SHALL be refused, naming the item and the limit, and the definition SHALL be unchanged. A footprint query (query kind 3) SHALL take the body, the layer, a tile of the tangent-warp mapping and a halo (a fraction of the tile edge, finite and in `[0, 1]`, `-0.0` read as `0.0`) and SHALL return the items of the layer whose anchor lies within the tile's bounding cap enlarged by the halo and the item's reach, tested in f64 with `libm` with a fixed relative margin, so a false positive can change a key and never an output and a false negative is a defect. For a tile that records region summaries (WDEF-008) the footprint result is what the generator receives and the read carries kind 4 entries instead; the rest of this requirement is about the kind 3 entry. Its entry in a read set SHALL be the parameters `body u32 | layer (u32 length + UTF-8) | tile raw id u64 | halo f64 bits` and the 128-bit hash of the result: the items in application order (priority, then id big-endian), each as a 16-byte id and a 128-bit item hash under `planet-builder footprint item v1` (the item file's own content hash keeps `planet-builder item hash v1`, ADR 0066 point 6), hashed under `planet-builder footprint result v1`; an empty result is that hash over empty input. An item whose reach touches no tile of a read SHALL leave that read's key unchanged when it is added, moved, edited or removed (a tile that records region summaries is re-keyed by any item anchored in the cells it reads, a false positive). An item with a brush stamp that holds strokes SHALL be returned only when the reach test passes and some stroke reaches the cap: the angle between the cap's centre and the stroke's polyline (its point for a dab) is at most the stroke's radius over the body's radius plus the cap's angle, with the same margins (a stroke changes no output beyond its radius, GEN-030, and a distance to a polyline moves by at most the distance moved); the item's reach is a disc round its first point that holds the whole stroke and is several times the stroke's own area, so a long stroke re-keys only the tiles along it.

Verify: A
Status: active
Source: ADR 0066 points 1 and 2, SPIKE-13

#### Scenario: A brush stroke re-keys the tiles along it
- **WHEN** a brush item of one 20-point stroke 712 m long (radius 150 m, and the same at 1 km) is added and the footprint query is run for the tiles round it at levels where the reach disc holds at least 100 tiles
- **THEN** every tile with a sample within the stroke's radius of the polyline returns the item, and at least a third fewer tiles return it than the reach disc round the first point holds

#### Scenario: Reach limits the re-key
- **WHEN** an item with a reach of 150 m is added at a place and the keys of every tile of levels 8 to 16 are compared before and after
- **THEN** exactly the tiles whose bounding cap enlarged by the halo is within 150 m of the anchor, up to the margin, change their key, and removing the item restores every key

#### Scenario: Reach over the limit
- **WHEN** an item is put with a reach over one cell edge, a negative or non-finite reach or bound, or an empty layer name
- **THEN** the call is refused with the item id and the limit and the definition is unchanged, an item of the same id staying where it was

#### Scenario: Result material
- **WHEN** the same items are inserted in any order, or an item changes its direction, reach, bound, layer or priority
- **THEN** the result hash is equal for equal content in application order and changes with each of those fields, and an empty result is the pinned hash of empty input

#### Scenario: Tile alone
- **WHEN** a stage that reads items generates a tile alone, and the same samples are cut from its parent's grid or generated after its neighbours
- **THEN** the samples are bit-identical, whatever the order of generation

#### Scenario: Declared against recorded reads
- **WHEN** a generator samples items while a recording context observes it
- **THEN** every item it touched is in the footprint result its tile declared, and its output is unchanged beyond `reach_m` of every item and within `max_delta_m`

### Requirement: WDEF-008 Region summaries
A tile that is three or more levels above the cell level (it covers 64 cells or more) SHALL record its item read as region summary entries (query kind 4) instead of the footprint result: the smallest set of nodes of a Merkle quadtree over the cell grid that covers exactly its own cells and the margin ring of cells around them, one entry per node with the parameters `body u32 | layer (u32 length + UTF-8) | level u8 | node raw id u64` and the node hash as the result. A leaf SHALL be a cell at the body's cell level, hashed under `planet-builder cell hash v1` over its item rows in application order (each a 16-byte id and the 128-bit footprint item hash); an inner node SHALL be hashed under `planet-builder summary node v1` over its level and its four child hashes in child order; an empty subtree SHALL be the same function over empty children, so its hash depends on its level. The tree SHALL be kept per body and layer and an edit SHALL rewrite one node per level; the margin ring SHALL be wide enough that every item whose reach can touch the tile has its anchor in a covered cell: `ceil((cap arc - the arc from the tile centre to its nearest edge great circle) x radius + 40 km) / narrowest cell edge) + 1` rings, the narrowest cell edge being half the nominal width of a cell, and a tile that needs more than 64 rings SHALL read the six face roots. The items a generator receives for the tile SHALL be the footprint result. The halo of a field's footprint query is fixed by the field implementation and its version, so two reads of one tile with equal nodes never receive different items under an equal key. The node set of a tile is a pure function of the tile, the halo and the body's radius, so it SHALL be memoised across reads (a process-wide memo that starts over past a fixed size) without changing any result or key.

Verify: A
Status: active
Source: ADR 0066 point 3, SPIKE-13

#### Scenario: Edit under a coarse tile
- **WHEN** an item is added, moved, edited or removed inside the covered cells of a coarse tile
- **THEN** the tile's keys change, and an edit beyond the margin ring changes none

#### Scenario: One node per level
- **WHEN** an item is put or removed
- **THEN** exactly one node per level above its cell is rewritten, and every node equals the one a rebuild from the items gives

#### Scenario: Cover memo
- **WHEN** the node set of one tile, halo and radius is read twice, with a negative zero for a zero halo, and by several threads at once
- **THEN** the reads share one memoised set that equals the uncached one, and another halo, radius or tile is another set

#### Scenario: Node set
- **WHEN** the nodes of a tile's read are listed
- **THEN** they cover exactly the tile's cells and the margin ring's cells, with no overlap, and an empty subtree hashes differently at each level

### Requirement: WDEF-010 Stamp items
An authored item SHALL carry a kind and its parameters; the kinds are none, `hill`, `crater` and `plateau` (GEN-026), with `height_m` for a hill (signed), `depth_m` for a crater (at least 0) and `target_m` for a plateau (an absolute height) in an item file, one parameter in the stamp and two reserved at 0. A stamp SHALL be refused, naming the item and the reason, when `reach_m` is not above 0, a parameter is not finite, the hill's height or the crater's depth is above its `max_delta_m` in absolute value, or a crater's depth is negative. The content hash of a stamp SHALL be derived from its kind and parameters (BLAKE3-128 under `planet-builder stamp v1`; a given hash that disagrees is refused), so the kind and the parameters are part of every hash that covers the content: the item hash (WDEF-007), the footprint result, the region summary leaves (WDEF-008) and the definition hash; no hash format changes, and an item with no stamp hashes as it did before kinds existed. The footprint query SHALL return the stamp with each item, in the cap form (`footprint_items_cap`) as well as the tile form. An item file's `kind` and that one parameter SHALL map to the stamp (the stamp's reach is the file's `reach_m`, its bound the file's `max_delta_m`), the map SHALL round-trip, a stamp kind without its parameter SHALL be refused by the file's validation naming the key, other keys SHALL be ignored (they are in no hash), and for a stamp kind the item's content hash SHALL be the stamp's (the name, the notes and the recorded versions re-key nothing); a kind that is none of the three is a plain item that keeps the file hash.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0067, M6 plan row E2

#### Scenario: Refusal
- **WHEN** a stamp is put with no reach, an amount above its bound, a negative crater depth or a non-finite parameter
- **THEN** the call is refused naming the item and the reason, and the definition is unchanged

#### Scenario: Hash
- **WHEN** the kind or a parameter of a stamp changes, and when an item of kind none is hashed
- **THEN** the content hash, the item hash, the footprint result, the summary leaf and the definition hash change in the first case, and the pinned known answers of the footprint and summary tests are unchanged in the second

#### Scenario: One source of truth
- **WHEN** a stamp is put with a content hash that is not the one its kind and parameters give
- **THEN** it is refused

#### Scenario: Item file
- **WHEN** a hill, a crater and a plateau item are written as files with their parameter, read back and put into a definition
- **THEN** each stamp equals the one it was made from, its content hash is the stamp's and stable, a stamp kind without its parameter is refused naming the key, and a kind that is none of the three keeps the file hash

### Requirement: WDEF-011 Line items
The stamp kinds SHALL include `ridge` and `channel` (ADR 0069): an open polyline of 2 to 64 points (`[[points]]` of the item file, AUTH-005), the first of which is the item's anchor, with the parameters `amount_m` (the crest height of a ridge, the bed depth of a channel; finite, at least 0, at most `max_delta_m`) and `width_m` (the half-width of the line; finite, at least 1). The item's `reach_m` SHALL be at least `width_m` plus the largest distance, on the body's sphere, of a point from the anchor, and stays within one nominal cell edge (WDEF-007), so the polyline and its width fit inside the reach. The stamp of a line item SHALL hold `[amount_m, width_m, 0]` and the points as unit directions, and its content hash SHALL be derived from the kind, the three numbers and, for a stamp with points, their count (`u32`) and bit patterns in order under `planet-builder stamp v1` (WDEF-010), so a change of a point, the width or the amount re-keys the tiles the line touches; a stamp without points hashes as before. An item file's `kind` and its two `[params]` keys SHALL map to the stamp and back, round-trip, and a line kind without either key SHALL be refused naming the key. A put of a line item SHALL be refused, naming the item and the reason, when a number is not finite or negative, the amount is above `max_delta_m`, the width is under 1 m, there are fewer than 2 or more than 64 points, two consecutive points coincide, a point lies farther from the anchor than `reach_m - width_m`, a point is not a unit direction, or the first point is not the anchor (to 1e-9 rad); the definition is then unchanged. The footprint of a tile (WDEF-007) SHALL return the line item with its points, shared and immutable. The `group` and `part` of an item file (a long line split into linked items by the command layer) SHALL not enter the stamp: the world definition sees independent items.

Verify: A
Status: active
Source: ADR 0069, M6 plan row E5, owner (2026-10-19), WDEF-010

#### Scenario: Refusal
- **WHEN** a ridge is put with a negative amount, an amount over `max_delta_m`, a width under 1 m, one point, 65 points, a repeated consecutive point, a point beyond `reach_m - width_m` of the anchor, or a non-finite number
- **THEN** the call is refused naming the item and the reason and the definition is unchanged

#### Scenario: Item file
- **WHEN** a ridge and a channel are written as files, read back and put
- **THEN** each stamp equals the one it was made from, the anchor is the first point, a missing `width_m` is refused naming the key, and the content hash is the stamp's

#### Scenario: Re-key
- **WHEN** a point, the width or the amount of a line changes, and when only its `group` or `part` changes
- **THEN** the footprint hash of a tile under the line changes in the first case and that of a tile out of reach does not, and the second case changes no hash

### Requirement: WDEF-012 Regions and surface types
The world definition SHALL hold a surface-type table (ADR 0069), a table of data and not a code enum, with the entries `rock`, `sand`, `grass`, `snow`, `forest`, `ice`, `desert`, `wetland` and `urban` in this order, each with its name and a debug tint (an sRGB colour). The item kind `region` SHALL be a closed polygon of 3 to 64 points (`[[points]]`, AUTH-005; the polygon closes from the last point to the first, which is the anchor) with the parameters `surface_type` (a whole number, the index of an entry of the table) and `mask_mode` (a whole number: 0 none, 2 cut inside: the edits of the layer are removed inside the outline and stay outside it), and the item's `mask_layer` (the name of the layer whose edits the region clips; required for mode 2, refused for mode 0). The mode 1, clip outside, is not available yet: it is refused naming the reason (a region that clips everything outside it must be read by every tile of its layer, which the footprint does not do; see `openspec/changes/region-mode1/design.md`). A region changes no height itself. The item's `reach_m` SHALL cover every point and stay within one nominal cell edge (WDEF-007), so a polygon larger than one reach is refused naming the limit. A put of a region SHALL be refused, naming the item and the reason, when an index is outside the table, a parameter is not a whole number, the mode is not 0 or 2 (mode 1 with the reason above), `mask_layer` is missing for mode 2 or given for mode 0, the polygon has fewer than 3 or more than 64 points, two consecutive points coincide (the last and the first included), the polygon crosses itself or has no area, a point lies beyond `reach_m` of the anchor, a point is not a unit direction, or the first point is not the anchor; the definition is then unchanged. A point is inside a region when it lies inside or on the polygon by the even-odd rule on the gnomonic projection of the points onto the tangent plane at the anchor (f64), a point behind that plane being outside. A region of mode 2 SHALL be filed in the layer `mask_layer` names, so the footprint of a tile of that layer returns it with its points when its cap meets the reach, and its content hash SHALL be derived from the kind, the parameters and the points under `planet-builder stamp v1` (WDEF-010), so a change of a point, the surface type or the mode re-keys the tiles it touches. A region of mode 0 stays in the item's own layer and changes no output.

Verify: A
Status: active
Source: ADR 0069, M6 plan row E5, owner (2026-10-19), WDEF-010

#### Scenario: The table
- **WHEN** the table is read
- **THEN** it has the nine entries in the order above, each with a tint, and an index of 9 or more is refused

#### Scenario: Refusal
- **WHEN** a region is put with an index outside the table, a mode of 3, a mode 1, a mode 2 without `mask_layer`, a mode 0 with one, two points, a self-crossing polygon or a point beyond its reach
- **THEN** the call is refused naming the item and the reason (mode 1 naming the missing footprint read) and the definition is unchanged

#### Scenario: Inside
- **WHEN** points inside, outside, on an edge and at a vertex of a concave polygon are tested, near the pole and across the antimeridian included
- **THEN** inside and on-edge points are inside, the others outside, and the answer does not depend on the order or starting point of the polygon's points

#### Scenario: Footprint
- **WHEN** the footprint of a tile that meets a mode-2 region of its layer and of one that does not is asked, and of a tile of another layer
- **THEN** the first returns the region with its points, the second and the third do not

#### Scenario: Re-key
- **WHEN** a point, the surface type, the mode or the mask layer of a region changes
- **THEN** the footprint hash of a tile it clips changes and that of an unaffected tile does not

### Requirement: WDEF-013 Brush items
The stamp kinds SHALL include `brush` (ADR 0070): an ordered list of 1 to 64 strokes (`[[strokes]]` of the item file, AUTH-005), each with a kind (`raise`, `lower`, `smooth` or `flatten`), `radius_m` (finite, at least 1, at most one nominal cell edge), `strength_m` (finite, above 0: the largest height change of the stroke at its centre line), for a flatten `target_m` (finite: the height it levels towards, metres above the reference surface) and 1 to 64 points of its own (4,096 in all), no two consecutive points of a stroke equal. The item has no `[params]` and no `[[points]]`; its anchor is the first point of the first stroke. The item's `reach_m` SHALL be at least the largest `radius_m` plus the largest distance, on the body's sphere, of a point from the anchor, and stay within one nominal cell edge (WDEF-007). The item's `max_delta_m` SHALL be at least the sum of the `strength_m` of its strokes and at most 100,000 m. The stamp of a brush item SHALL hold the parameters `[0, 0, 0]` and the strokes (kind code `u8`: 0 raise, 1 lower, 2 smooth, 3 flatten; radius, strength and target as bit patterns with `-0.0` made `0.0`, the target 0 bits when absent; the points as unit directions), shared and immutable, and its content hash SHALL be derived under `planet-builder stamp v1` (WDEF-010) from the kind, the three numbers and then, a brush having no points, directly the stroke count (`u32`) and per stroke the kind code, the three numbers as bits, the point count (`u32`) and the point bit patterns, so a change of a point, a radius, a strength, a target, a kind or the order of the strokes re-keys the tiles the item touches; a stamp without strokes hashes as before. A put of a brush item SHALL be refused, naming the item, the stroke and the reason, when a number is not finite, a radius is under 1 m or above a cell edge, a strength is not above 0, a flatten has no target or another kind has one, there are no strokes, more than 64 strokes, a stroke without points or with more than 64, more than 4,096 points in all, two consecutive points of a stroke coincide, a point lies farther from the anchor than `reach_m` less the largest radius, a point is not a unit direction, the first point is not the anchor (to 1e-9 rad), or `max_delta_m` is under the sum of the strengths; the definition is then unchanged. The footprint of a tile (WDEF-007) SHALL return the brush item with its strokes, shared and immutable. An item file's `kind` `brush` and its strokes SHALL map to the stamp and back and round-trip.

Verify: A
Status: active
Source: ADR 0070, M6 plan row E6, WDEF-010, WDEF-011

#### Scenario: Refusal
- **WHEN** a brush is put with a radius of 0.5 m, a strength of 0, a flatten without a target, a raise with a target, 65 strokes, a stroke of 65 points, 4,097 points in all, a repeated consecutive point, a point beyond `reach_m` less the largest radius, a `max_delta_m` under the sum of the strengths, or a non-finite number
- **THEN** the call is refused naming the item, the stroke and the reason and the definition is unchanged

#### Scenario: The stamp
- **WHEN** a brush item is put and its footprint is read, and each stroke field, a point and the order of two strokes change in turn
- **THEN** the footprint returns the item with its strokes, shared, the stamp's content hash equals the hash of the layout above computed independently in the test, every change gives another hash, and a ridge or a hill hashes as it did before

#### Scenario: Item file
- **WHEN** a brush item is converted to a file and back
- **THEN** the strokes round-trip bit for bit and the definition hash after putting it back is the one before
