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

### Requirement: STRM-035 The reach of a change
A change at direction `d` with reach `r` can alter the reads of a tile when the angle from the tile's centre to `d` is within `reach(tile) + r / R` (`R` the definition's radius), widened as WDEF-007's touch test widens its limit (twice its margins) and by 1e-7 rad. `reach(tile)` is the cap of the tile with halo 1 for a fine tile; for a coarser one, its angular radius plus `(rings + 2) * 1.5` nominal cell edges (`summary::margin_rings` at halo 1), infinite when the margin needs more rings than the index allows. A tile so reached SHALL be taken as changed without its hash being made. 
Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29), STRM-034, LOD-012

#### Scenario: Just beyond the reach
- **WHEN** an item is put just beyond the reach the cache gives a tile (by 10 micro-radians to 0.02 radians, with reaches of 1 m to 40 km, in a random direction), for tiles of levels 0 to 17 at 60 places
- **THEN** the cache spares the tile and its read hash is the same as without the item

### Requirement: STRM-036 The window announces its edits
Every definition the editor hands to the generator (a put, a change, a delete, an undo, a redo, a load) SHALL be announced to the plan cache before the next plan, with the places of the items that differ from the definition it replaces (`WorldDefinition::item_changes_from`) and the field's item revisions before and after.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29), STRM-028, STRM-034

#### Scenario: Put, undo and redo
- **WHEN** a stamp is put, undone and redone in the editor, and each definition it hands over is compared with the one before
- **THEN** each gives the same one place (the stamp's), once as it was and once as it is, so the cache is told every change

### Requirement: STRM-037 The reach of a change on bounds
Certificates and cap bounds of a tile SHALL be dropped when the angle from its centre to a change is within 8 angular radii of the tile plus `r / R`, widened the same way. Certificates SHALL also be dropped when the set of levels that read height bounds (`reads_bounds`) differs, not when the field's largest height does. At most 32 announcements SHALL be kept.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29), STRM-034, LOD-012

#### Scenario: The oracle
- **WHEN** the random edits of STRM-034 are planned
- **THEN** every plan equals a fresh one, with the certificates and cap bounds kept beyond the reach of each change
