## MODIFIED Requirements

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL be able to hold tiles of levels 3 and finer in the compact format (GEN-019) in pooled slots of storage-buffer blocks and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target), beside vertex-format tiles (levels 0-2, and fallback tiles whose finest-level relief breaks CON-13) in the same frame. A block of the main pool SHALL be one storage buffer of at most 128 MiB (`block_slots x slot_bytes`: 14,639 slots of 9,168 bytes at cells 32, allocated on first use or by the warm-up of REND-017), its slots following one another and the texels row-major in the slot (the slot of GEN-025: a 128-byte header, the 324-byte climate block and the wide texels, a 32-bit height step count and two 15-bit octahedral normal components; `MESH_VERSION` 8, so every terrain golden is regenerated through `golden-review`); the vertex stage SHALL read the slot's header and the texel `j x (cells + 1) + i` at the offsets of GEN-025 from the block bound for the run and decode the height as `hr_min + f32(q) * hr_step` and the normal by `extractBits`, and the fragment stage the slot's climate block (GEN-021); there is no separate record buffer; a draw SHALL name its block by being in a run of that block (consecutive nodes in one block form one run, drawn with the compact pipeline, other runs with the vertex pipeline, in the plan's order) and its slot through a per-frame 16-byte tile reference (the slot's first word in its block, reserved, the slot index, reserved) indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the vertex format's within the model tolerance of the compact tile (position model error, 15-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile in the wrong pool (a level 0-2 mesh or a compact tile of level 2 in a block slot, a compact tile in the vertex pool or in a vertex-format renderer, a slot outside the pool) SHALL be refused, not written. A fallback tile SHALL live in the renderer's own region of the vertex pool (the manager's main slot staying empty) and be given back with the tile, and a fallback beyond the region SHALL be refused with the tile id. The product paths (`test-render` single views, `--stream`, `--pipelined`, `--flight`, `--perf`, the repro replay) SHALL build and draw the compact format by default and the vertex format with `--tile-format vertex` (the reference); a flight from compact tiles SHALL equal the vertex flight frame by frame within 0.1 % of the pixels over 12 levels, and the plans SHALL NOT depend on the format. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: compact blocks (buffer bytes), vertex pool, index buffer, and node records, and the mid and high shares SHALL hold 15 and 23 blocks (219,585 and 336,697 slots at cells 32, STRM-006). The known-answer pixel hashes of the buffer path (the ten scripted views in every debug view, persistent renderers per slot and batched, the lit frame with the atmosphere, the lit water frames) SHALL be re-recorded for the wide texel under `MESH_VERSION` 8 on WARP and on the discrete GPU, each change through `golden-review` as rounding noise (ADR 0017), and the pixels SHALL equal them.

Verify: A, C
Status: planned
Source: ADR 0033 points 3-5, ADR 0054, SPIKE-06, CON-03, CON-11, GEN-019, GEN-021, REND-015, GEN-025, ADR 0062

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
- **WHEN** a plan is drawn per-draw and indirect with the tiles placed by the free list and scattered over blocks and the vertex pool
- **THEN** all four images are identical, and the scattered indirect path needs more than one multi-draw

#### Scenario: Fallback and coarse tiles beside compact tiles
- **WHEN** the steep stress field is planned with the finest level declared at 9 so finest tiles exceed 65.535 m of relief
- **THEN** levels 0-2 are vertex format, the steep finest tiles are fallbacks in the renderer's region, the others compact, the image equals the vertex path's within the tolerance, and forgetting the tiles frees the fallback slots

#### Scenario: Pools and the share
- **WHEN** the tier pools are configured for cells 32 (and a real block of 128 MiB is drawn from, with tiles in its last slots)
- **THEN** the mid share holds 15 blocks (219,585 slots) and the high share 23 (336,697), the model's allocation stays under the share with the fixed items inside their reserve, a renderer's allocation equals the model for its live blocks, and no tile size overflows the reserve with records (the record is in the slot)

#### Scenario: The buffer path draws the recorded pixels
- **WHEN** the ten scripted views are drawn in all nine debug views from compact tiles (one-shot), persistent renderers draw four views per slot and batched, and the lit frame with the atmosphere and the lit water frames are drawn
- **THEN** the hash of the pixels equals the hash recorded for the wide texel under `MESH_VERSION` 8 for the adapter's driver (WARP, the discrete GPU), and a driver without a recorded hash prints its hash

#### Scenario: One slot, one write
- **WHEN** a tile is written to a block and the block is read back
- **THEN** the slot's bytes at its start equal the CPU tile's slot (GEN-025) from one `write_buffer` range, no record buffer exists, and the shader's header and climate reads equal the CPU mirror's

### Requirement: REND-018 Batched tile writes
The terrain renderer SHALL write a frame's tiles to the pools in batches: `TerrainRenderer::upload_tiles_at` places the frame's tiles in the order given (the upload order of STRM-006), each exactly as `upload_tile_at` does (the manager's slot, the same checks, errors and bookkeeping), and issues the data writes of the compact tiles after the last one, sorted by slot, with one `write_buffer` per run of adjacent slots of one block for the slots (the run's slots concatenated into one contiguous range of the block: header, climate block and texels, GEN-025); a slot written twice in a batch SHALL hold the last tile. Vertex-format tiles (levels 0-2, fallback tiles, the vertex reference format) SHALL be written at once, one call each, because a call carries 58,608 bytes (53,724 B of vertices and 4,884 B of vertex climate, ADR 0039) and the call is not the cost (a coalescing copy would add a pass over the data). The calls a batch makes for compact tiles SHALL be exactly the number of those runs (slot runs), so at most the per-slot path's one per tile for any slots, and for adjacent slots of one block one run. The reference path (`UploadMode::PerSlot`: `--upload per-slot`, one `write_buffer` call per compact tile) SHALL stay selectable, the default being batched, and the pool contents (every block, read back), the images and the bookkeeping SHALL be identical in both modes. The renderer SHALL count the queue calls (`write_calls`) and the bytes handed to them (`staging_bytes`) of its tile writes; the managed loop SHALL hand the frame's tiles to its sink as one batch (`TileSink::upload_batch`, whose default is the loop over `upload_at`, one call per tile, so a sink that does not batch keeps working), in the upload order, and report the sink's calls in the frame's counts (`upload_calls`, TEST-012). The tile bytes of the 1 MiB cap (STRM-006) and the placement of the manager SHALL NOT change. The compact blocks carry `COPY_SRC` so that tests can read the pools back (`read_compact_pools`; no change to the allocated bytes). The conversion of a tile to bytes (vertex floats, texels) SHALL NOT go through a byte-wise iterator (about eight times the CPU of copying the same bytes in slices). The staging of a batch SHALL NOT allocate per frame once the buffers have grown to the largest batch: the converted texels, the lists of writes, their sort orders, the rectangle of a run, the bytes of a vertex-format tile live in buffers the renderer keeps and clears between batches (`TerrainRenderer::batch_scratch_capacity` stays constant over frames of the same or a smaller size), because the frame thread shares the heap with the generation threads and the time of an allocation there varies with their load. The managed loop SHALL keep its list of waiting tiles and its batch between frames, SHALL rebuild the draw class of the upload ranking (the draw tiles with their ancestors and plan priorities) only when the draw wants changed and the prefetch closure only when the prefetch wants changed (`ManagedLoop::ranking_rebuilds`), and SHALL rank the residency class by looking the tile up in R, which is closed under parents, never by building a set over R (R is sorted and closed under parents, `ResidencySet::is_sorted_and_closed`, which the loop checks in debug builds; it costs the size of R, about 11,000 tiles at the ground), so that a frame whose wants stayed builds no set over them whatever it uploads, and the draw walk SHALL stop at an ancestor already holding a priority at most the walked tile's (the sets equal those of the full walk), so that the ranking costs the draw set's size and not its size times the depth.

Verify: A, C
Status: planned
Source: CON-20, CON-21, STRM-006, REND-013, REND-016, ADR 0033 point 9, ADR 0054, TEST-018, perf window 1 (F5), GEN-025, ADR 0062

#### Scenario: Calls follow runs
- **WHEN** a frame writes 1, 10 and 118 compact tiles into adjacent slots, from the middle of a row, every third slot, and in a shuffled order
- **THEN** the batched path makes the number of runs of adjacent slots in calls (118 adjacent tiles over blocks of 64 slots: 2 runs, 2 calls against 118 per slot; inside one product block, 1 call), never more than the per-slot path, and the same staging bytes

#### Scenario: Same pools and images
- **WHEN** the same tiles are placed per slot and batched, including a slot written twice in one batch and the ten standard views with levels 0-2 in the vertex pool
- **THEN** every block read back equal, the images are identical, and a batch that fails writes the tiles placed before the error

#### Scenario: The loop hands over one batch
- **WHEN** the managed loop runs the ground fill with caps of 1, 10 and 118 tiles
- **THEN** a frame that writes calls `upload_batch` once with its tiles in the order the per-slot path wrote them, in the same slots, with the same counts and bytes, `upload_calls` is the sink's calls (one per tile for a sink that does not batch, the runs of adjacent slots for one that does), and a frame that writes nothing makes no call

#### Scenario: Pipelined loop on the renderer
- **WHEN** the pipelined loop runs the cold start and a jump with `--upload per-slot` and `--upload batched` on the software adapter
- **THEN** the settled frames are identical and the batched log's `upload_calls` per uploaded tile is lower

#### Scenario: Staging buffers are reused
- **WHEN** frames of 118, 10, 20 and 100 compact tiles in moving, reversed and shuffled slots are placed one after another on one renderer
- **THEN** the capacity of its staging buffers is zero before the first batch, equal from the second frame on, and the pools read back equal those of the per-slot path that wrote the same frames

#### Scenario: Ranking sets follow the wants
- **WHEN** the managed loop uploads a fill of hundreds of tiles at two a frame with a camera that stays
- **THEN** no frame whose draw wants equal the previous frame's rebuilds the draw class, no frame whose prefetch wants stayed rebuilds the prefetch closure, and the draw class built with the early-stopping walks equals that of the definition for ascending, shuffled and tied priorities

### Requirement: REND-027 Climate and slope reach the fragment stage
Both tile formats SHALL deliver the tile's climate field (GEN-021) to the fragment stage: the compact tile's fragment stage reads the 9 x 9 texture from the climate block of the tile's slot (GEN-025; the tile reference and the block are visible to the fragment stage) and interpolates it bilinearly at the fragment's tile coordinates; the vertex format's GPU vertex is 48 bytes (the 44-byte vertex and the vertex's climate as two f16, `MeshVertex::GPU_STRIDE`; 58,608 B per tile at cells 32), the value at each vertex being the stored texture sampled at the vertex (`TileMesh::gpu_climate`) and interpolated across the triangles. The node record (208 bytes) SHALL carry the unit up vector of the tile origin and the inverse of its distance from the planet centre, decided in f64 and rounded once; the fragment stage SHALL form up as `normalize(up0 + local x inv_r)` from the tile-local position, so no planet-radius value reaches the stage (CON-11 v1.4). Class and bound of each attribute (ADR 0035): the stored climate texture is a **field** (GEN-021: the GPU decode is exact); the climate at a fragment is presentation data, the bilinear interpolation of the stored texels (the climate views agree with the colour of the analytic climate at the point under each pixel with a mean error of at most 0.02 on the orbit view and 0.005 on the 20 km view (at most 3 % of pixels, the limb, more than 0.1 off), and the two formats shade the same picture: at most 0.5 % of covered pixels differ by more than 5 % of a lit unit); the up vector is presentation data within 5e-7 rad of the exact radial direction at every tile-local offset up to 1 km from the origin of a tile at 8,848 m or at -11,000 m, anywhere on the planet; temporal stability at 1 m (CON-11): the slope input changes by at most 3e-7 between fragments 1 mm apart at the worst case (a finest-level tile far from the origin), the albedo selected from it by at most 1/255, two tiles that cover one point agree about up within 5e-7 rad, and the shading depends only on tile-local quantities, never on the camera; the albedo, the radiance, the sun direction (f32, 6e-8) and the water level (f32) are presentation data with the bounds of REND-025 and LOOK-003.

Verify: B, C
Status: planned
Source: ADR 0039 points 2 and 3, ADR 0035, ADR 0043, ADR 0033 point 4, CON-11, CON-14, REND-016, GEN-025, ADR 0062

#### Scenario: Up vector
- **WHEN** 1,584 tile origins and offsets, anywhere on the planet and at 8,848 m, -11,000 m and sea level, are evaluated by the shader's `up_at` and compared with the exact radial direction
- **THEN** every angle is at most 5e-7 rad (measured 1.4e-7) and every vector is a unit vector

#### Scenario: Stability at 1 m
- **WHEN** 1,001 fragments 1 mm apart along three axes of a tile at 8,848 m are evaluated against a surface at the middle of the soil-to-rock transition
- **THEN** the slope input changes by at most 3e-7 and the albedo by at most 1/255 between neighbours, and two neighbouring tiles agree about up at one point within 5e-7 rad

#### Scenario: Climate views
- **WHEN** the temperature and precipitation views of the orbit and the 20 km view are drawn in both formats
- **THEN** their colours are the colours of the world's climate at the point under each pixel within the bounds above

#### Scenario: Formats
- **WHEN** every view is drawn from compact and from vertex tiles
- **THEN** the pictures agree within the bound above
