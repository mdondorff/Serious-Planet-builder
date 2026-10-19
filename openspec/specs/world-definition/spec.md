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
An authored item SHALL carry `reach_m` (the distance from its anchor beyond which it changes no output of any stage) and `max_delta_m` (a bound on the height change anywhere in its reach), both finite and at least 0, a layer name and an integer priority. An item whose `reach_m` is over one nominal cell edge (40 km, independent of the body's radius, so a radius edit cannot leave an item over it) or whose numbers are not finite SHALL be refused, naming the item and the limit, and the definition SHALL be unchanged. A footprint query (query kind 3) SHALL take the body, the layer, a tile of the tangent-warp mapping and a halo (a fraction of the tile edge, finite and in `[0, 1]`, `-0.0` read as `0.0`) and SHALL return the items of the layer whose anchor lies within the tile's bounding cap enlarged by the halo and the item's reach, tested in f64 with `libm` with a fixed relative margin, so a false positive can change a key and never an output and a false negative is a defect. For a tile that records region summaries (WDEF-008) the footprint result is what the generator receives and the read carries kind 4 entries instead; the rest of this requirement is about the kind 3 entry. Its entry in a read set SHALL be the parameters `body u32 | layer (u32 length + UTF-8) | tile raw id u64 | halo f64 bits` and the 128-bit hash of the result: the items in application order (priority, then id big-endian), each as a 16-byte id and a 128-bit item hash under `planet-builder footprint item v1` (the item file's own content hash keeps `planet-builder item hash v1`, ADR 0066 point 6), hashed under `planet-builder footprint result v1`; an empty result is that hash over empty input. An item whose reach touches no tile of a read SHALL leave that read's key unchanged when it is added, moved, edited or removed (a tile that records region summaries is re-keyed by any item anchored in the cells it reads, a false positive).

Verify: A
Status: active
Source: ADR 0066 points 1 and 2, SPIKE-13

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
