## ADDED Requirements

### Requirement: STRM-047 The change and its weight reach the draw
The compact tile builder SHALL store the chunk change of a tile at the tile's own level beside its heights (`HeightField::level_change`, forwarded by every wrapper that forwards `height_at_level`), and the plan's node (`TerrainDraw::chunk_weight`) SHALL carry the tile's fade weight, the minimum of the weights of the chunks the tile reads, so two tiles on either side of a chunk border rise together; the node record holds it as the fade `1 - w` (REND-039).

Verify: A
Status: active
Source: STRM-039 (the weight), STRM-038 (the chunks a tile reads), REND-039

#### Scenario: Change and weight
- **WHEN** a tile at the pyramid top over a ready chunk and a tile near its border with a chunk that landed later are built and planned
- **THEN** the stored change equals the field's height at the tile's level minus the base's, the node weights follow the fade of each chunk, and the border tile takes the smaller weight of the two chunks it reads

### Requirement: STRM-052 The fade clock of a landed chunk
The fade of a landed chunk's change (STRM-047) SHALL start when the change is first drawn: when the first exact tile that reads the chunk is drawn (its mesh is the one the plan expects, STRM-038), once no stale tile that reads it is still drawn from the mesh it had before the landing. A stale tile SHALL start nothing; until the clock starts the chunk SHALL weigh 0, so a rebuild slower than `chunk_fade_ms` still fades. Every tile of the chunk SHALL share that one clock. A chunk that lands again without having left RAM (a new version) SHALL keep its state, so the tiles under it neither drop to the base nor fade again.

Verify: A
Status: active
Source: STRM-047, STRM-038, M8 plan criterion 3

#### Scenario: A slow rebuild and a new version
- **WHEN** a chunk lands and its rebuilt tiles reach the renderer 2 s later (the fade is 1.5 s), and later the chunk and a neighbour land again
- **THEN** the tiles under the chunk weigh 0 until the rebuilt tiles arrive, 0.5 at 750 ms after, and 1 after 1.5 s, and the later landings leave the weights at 1

## MODIFIED Requirements

### Requirement: STRM-039 Chunk jobs never block a frame
Chunk jobs SHALL run on the injected spawner (CON-04), ordered by distance to the camera, at most `chunk_workers` at once, and no frame SHALL wait for one. A tile whose chunk is not ready SHALL draw the base; on landing, the chunk's change SHALL fade in per tile over `chunk_fade_ms` (default 1500, the owner, 2026-10-24) on the injected clock, through a plain f32 weight (CON-11; the weight is the CPU side, drawing with it is REND-039). The per-frame log SHALL count the plan's nodes that read chunk detail (at or finer than the pyramid's top level, column `chunk_nodes`) and those among them whose own chunk is not ready (column `chunk_stand_in`): the stand-in share, for the speed-envelope scene.

Verify: A, B
Status: active
Source: ADR 0077 point 7, CON-22, CON-04, CON-11, M8 plan criteria 3 and 7

#### Scenario: Flight without waiting
- **WHEN** a scripted flight from 30 km to 1.8 m and a 300 m pan run on the fake clock with a stand-in chunk cost of 0.25 s per km2 and 6 workers
- **THEN** no frame waits for a chunk, every chunk lands (the first within 10 s at the pan), and the log shows the stand-in share falling to zero

#### Scenario: Fade
- **WHEN** a chunk lands under visible tiles
- **THEN** each tile's weight rises linearly from 0 to 1 over `chunk_fade_ms` (default 1500), so the drawn height `base + w x change` steps by at most the weight step times the change per frame (a change of 17.55 m, the 256-bake figure, moves 0.19 m a frame at 60 frames per second; the shared store's largest entry step of 58.3 m moves 0.65 m a frame, which is 0.20 px on screen at the distance where it enters: design.md 6a of `m8-chunk-fade`)
