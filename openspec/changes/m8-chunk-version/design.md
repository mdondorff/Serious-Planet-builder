## Read set
- Region: the chunk's square plus `ANET_HALO_M` (GEN-049), a disc around its centre of the region's radius.
- Network: the segments `segments_reaching` keeps for the region (GEN-078) plus, for each, the segments upstream along the main stem up to `law.window` back (the bed's running minimum, `anchor_profiles`). Each segment contributes discs along its chord (every chord / 8) of radius `routing_reach_m(law, cell_m)` plus the half step.
- One function returns the discs; the version and the pruning take it. The pruning (#730) is the source of the reach; the function lives beside it in `anet.rs` and `segments_reaching` calls it (no change in what it keeps; tier A: equal sets on the existing network tests). The owner of the pruning (explorer) is asked for its reach before the code.
- A disc is a cap: the test against an item is `EditReach::touches_cap` (R3, strokes included).

## Version
- `version = blake3(sorted item hashes)` of the items whose reach touches a disc (`footprint_hash` of `FootprintItem`s, the tile rule). Only items of layers in the `Under` chain count.
- Disk key `ChunkSpec` gains the version; a chunk of another version is a different record. Superseded versions stay until the cache evicts them (owner question: drop at once).
- The chunk's change is stored as `z - z_base` where `z_base` is base plus the stamps of the version.

## Edit path
- `world.set_definition` computes the old and new item sets; the changed items (`item_changes_from`) give `(dir, reach_m, strokes)`.
- For each ready chunk the read set is tested against the changed items; a touch marks it stale with the new version. The chunk stays in `ready` with its old data and weight.
- The planner asks `wanted` as today; a wanted chunk whose version differs from the current one is a job (nearest first, ahead of prefetch, within 2 of the 3 workers); on landing it replaces the old record (second landing).
- A chunk that is not read is not re-solved; when it is read later it is asked at the current version.

## Risks
- The read set is large for a chunk on a river (many segments upstream): more chunks go stale; the cost is only for drawn ones.
- The version hashing cost per chunk per frame: computed once per chunk per definition revision and cached.
