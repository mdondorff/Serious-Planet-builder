## MODIFIED Requirements

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL be able to hold tiles of levels 3 and finer in the compact format (GEN-019) in pooled slots of storage-buffer blocks and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target), beside vertex-format tiles (levels 0-2, and fallback tiles whose finest-level relief breaks CON-13) in the same frame. A block of the main pool SHALL be one storage buffer of at most 128 MiB (`block_slots x slot_bytes`: 15,406 slots of 33 x 33 texels of 8 bytes at cells 32, allocated on first use or by the warm-up of REND-017), its slots following one another and the texels row-major in the slot (16-bit height, two 16-bit octahedral normal components, 16 spare bits: the texel bytes, the records, the packs and the mesh keys are those of the atlas texture of ADR 0033, so no golden changes); the vertex stage and the fragment stage (the climate of the tile, GEN-021) SHALL read a texel by `slot start + j x (cells + 1) + i` from the block bound for the run and get the values the atlas texture returned, byte for byte; each slot SHALL have a 128-byte record in one persistent buffer indexed by slot; a draw SHALL name its block by being in a run of that block (consecutive nodes in one block form one run, drawn with the compact pipeline, other runs with the vertex pipeline, in the plan's order) and its slot through a per-frame 16-byte tile reference (the slot's first texel in its block, reserved, the slot index, reserved) indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the vertex format's within the model tolerance of the compact tile (position model error, 16-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile in the wrong pool (a level 0-2 mesh or a compact tile of level 2 in a block slot, a compact tile in the vertex pool or in a vertex-format renderer, a slot outside the pool) SHALL be refused, not written. A fallback tile SHALL live in the renderer's own region of the vertex pool (the manager's main slot staying empty) and be given back with the tile, and a fallback beyond the region SHALL be refused with the tile id. The product paths (`test-render` single views, `--stream`, `--pipelined`, `--flight`, `--perf`, the repro replay) SHALL build and draw the compact format by default and the vertex format with `--tile-format vertex` (the reference); a flight from compact tiles SHALL equal the vertex flight frame by frame within 0.1 % of the pixels over 12 levels, and the plans SHALL NOT depend on the format. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: compact blocks (buffer bytes), vertex pool, index buffer, node records and the slot-record buffer, and the mid and high shares SHALL hold 15 and 23 blocks (231,090 and 354,338 slots at cells 32). The frames drawn from the storage-buffer blocks SHALL be the pixels of the frames the atlas textures drew (known-answer hashes recorded from the atlas path on WARP and on the discrete GPU: the ten scripted views in every debug view, persistent renderers per slot and batched, the lit frame with the atmosphere, the lit water frames).

Verify: A, C
Status: active
Source: ADR 0033 points 3-5, ADR 0054, SPIKE-06, CON-03, CON-11, GEN-019, GEN-021, REND-015

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
- **THEN** the mid share holds 15 blocks (231,090 slots) and the high share 23 (354,338), the model's allocation stays under the share with the fixed items inside their reserve, a renderer's allocation equals the model for its live blocks, and a tile size whose slot records would overflow the reserve (cells 16 and smaller) is refused with the numbers

#### Scenario: The buffer path draws the recorded pixels
- **WHEN** the ten scripted views are drawn in all nine debug views from compact tiles (one-shot), persistent renderers draw four views per slot and batched, and the lit frame with the atmosphere and the lit water frames are drawn
- **THEN** the hash of the pixels equals the hash recorded from the atlas path for the adapter's driver (WARP, the discrete GPU), and a driver without a recorded hash prints its hash

### Requirement: REND-017 No one-time initialisation inside a measured frame
The terrain renderer SHALL be able to do the one-time work of its first frames before the first frame (`warm_up`): create the small vertex pool and the first block of the main pool (block 0 of either format; for the compact format the storage buffer of block 0 with its bind group), grow the per-frame ring to a count above the node count of the standard scenes' plans (2048 nodes, within the share's node-record bytes), and submit one throwaway frame, polled to completion, that is representative of the measured frames: it records from every set of the per-frame ring, draws a zeroed tile through every pipeline of the tile format in the active draw mode (the indirect path as a multi-draw with a count above one), and runs the timestamp pass with its resolve and readback where the adapter supports timestamps, so that the lazy first-use work of the driver and of wgpu (pipeline state, resource initialisation) is done outside any measured frame. The throwaway frame SHALL make no tile resident, SHALL NOT count as a frame (the frame clock of the slot retirement and the frames in flight do not move) and SHALL NOT change any image. The bytes it allocates SHALL equal the pure model of the pool configuration with one block and the vertex pool live (STRM-006); every other block and ring growth stays on demand and the share stays a ceiling. The renderer SHALL count the long-lived GPU objects it creates (pipelines, pool allocations, ring builds); after `warm_up`, a script whose first frames fit the warmed block and ring SHALL create none in frames 0 and 1. The renderer SHALL also count the parts of itself a submission has used (ring sets, pipelines, timestamp passes, draw modes, lit variants); after `warm_up` these SHALL be the maximum a scene's frames reach, and frames 0 to 3 (untimed and timed) SHALL use no part for the first time. `enable_lit` and `enable_atmosphere` are not part of `warm_up`: `warm_up_lit` SHALL enable the lit path and, for a plan with an atmosphere, the atmosphere, and draw that plan (no node) once for every set of the ring, each polled to completion, after which lit frames with tiles, adaptive and with the atmosphere, SHALL use no part for the first time and create no long-lived object; the lit variants recorded (a lit frame, a lit frame with the atmosphere) are counted. The pipelined loop, the streaming perf scenes and the viewer window SHALL warm the renderer in their setup, outside the timed loop; frame 0 stays in the measured frames (ADR 0032, TEST-018) and records like the frames after it.

