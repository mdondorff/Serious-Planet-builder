## MODIFIED Requirements

### Requirement: STRM-049 The world bakes the chunk horizon
A world with a chunk stage SHALL be able to enable the horizon jobs (`WorldTerrain::enable_horizon`, refused without the stage) with a baker that samples the world's own field over the chunk and a halo and bakes its horizon (GEN-044). `horizon_frame` SHALL run one frame of the jobs for the chunks the plan's tiles read, nearest first, and `horizon_map` SHALL return the landed map or none. The same world SHALL give the same map.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), STRM-048, STRM-026

Detail: A world with a chunk stage SHALL be able to enable the horizon jobs (`WorldTerrain::enable_horizon`, refused without the stage), with a baker that samples the world's own field (the base, the stamps and the changes of the chunks that are ready) at the cell directions of the chunk lattice over the chunk and a halo of `ceil(range / cell) + 2` cells (past a face edge on the extended cube plane), bakes the horizon of the chunk's cells (GEN-044) and reads the chunk and the chunks around it, nine in all and eight at a cube corner (STRM-048's state word), which holds only while the halo fits in one chunk edge: `enable_horizon` refuses a range whose halo is longer, and a march the bake would refuse, naming the cause (`chunk_horizon_params` gives the world's own, the range cut to a chunk edge less two cells). A world that is gone or a field that cannot be built gives a map of zeros of the right length, never a short one. `horizon_frame` SHALL run one frame of the jobs for the chunks the plan's tiles read, nearest first, and `horizon_map` SHALL return the landed map or none. The march's hash SHALL carry the lattice and the bake version; the edits enter through the chunk's edit word (STRM-057). The same world SHALL give the same map. The cell is the chunk lattice's (8 m at the A-net default).

#### Scenario: A flight
- **WHEN** a world with the stand-in chunk stage and a small lattice flies 12 s on a fake clock, then stands still, then gets an edit
- **THEN** every ready chunk has its map after its heights, the maps are one byte per azimuth per cell, the same world gives the same maps, the re-bakes stay within two per chunk, an unchanged world bakes nothing more, and an edit at the chunks bakes the maps it touches again

## ADDED Requirements

### Requirement: STRM-056 A stale chunk keeps drawing and is re-solved when read
An edit SHALL mark the ready chunks whose read set (GEN-081) it touches stale. A stale chunk SHALL keep drawing its old version (STRM-054) until the new version lands, and SHALL be re-solved only when the plan reads it: nearest to the camera first, ahead of prefetch (STRM-042), with at most two of the chunk workers on stale chunks. The new version SHALL land as a second landing (STRM-052, STRM-053). No frame waits for it (CON-22).

Verify: A
Status: active
Source: m8-chunk-version, STRM-040, STRM-042, STRM-054

#### Scenario: An edit over a drawn chunk
- **WHEN** a world with a stand-in chunk stage on a fake clock has a chunk ready, gets an edit inside its halo, and the plan keeps reading the chunk
- **THEN** the chunk draws its old version until the new one lands, one job runs for it, a chunk the plan does not read stays stale and unsolved, and at most two stale jobs run at once

### Requirement: STRM-057 A chunk's horizon is baked again only when an edit touches it
The state word of a chunk's horizon (STRM-048) SHALL carry, instead of the definition's revision, the hash of the items of every stamp layer that touch the chunk and the halo the march reads around it (`HorizonBaker::edit_word`), 0 for none. An edit that touches none of a chunk's items SHALL NOT bake its map again; one that does SHALL, once the neighbourhood has settled, at most once per chunk.

Verify: A
Status: active
Source: m8-chunk-version, STRM-048, STRM-049, GEN-082

#### Scenario: An edit far from the flight
- **WHEN** a world with the horizon jobs has every map landed and gets an edit outside every chunk's halo
- **THEN** no map is baked again; an edit at the chunks bakes at most one map per chunk it touches
