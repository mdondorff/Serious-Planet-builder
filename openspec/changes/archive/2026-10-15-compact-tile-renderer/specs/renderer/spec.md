## ADDED Requirements

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL be able to hold tiles of levels 3 and finer in the compact format (GEN-019) in pooled atlas slots and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target), beside vertex-format tiles (levels 0-2, and fallback tiles whose finest-level relief breaks CON-13) in the same frame. A block of the main pool SHALL be one 4096 x 4096 `Rgba16Uint` atlas texture (124 x 124 = 15,376 slots at cells 32, allocated on first use); each slot SHALL have a 128-byte record in one persistent buffer indexed by slot; a draw SHALL name its atlas by being in a run of that atlas (consecutive nodes in one atlas form one run, drawn with the compact pipeline, other runs with the vertex pipeline, in the plan's order) and its slot through a per-frame 16-byte tile reference indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the vertex format's within the model tolerance of the compact tile (position model error, 16-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile in the wrong pool (a level 0-2 mesh or a compact tile of level 2 in an atlas slot, a compact tile in the vertex pool or in a vertex-format renderer, a slot outside the pool) SHALL be refused, not written. A fallback tile SHALL live in the renderer's own region of the vertex pool (the manager's main slot staying empty) and be given back with the tile, and a fallback beyond the region SHALL be refused with the tile id. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: atlases (texture size), vertex pool, index buffer, node records and the slot-record buffer, and the mid and high shares SHALL hold 15 and 23 atlases (230,640 and 353,648 slots at cells 32).

Verify: A, C
Status: active
Source: ADR 0033 points 3-5, SPIKE-06, CON-03, CON-11, GEN-019, REND-015

#### Scenario: Images against the vertex path
- **WHEN** the ten scripted views are rendered in the level, relief and normals views from compact tiles and from vertex-format tiles, on the software adapter (cells 16), and three views at cells 32 including the 1 m ground camera
- **THEN** the pixels that differ by more than 12 levels stay within the stated fractions, and compact tiles were drawn

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