Verify: C
Status: active
Source: ADR 0032, REND-013, REND-016, TEST-018, STRM-006, perf windows 1 and 2 (F4, F9), owner 2026-10-17 (item 5b)

#### Scenario: Nothing is created in the first frames
- **WHEN** a vertex-format and a compact renderer, per-draw and indirect, are warmed and then upload a plan's tiles and submit frames 0, 1 and 2
- **THEN** the pipeline, pool-allocation and ring-build counters and the live blocks are the warm-up's, none became resident during the warm-up, and a renderer that was not warmed allocates in the same frames

#### Scenario: Allocation equals the model
- **WHEN** a renderer is warmed
- **THEN** one block and the vertex pool are live, and `pool_allocated_bytes` equals `allocated_bytes_with(1, true)` with the node records at their allocated size, under the share

#### Scenario: Same image
- **WHEN** the same plan is rendered by a warmed and by a cold renderer
- **THEN** the pixels are identical

#### Scenario: Lit warm-up
- **WHEN** a warmed renderer is given `warm_up_lit` with an atmosphere plan and then draws four lit frames with tiles, untimed and timed
- **THEN** the lit and atmosphere variants are both counted after the warm-up, the other counters and the long-lived lit objects do not change in the four frames, and no tile became resident during the warm-up

#### Scenario: Nothing is first-used by a measured frame
- **WHEN** a vertex-format and a compact renderer, per-draw and indirect, are warmed, then submit frames 0 to 3 alternating untimed and timed
- **THEN** the warm-up used every ring set, every pipeline of the format, the timed pass where supported and the active draw mode in one waited submission, frames 0 to 3 change none of those counts, and a cold renderer reaches every ring set only through its first two frames

