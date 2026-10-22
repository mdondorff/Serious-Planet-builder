## ADDED Requirements

### Requirement: STRM-034 The plan cache keeps what an edit cannot reach
`SelectionCache::note_item_change(from, to, changes)` SHALL announce that the field's item revision went from `from` to `to` and where the items that differ were and are (`ItemChange`). The next plan SHALL keep, as checked at the new revision, the footprint hashes of tiles whose reads the changes cannot reach (the tile's cap with its halo, or the cover of a coarse tile, widened by the change's reach; never a tile that reads the whole body) and the certificates and cap bounds beyond eight radii of a change; the others SHALL be checked again. A revision that is not announced, or whose chain of announcements is broken, SHALL start the checks from all tiles. Plans SHALL equal a fresh cache's, node for node.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29: the plan after an edit took 70 to 75 % of a cold plan), STRM-027, STRM-026, ADR 0074

#### Scenario: The oracle
- **WHEN** ten random edits (put, change of place and size, removal; at the view, at a cube corner and a face edge, elsewhere) are announced one by one to two caches, each planned after the edit and compared with a fresh cache's plan
- **THEN** every plan equals the fresh one, and the persistent cache computes fewer footprint hashes than the fresh one in every plan

#### Scenario: Chains and gaps
- **WHEN** two edits are announced one after the other and planned once, then an edit nobody announced, then an announcement to a revision the field does not have
- **THEN** each plan equals the fresh one, and after the unannounced edit the tiles are checked again from all (at least half of a fresh cache's checks)

#### Scenario: The reach test
- **WHEN** tiles of levels 0 to 17 round random edits (including the largest reach, cube corners and face edges) and elsewhere have their reads hashed before and after the edit
- **THEN** every tile the test spares has the same hash before and after, and it spares at least a third of the tiles

#### Scenario: Just beyond the reach
- **WHEN** an item is put just beyond the reach the cache gives a tile (by 10 micro-radians to 0.02 radians, with reaches of 1 m to 40 km, in a random direction), for tiles of levels 0 to 17 at 60 places
- **THEN** the cache spares the tile and its read hash is the same as without the item
