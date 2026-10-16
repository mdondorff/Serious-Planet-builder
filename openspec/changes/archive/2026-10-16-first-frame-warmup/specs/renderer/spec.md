## ADDED Requirements

### Requirement: REND-017 No one-time initialisation inside a measured frame
The terrain renderer SHALL be able to do the one-time work of its first frames before the first frame (`warm_up`): create the small vertex pool and the first block of the main pool (block 0 of the vertex format, or atlas 0 with its bind group), grow the per-frame ring to a count above the node count of the standard scenes' plans (2048 nodes, within the share's node-record bytes), and submit one throwaway frame that draws a zeroed tile through every pipeline of the tile format in the active draw mode, polled to completion, so that the lazy first-use work of the driver and of wgpu (pipeline state, resource initialisation) is done outside any measured frame. The throwaway frame SHALL make no tile resident, SHALL NOT count as a frame (the frame clock of the slot retirement and the frames in flight do not move) and SHALL NOT change any image. The bytes it allocates SHALL equal the pure model of the pool configuration with one block and the vertex pool live (STRM-006); every other block, atlas and ring growth stays on demand and the share stays a ceiling. The renderer SHALL count the long-lived GPU objects it creates (pipelines, pool allocations, ring builds); after `warm_up`, a script whose first frames fit the warmed block and ring SHALL create none in frames 0 and 1. The pipelined loop and the streaming perf scenes SHALL warm the renderer in their setup, outside the timed loop; frame 0 stays in the measured frames (ADR 0032, TEST-018) and records like the frames after it.

Verify: C
Status: active
Source: ADR 0032, REND-013, REND-016, TEST-018, STRM-006, perf window 1 (F4)

#### Scenario: Nothing is created in the first frames
- **WHEN** a vertex-format and a compact renderer, per-draw and indirect, are warmed and then upload a plan's tiles and submit frames 0, 1 and 2
- **THEN** the pipeline, pool-allocation and ring-build counters and the live blocks are the warm-up's, none became resident during the warm-up, and a renderer that was not warmed allocates in the same frames

#### Scenario: Allocation equals the model
- **WHEN** a renderer is warmed
- **THEN** one block and the vertex pool are live, and `pool_allocated_bytes` equals `allocated_bytes_with(1, true)` with the node records at their allocated size, under the share

#### Scenario: Same image
- **WHEN** the same plan is rendered by a warmed and by a cold renderer
- **THEN** the pixels are identical

## MODIFIED Requirements

### Requirement: REND-013 Persistent terrain renderer
The terrain renderer SHALL create its pipeline, uniform buffers and render targets once, keep tiles in pooled fixed-size slots of a few large vertex buffers (blocks allocated on first use or by the warm-up of REND-017, a small pool for levels 0-2, no buffer per tile), upload each tile's vertices once per mesh key into a slot (a mesh with a different key takes a new slot and the old one is released), draw a frame by writing only the per-frame uniforms and recording the draws, report CPU and wall time per frame and GPU time from timestamp queries when the device supports them, and produce exactly the image of the one-shot executor. Where a tile sits (the renderer's own free list or a placement chosen by the residency manager) SHALL NOT change the image. The bytes it allocates SHALL equal the pure model of the pool configuration (STRM-006) and a full pool SHALL refuse an upload with the tile id.

Verify: C
Status: active
Source: CON-20, CON-21, ADR 0013, ADR 0033 point 3

#### Scenario: Reuse
- **WHEN** the same tiles are drawn in several frames and views
- **THEN** no vertex data is uploaded after the first upload and every frame equals the one-shot result

#### Scenario: Missing tile
- **WHEN** a plan names a tile that was not uploaded
- **THEN** rendering fails with the tile id

#### Scenario: Changed mesh
- **WHEN** a mesh with the same tile id but a different key (other seed) is uploaded
- **THEN** the resident buffer is replaced and the next frame equals the one-shot render of the new mesh

#### Scenario: Stale mesh
- **WHEN** a plan expects a mesh key that differs from the resident buffer's key (the seed changed and the mesh was not re-uploaded)
- **THEN** rendering fails with the tile id and both keys instead of drawing the stale buffer

#### Scenario: Placement
- **WHEN** the same tiles are placed by the renderer's free list, scattered over blocks by the caller, or by the residency manager's slots across evictions
- **THEN** every frame equals the one-shot result