### Requirement: REND-018 Batched tile writes
The terrain renderer SHALL write a frame's tiles to the pools in batches: `TerrainRenderer::upload_tiles_at` places the frame's tiles in the order given (the upload order of STRM-006), each exactly as `upload_tile_at` does (the manager's slot, the same checks, errors and bookkeeping), and issues the data writes of the compact tiles after the last one, sorted by slot, with one `write_buffer` per run of adjacent slots of one block for the texels (the run's tiles concatenated into one contiguous range of the block) and one `write_buffer` per run of adjacent slot indices for the slot records (across blocks, the records being one buffer); a slot written twice in a batch SHALL hold the last tile. Vertex-format tiles (levels 0-2, fallback tiles, the vertex reference format) SHALL be written at once, one call each, because a call carries 58,608 bytes (53,724 B of vertices and 4,884 B of vertex climate, ADR 0039) and the call is not the cost (a coalescing copy would add a pass over the data). The calls a batch makes for compact tiles SHALL be exactly the number of those runs (texel runs plus record runs), so at most the per-slot path's two per tile for any slots, and for adjacent slots of one block one texel run plus one record run. The reference path (`UploadMode::PerSlot`: `--upload per-slot`, two `write_buffer` calls per compact tile) SHALL stay selectable, the default being batched, and the pool contents (every block and the slot records, read back), the images and the bookkeeping SHALL be identical in both modes. The renderer SHALL count the queue calls (`write_calls`) and the bytes handed to them (`staging_bytes`) of its tile writes; the managed loop SHALL hand the frame's tiles to its sink as one batch (`TileSink::upload_batch`, whose default is the loop over `upload_at`, one call per tile, so a sink that does not batch keeps working), in the upload order, and report the sink's calls in the frame's counts (`upload_calls`, TEST-012). The tile bytes of the 1 MiB cap (STRM-006) and the placement of the manager SHALL NOT change. The compact blocks and the slot-record buffer carry `COPY_SRC` so that tests can read the pools back (`read_compact_pools`; no change to the allocated bytes). The conversion of a tile to bytes (vertex floats, texels) SHALL NOT go through a byte-wise iterator (about eight times the CPU of copying the same bytes in slices). The staging of a batch SHALL NOT allocate per frame once the buffers have grown to the largest batch: the converted texels, the lists of writes, their sort orders, the rectangle of a run, the bytes of a run of records and the bytes of a vertex-format tile live in buffers the renderer keeps and clears between batches (`TerrainRenderer::batch_scratch_capacity` stays constant over frames of the same or a smaller size), because the frame thread shares the heap with the generation threads and the time of an allocation there varies with their load. The managed loop SHALL keep its list of waiting tiles and its batch between frames, SHALL rebuild the draw class of the upload ranking (the draw tiles with their ancestors and plan priorities) only when the draw wants changed and the prefetch closure only when the prefetch wants changed (`ManagedLoop::ranking_rebuilds`), and SHALL rank the residency class by looking the tile up in R, which is closed under parents, never by building a set over R (R is sorted and closed under parents, `ResidencySet::is_sorted_and_closed`, which the loop checks in debug builds; it costs the size of R, about 11,000 tiles at the ground), so that a frame whose wants stayed builds no set over them whatever it uploads, and the draw walk SHALL stop at an ancestor already holding a priority at most the walked tile's (the sets equal those of the full walk), so that the ranking costs the draw set's size and not its size times the depth.

Verify: A, C
Status: active
Source: CON-20, CON-21, STRM-006, REND-013, REND-016, ADR 0033 point 9, ADR 0054, TEST-018, perf window 1 (F5)

#### Scenario: Calls follow runs
- **WHEN** a frame writes 1, 10 and 118 compact tiles into adjacent slots, from the middle of a row, every third slot, and in a shuffled order
- **THEN** the batched path makes the number of runs of adjacent slots in calls (118 adjacent tiles over blocks of 64 slots: 2 texel runs and 1 record run, 3 calls against 236 per slot; inside one product block, 2 calls), never more than the per-slot path, and the same staging bytes

#### Scenario: Same pools and images
- **WHEN** the same tiles are placed per slot and batched, including a slot written twice in one batch and the ten standard views with levels 0-2 in the vertex pool
- **THEN** every block and the slot records read back equal, the images are identical, and a batch that fails writes the tiles placed before the error

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
