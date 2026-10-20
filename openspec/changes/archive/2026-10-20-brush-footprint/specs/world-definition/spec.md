## MODIFIED Requirements

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
