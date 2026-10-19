## MODIFIED Requirements

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

## ADDED Requirements

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
