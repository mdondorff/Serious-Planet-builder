## MODIFIED Requirements

### Requirement: WDEF-005 Incremental invalidation
Authored items SHALL be indexed by spatial cell, a `TileId` of the tangent-warp mapping at a level per body chosen so a cell is about 40 km wide (level 8 at Earth's radius, computed with `libm` from the body's radius), re-filed whenever the body's radius changes, and each item SHALL keep its unit direction so a move inside a cell is a change. The cell is the lookup index and no longer the invalidation unit: a generator SHALL read the items of a layer for a tile only through a footprint query (WDEF-007) that records the items whose reach touches the tile, so a read is never silently stale; a tile that touches more cells than a limit scans the items of the body for the same result. A cell query that records the hash of one cell, an empty cell included, remains for readers of a whole cell. A field that queries items SHALL say so (`reads_cells`) and report the read set of each tile (`inputs_hash_for`, given the mapping, which must be the tangent warp), so that adding, moving, editing or removing an item changes the keys of exactly the tiles its reach touches and no others, and editing a node no generator read changes no key. The definition hash SHALL cover the items, the base hash (ADR 0007) the nodes only, and a definition with items SHALL NOT be written to a TOML file that cannot carry them.

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
An authored item SHALL carry `reach_m` (the distance from its anchor beyond which it changes no output of any stage) and `max_delta_m` (a bound on the height change anywhere in its reach), both finite and at least 0, a layer name and an integer priority. An item whose `reach_m` is over one cell edge SHALL be refused, naming the item and the limit; a radius edit that would leave an item over the limit SHALL be refused the same way and change nothing. A footprint query (query kind 3) SHALL take the body, the layer, a tile of the tangent-warp mapping and a halo (a fraction of the tile edge, finite and in `[0, 1]`, `-0.0` read as `0.0`) and SHALL return the items of the layer whose anchor lies within the tile's bounding cap enlarged by the halo and the item's reach, tested in f64 with `libm` with a fixed relative margin, so a false positive can change a key and never an output and a false negative is a defect. Its entry in a read set SHALL be the parameters `body u32 | layer (u32 length + UTF-8) | tile raw id u64 | halo f64 bits` and the 128-bit hash of the result: the items in application order (priority, then id big-endian), each as a 16-byte id and a 128-bit content hash under `planet-builder item hash v1`, hashed under `planet-builder footprint result v1`; an empty result is that hash over empty input. An item whose reach touches no tile of a read SHALL leave that read's key unchanged when it is added, moved, edited or removed.

Verify: A
Status: planned
Source: ADR 0066 points 1 and 2, SPIKE-13

#### Scenario: Reach limits the re-key
- **WHEN** an item with a reach of 150 m is added at a place and the keys of every tile of levels 8 to 16 are compared before and after
- **THEN** exactly the tiles whose bounding cap enlarged by the halo is within 150 m of the anchor, up to the margin, change their key, and removing the item restores every key

#### Scenario: Reach over the limit
- **WHEN** an item is put with a reach over one cell edge, or a radius edit would leave one over it
- **THEN** the call is refused with the item id and the limit and the definition is unchanged

#### Scenario: Result material
- **WHEN** the same items are inserted in any order, or an item changes its direction, reach, bound, layer or priority
- **THEN** the result hash is equal for equal content in application order and changes with each of those fields, and an empty result is the pinned hash of empty input

#### Scenario: Declared against recorded reads
- **WHEN** a generator samples items while a recording context observes it
- **THEN** every item it touched is in the footprint result its tile declared, and its output is unchanged beyond `reach_m` of every item and within `max_delta_m`