#### Scenario: Full pool
- **WHEN** every slot is in use or still in flight and another tile is uploaded
- **THEN** the upload fails with the tile id, and succeeds into the freed slot once a frame has retired it

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL be able to hold tiles of levels 3 and finer in the compact format (GEN-019) in pooled atlas slots and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target), beside vertex-format tiles (levels 0-2, and fallback tiles whose finest-level relief breaks CON-13) in the same frame. A block of the main pool SHALL be one 4096 x 4096 `Rgba16Uint` atlas texture (124 x 124 = 15,376 slots at cells 32, allocated on first use or by the warm-up of REND-017); each slot SHALL have a 128-byte record in one persistent buffer indexed by slot; a draw SHALL name its atlas by being in a run of that atlas (consecutive nodes in one atlas form one run, drawn with the compact pipeline, other runs with the vertex pipeline, in the plan's order) and its slot through a per-frame 16-byte tile reference indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the vertex format's within the model tolerance of the compact tile (position model error, 16-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile in the wrong pool (a level 0-2 mesh or a compact tile of level 2 in an atlas slot, a compact tile in the vertex pool or in a vertex-format renderer, a slot outside the pool) SHALL be refused, not written. A fallback tile SHALL live in the renderer's own region of the vertex pool (the manager's main slot staying empty) and be given back with the tile, and a fallback beyond the region SHALL be refused with the tile id. The product paths (`test-render` single views, `--stream`, `--pipelined`, `--flight`, `--perf`, the repro replay) SHALL build and draw the compact format by default and the vertex format with `--tile-format vertex` (the reference); a flight from compact tiles SHALL equal the vertex flight frame by frame within 0.1 % of the pixels over 12 levels, and the plans SHALL NOT depend on the format. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: atlases (texture size), vertex pool, index buffer, node records and the slot-record buffer, and the mid and high shares SHALL hold 15 and 23 atlases (230,640 and 353,648 slots at cells 32).

Verify: A, C
Status: active
Source: ADR 0033 points 3-5, SPIKE-06, CON-03, CON-11, GEN-019, REND-015

#### Scenario: Images against the vertex path
- **WHEN** the ten scripted views are rendered in the level, relief and normals views from compact tiles and from vertex-format tiles, on the software adapter (cells 16), and three views at cells 32 including the 1 m ground camera
- **THEN** the pixels that differ by more than 12 levels stay within the stated fractions, and compact tiles were drawn

#### Scenario: Default path and flight
- **WHEN** the scripted flight is rendered with the default tile format and with `--tile-format vertex`
- **THEN** the flight index (plans) is identical and every frame differs in at most 0.1 % of its pixels by more than 12 levels; the golden views drawn from compact tiles agree with their goldens within rounding on the adapters that have them

#### Scenario: Morph target
- **WHEN** every node's morph interval is forced so each vertex is drawn at its parent-grid position and height, on the steep field, in the relief and morph-height views
- **THEN** the compact and vertex images agree within the steep tolerance (the morph-height view shows variation, and a shader that morphs the height to the wrong target fails)

#### Scenario: Both submission paths, any placement
- **WHEN** a plan is drawn per-draw and indirect with the tiles placed by the free list and scattered over atlases and the vertex pool
- **THEN** all four images are identical, and the scattered indirect path needs more than one multi-draw

#### Scenario: Fallback and coarse tiles beside compact tiles
- **WHEN** the steep stress field is planned with the finest level declared at 9 so finest tiles exceed 65.535 m of relief
- **THEN** levels 0-2 are vertex format, the steep finest tiles are fallbacks in the renderer's region, the others compact, the image equals the vertex path's within the tolerance, and forgetting the tiles frees the fallback slots

#### Scenario: Pools and the share
- **WHEN** the tier pools are configured for cells 32 (and a real 4096-texel atlas is drawn from, with tiles in its last slots)
- **THEN** the mid share holds 15 atlases (230,640 slots) and the high share 23 (353,648), the model's allocation stays under the share with the fixed items inside their reserve, a renderer's allocation equals the model for its live blocks, and a tile size whose slot records would overflow the reserve (cells 16 and smaller) is refused with the numbers
