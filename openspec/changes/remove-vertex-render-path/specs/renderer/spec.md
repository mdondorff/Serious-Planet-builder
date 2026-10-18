## ADDED Requirements

### Requirement: REND-031 Reference renderer
`render` SHALL offer a test-only reference renderer (`render::reference`, behind the cargo feature `reference-renderer`, ADR 0062 point 1) that draws a `TerrainPlan` from CPU `TileMesh`es in the debug views, with one pipeline and one vertex buffer per tile, so that the compact renderer's image tests keep an independent geometry source. It SHALL have no pools, no fallback, no lit path, no atmosphere and no water, and SHALL refuse a plan with a lit part. It SHALL NOT be a second production path: the feature is off by default and no product crate and no non-dev dependency edge enables it (BUILD-001).

Verify: C
Status: planned
Source: ADR 0062, SPIKE-12 Q4, REND-016

#### Scenario: Reference against the compact renderer
- **WHEN** the ten scripted views are drawn in the level, relief and normals views, once by the reference renderer from the meshes and once by the compact renderer from the same tiles
- **THEN** at most the tolerance of REND-016 of the pixels differ (the model error of the compact base surface), and the compact path is exercised on more than 100 tiles

#### Scenario: Lit plan refused
- **WHEN** a plan with a lit part is given to the reference renderer
- **THEN** it fails with an error naming the missing lit path, and draws nothing

## MODIFIED Requirements

### Requirement: REND-013 Persistent terrain renderer
The terrain renderer SHALL create its pipeline, uniform buffers and render targets once, keep tiles in pooled fixed-size slots of a few large storage-buffer blocks (allocated on first use or by the warm-up of REND-017, no buffer per tile), upload each tile's slot (GEN-025) once per mesh key (a mesh with a different key takes a new slot and the old one is released), draw a frame by writing only the per-frame uniforms and recording the draws, report CPU and wall time per frame and GPU time from timestamp queries when the device supports them, and produce exactly the image of the one-shot executor. Where a tile sits (the renderer's own free list or a placement chosen by the residency manager) SHALL NOT change the image. The bytes it allocates SHALL equal the pure model of the pool configuration (STRM-006) and a full pool SHALL refuse an upload with the tile id.

Verify: C
Status: active
Source: CON-20, CON-21, ADR 0013, ADR 0033 point 3

#### Scenario: Reuse
- **WHEN** the same tiles are drawn in several frames and views
- **THEN** no tile data is uploaded after the first upload and every frame equals the one-shot result

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

### Requirement: REND-015 Indirect multi-draw
The terrain renderer SHALL be able to submit a plan's draws as indirect multi-draws built from the plan on the CPU: one `DrawIndexedIndirect` record per planned node in the plan's order (`first_instance` = the node's index, `base_vertex` 0, because a compact tile has no vertex buffer and the vertex stage pulls from the block (ADR 0054), `instance_count` 1, the index count of the tile grid), grouped into runs of consecutive nodes that sit in one block of the pool, one `multi_draw_indexed_indirect` per run, with the per-node records in a storage buffer the shader indexes with the instance index. The per-draw loop SHALL remain, selectable (`--draw per-draw|indirect`), as the fallback where the adapter lacks `INDIRECT_FIRST_INSTANCE` or indirect execution, and as the reference: both paths SHALL draw identical images for the same plan. The records SHALL come from the plan and the slots alone (no GPU-side culling or decisions, CON-03); per-frame globals, node records and argument records SHALL live in a ring so a frame in flight is never overwritten (STRM-016); shaders SHALL read only camera-relative and tile-local values (REND-005). The bytes written per frame for node and argument records SHALL be reported (TEST-011).

Verify: A, C
Status: active
Source: CON-03, CON-11, ADR 0033, M4 item 7 (review item C)

#### Scenario: Records equal the plan
- **WHEN** the draw list of a standard view's plan is built from the slots of its tiles
- **THEN** there is one record per node in the plan's order, `first_instance` equals the node index, `base_vertex` is 0, the runs cover the records, and the bytes per frame (208 B node record, the 192 B of origin, colour, morph interval and grid phases and the 16 B up vector of ADR 0039, plus 20 B argument record per node) stay within nodes x 228 B (the pin moved from 212 with the record) for each of the ten views (a declared addition to the pinned counts)

#### Scenario: Identical images
- **WHEN** the ten scripted views are rendered in the level and relief views, with the tiles placed by the free list and scattered over blocks, once per-draw and once indirect, on the software adapter and on the discrete GPU
- **THEN** the images are identical and the renderer reports the same draws, triangles and node-record bytes, one argument record per node, and one multi-draw per run (more than one where the nodes sit in several buffers)

#### Scenario: Unsupported adapter
- **WHEN** indirect mode is requested on an adapter without the feature
- **THEN** the renderer keeps the per-draw loop and reports the mode in effect

#### Scenario: Frames in flight
- **WHEN** more frames than the ring is deep are submitted without waiting, alternating two plans
- **THEN** the last frame shows its own plan, and `--stream` and `--pipelined` runs on either path settle on the same image

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL hold every drawn tile, levels 3 and finer (the render floor, LOD-011), in the compact format (GEN-019), the only tile format, in pooled slots of storage-buffer blocks and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target). A block of the main pool SHALL be one storage buffer of at most 128 MiB (`block_slots x slot_bytes`: 14,639 slots of 9,168 bytes at cells 32, allocated on first use or by the warm-up of REND-017), its slots following one another and the texels row-major in the slot (the slot of GEN-025: a 128-byte header, the 324-byte climate block and the wide texels, a 32-bit height step count and two 15-bit octahedral normal components; `MESH_VERSION` 8, so every terrain golden is regenerated through `golden-review`); the vertex stage SHALL read the slot's header and the texel `j x (cells + 1) + i` at the offsets of GEN-025 from the block bound for the run and decode the height as `hr_min + f32(q) * hr_step` and the normal by `extractBits`, and the fragment stage the slot's climate block (GEN-021); there is no separate record buffer; a draw SHALL name its block by being in a run of that block (consecutive nodes in one block form one run, in the plan's order) and its slot through a per-frame 16-byte tile reference (the slot's first word in its block, reserved, the slot index, reserved) indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the reference renderer's (REND-031: the same plan drawn from the CPU meshes) within the model tolerance of the compact tile (position model error, 15-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile below the render floor (a compact tile of level 2 or coarser) or a slot outside the pool SHALL be refused, not written. There is no fallback form: a tile whose relief the wide texel cannot hold is a generator concern (GEN-019), never drawn in another format. The product paths (`test-render` single views, `--stream`, `--pipelined`, `--flight`, `--perf`, the repro replay) SHALL build and draw the compact format only (there is no `--tile-format`); a flight SHALL equal the reference renderer's frames of the same plans within 0.1 % of the pixels over 12 levels. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: compact blocks (buffer bytes), index buffer, and node records, and the mid and high shares SHALL hold 15 and 23 blocks (219,585 and 336,697 slots at cells 32, STRM-006). The known-answer pixel hashes of the buffer path (the ten scripted views in every debug view, persistent renderers per slot and batched, the lit frame with the atmosphere, the lit water frames) SHALL be re-recorded for the wide texel under `MESH_VERSION` 8 on WARP and on the discrete GPU, each change through `golden-review` as rounding noise (ADR 0017), and the pixels SHALL equal them.

Verify: A, C
Status: active
Source: ADR 0033 points 3-5, ADR 0054, SPIKE-06, CON-03, CON-11, GEN-019, GEN-021, REND-015, GEN-025, ADR 0062

#### Scenario: Images against the vertex path
- **WHEN** the ten scripted views are rendered in the level, relief and normals views from compact tiles and by the reference renderer from the meshes, on the software adapter (cells 16), and three views at cells 32 including the 1 m ground camera
- **THEN** the pixels that differ by more than 12 levels stay within the stated fractions, and compact tiles were drawn

#### Scenario: Default path and flight
- **WHEN** the scripted flight is rendered by the product path and, from the same plans, by the reference renderer
- **THEN** every frame differs in at most 0.1 % of its pixels by more than 12 levels; the golden views drawn from compact tiles agree with their goldens within rounding on the adapters that have them

#### Scenario: Morph target
- **WHEN** every node's morph interval is forced so each vertex is drawn at its parent-grid position and height, on the steep field, in the relief and morph-height views
- **THEN** the compact and reference images agree within the steep tolerance (the morph-height view shows variation, and a shader that morphs the height to the wrong target fails)

#### Scenario: Both submission paths, any placement
- **WHEN** a plan is drawn per-draw and indirect with the tiles placed by the free list and scattered over blocks
- **THEN** all four images are identical, and the scattered indirect path needs more than one multi-draw

#### Scenario: Fallback and coarse tiles beside compact tiles
- **WHEN** the steep stress field is planned with the finest level declared at 9 so finest tiles exceed 65.535 m of relief
- **THEN** every drawn tile is compact with the wide texel, none of level 0-2 is drawn, and the image equals the reference renderer's within the steep tolerance

#### Scenario: Pools and the share
- **WHEN** the tier pools are configured for cells 32 (and a real block of 128 MiB is drawn from, with tiles in its last slots)
- **THEN** the mid share holds 15 blocks (219,585 slots) and the high share 23 (336,697), the model's allocation stays under the share with the fixed items inside their reserve, a renderer's allocation equals the model for its live blocks, and no tile size overflows the reserve with records (the record is in the slot)

#### Scenario: The buffer path draws the recorded pixels
- **WHEN** the ten scripted views are drawn in all nine debug views from compact tiles (one-shot), persistent renderers draw four views per slot and batched, and the lit frame with the atmosphere and the lit water frames are drawn
- **THEN** the hash of the pixels equals the hash recorded for the wide texel under `MESH_VERSION` 8 for the adapter's driver (WARP, the discrete GPU), and a driver without a recorded hash prints its hash

#### Scenario: One slot, one write
- **WHEN** a tile is written to a block and the block is read back
- **THEN** the slot's bytes at its start equal the CPU tile's slot (GEN-025) from one `write_buffer` range, no record buffer exists, and the shader's header and climate reads equal the CPU mirror's

### Requirement: REND-017 No one-time initialisation inside a measured frame
The terrain renderer SHALL be able to do the one-time work of its first frames before the first frame (`warm_up`): create the first block of the pool (the storage buffer of block 0 with its bind group), grow the per-frame ring to a count above the node count of the standard scenes' plans (2048 nodes, within the share's node-record bytes), and submit one throwaway frame, polled to completion, that is representative of the measured frames: it records from every set of the per-frame ring, draws a zeroed tile through every pipeline of the tile format in the active draw mode (the indirect path as a multi-draw with a count above one), and runs the timestamp pass with its resolve and readback where the adapter supports timestamps, so that the lazy first-use work of the driver and of wgpu (pipeline state, resource initialisation) is done outside any measured frame. The throwaway frame SHALL make no tile resident, SHALL NOT count as a frame (the frame clock of the slot retirement and the frames in flight do not move) and SHALL NOT change any image. The bytes it allocates SHALL equal the pure model of the pool configuration with one block live (STRM-006); every other block and ring growth stays on demand and the share stays a ceiling. The renderer SHALL count the long-lived GPU objects it creates (pipelines, pool allocations, ring builds); after `warm_up`, a script whose first frames fit the warmed block and ring SHALL create none in frames 0 and 1. The renderer SHALL also count the parts of itself a submission has used (ring sets, pipelines, timestamp passes, draw modes, lit variants); after `warm_up` these SHALL be the maximum a scene's frames reach, and frames 0 to 3 (untimed and timed) SHALL use no part for the first time. `enable_lit` and `enable_atmosphere` are not part of `warm_up`: `warm_up_lit` SHALL enable the lit path and, for a plan with an atmosphere, the atmosphere, and draw that plan (no node) once for every set of the ring, each polled to completion, after which lit frames with tiles, adaptive and with the atmosphere, SHALL use no part for the first time and create no long-lived object; the lit variants recorded (a lit frame, a lit frame with the atmosphere) are counted. The pipelined loop, the streaming perf scenes and the viewer window SHALL warm the renderer in their setup, outside the timed loop; frame 0 stays in the measured frames (ADR 0032, TEST-018) and records like the frames after it.

Verify: C
Status: active
Source: ADR 0032, REND-013, REND-016, TEST-018, STRM-006, perf windows 1 and 2 (F4, F9), owner 2026-10-17 (item 5b)

#### Scenario: Nothing is created in the first frames
- **WHEN** a renderer, per-draw and indirect, is warmed and then uploads a plan's tiles and submit frames 0, 1 and 2
- **THEN** the pipeline, pool-allocation and ring-build counters and the live blocks are the warm-up's, none became resident during the warm-up, and a renderer that was not warmed allocates in the same frames

#### Scenario: Allocation equals the model
- **WHEN** a renderer is warmed
- **THEN** one block is live, and `pool_allocated_bytes` equals `allocated_bytes_with(1)` with the node records at their allocated size, under the share

#### Scenario: Same image
- **WHEN** the same plan is rendered by a warmed and by a cold renderer
- **THEN** the pixels are identical

#### Scenario: Lit warm-up
- **WHEN** a warmed renderer is given `warm_up_lit` with an atmosphere plan and then draws four lit frames with tiles, untimed and timed
- **THEN** the lit and atmosphere variants are both counted after the warm-up, the other counters and the long-lived lit objects do not change in the four frames, and no tile became resident during the warm-up

#### Scenario: Nothing is first-used by a measured frame
- **WHEN** a renderer, per-draw and indirect, is warmed, then submits frames 0 to 3 alternating untimed and timed
- **THEN** the warm-up used every ring set, every pipeline of the format, the timed pass where supported and the active draw mode in one waited submission, frames 0 to 3 change none of those counts, and a cold renderer reaches every ring set only through its first two frames

### Requirement: REND-018 Batched tile writes
The terrain renderer SHALL write a frame's tiles to the pools in batches: `TerrainRenderer::upload_tiles_at` places the frame's tiles in the order given (the upload order of STRM-006), each exactly as `upload_tile_at` does (the manager's slot, the same checks, errors and bookkeeping), and issues the data writes of the compact tiles after the last one, sorted by slot, with one `write_buffer` per run of adjacent slots of one block for the slots (the run's slots concatenated into one contiguous range of the block: header, climate block and texels, GEN-025); a slot written twice in a batch SHALL hold the last tile. The calls a batch makes for compact tiles SHALL be exactly the number of those runs (slot runs), so at most the per-slot path's one per tile for any slots, and for adjacent slots of one block one run. The reference path (`UploadMode::PerSlot`: `--upload per-slot`, one `write_buffer` call per compact tile) SHALL stay selectable, the default being batched, and the pool contents (every block, read back), the images and the bookkeeping SHALL be identical in both modes. The renderer SHALL count the queue calls (`write_calls`) and the bytes handed to them (`staging_bytes`) of its tile writes; the managed loop SHALL hand the frame's tiles to its sink as one batch (`TileSink::upload_batch`, whose default is the loop over `upload_at`, one call per tile, so a sink that does not batch keeps working), in the upload order, and report the sink's calls in the frame's counts (`upload_calls`, TEST-012). The tile bytes of the 1 MiB cap (STRM-006) and the placement of the manager SHALL NOT change. The compact blocks carry `COPY_SRC` so that tests can read the pools back (`read_compact_pools`; no change to the allocated bytes). The conversion of a tile to bytes (texels) SHALL NOT go through a byte-wise iterator (about eight times the CPU of copying the same bytes in slices). The staging of a batch SHALL NOT allocate per frame once the buffers have grown to the largest batch: the converted texels, the lists of writes, their sort orders, the rectangle of a run live in buffers the renderer keeps and clears between batches (`TerrainRenderer::batch_scratch_capacity` stays constant over frames of the same or a smaller size), because the frame thread shares the heap with the generation threads and the time of an allocation there varies with their load. The managed loop SHALL keep its list of waiting tiles and its batch between frames, SHALL rebuild the draw class of the upload ranking (the draw tiles with their ancestors and plan priorities) only when the draw wants changed and the prefetch closure only when the prefetch wants changed (`ManagedLoop::ranking_rebuilds`), and SHALL rank the residency class by looking the tile up in R, which is closed under parents, never by building a set over R (R is sorted and closed under parents, `ResidencySet::is_sorted_and_closed`, which the loop checks in debug builds; it costs the size of R, about 11,000 tiles at the ground), so that a frame whose wants stayed builds no set over them whatever it uploads, and the draw walk SHALL stop at an ancestor already holding a priority at most the walked tile's (the sets equal those of the full walk), so that the ranking costs the draw set's size and not its size times the depth.

Verify: A, C
Status: active
Source: CON-20, CON-21, STRM-006, REND-013, REND-016, ADR 0033 point 9, ADR 0054, TEST-018, perf window 1 (F5), GEN-025, ADR 0062

#### Scenario: Calls follow runs
- **WHEN** a frame writes 1, 10 and 118 compact tiles into adjacent slots, from the middle of a row, every third slot, and in a shuffled order
- **THEN** the batched path makes the number of runs of adjacent slots in calls (118 adjacent tiles over blocks of 64 slots: 2 runs, 2 calls against 118 per slot; inside one product block, 1 call), never more than the per-slot path, and the same staging bytes

#### Scenario: Same pools and images
- **WHEN** the same tiles are placed per slot and batched, including a slot written twice in one batch and the ten standard views
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

### Requirement: REND-026 Lit terrain frame
A plan with a `LitPlan` SHALL be drawn into the `Rgba16Float` scene target with the same geometry and the same reversed-Z `Depth32Float` test as the debug path (the same pixels covered), every written value finite and within [0, 60000], and tone mapped into the renderer's colour target; the debug path of the same renderer SHALL stay byte-identical, before and after lit frames and compared with a renderer that never enabled the lit path. A lit frame at a fixed exposure SHALL be a pure function of its plan; the sun and the exposure SHALL come from the plan. Lit frames of the orbit, aerial and ground views, the EV-band view and the base shading's debug views (temperature, precipitation, albedo and lighting) SHALL match the adapter's goldens (class lit, first goldens of M5: blessed by `golden-review` under ADR 0037's policy, escalated to the owner's `/bless` otherwise). After `enable_lit` no frame SHALL create a pipeline, target or ring.

Verify: C
Status: active
Source: ADR 0038, ADR 0042 points 1-3, ADR 0037, ADR 0048, CON-12, CON-18, REND-017

#### Scenario: Coverage and range
- **WHEN** the scripted views are drawn on the lit path and on the debug path
- **THEN** the covered pixels are the same, every scene value is finite and at most 60,000, and the planet is lit

#### Scenario: Debug path untouched
- **WHEN** a renderer with the lit path draws a debug plan before and after lit frames
- **THEN** the image equals the image of a renderer that never enabled the lit path

#### Scenario: Pure function of the plan
- **WHEN** the same fixed-exposure lit plan is drawn twice, then with another sun, then two stops darker
- **THEN** the first two images are equal, the sun changes the image and the darker exposure is darker

#### Scenario: Formats
- **WHEN** the lit path's targets are inspected
- **THEN** the scene target is `Rgba16Float`, the tone-map output `Rgba8Unorm` and the depth `Depth32Float` with the Greater test

### Requirement: REND-027 Climate and slope reach the fragment stage
The tile's climate field (GEN-021) SHALL reach the fragment stage: the fragment stage reads the 9 x 9 texture from the climate block of the tile's slot (GEN-025; the tile reference and the block are visible to the fragment stage) and interpolates it bilinearly at the fragment's tile coordinates. The node record (208 bytes) SHALL carry the unit up vector of the tile origin and the inverse of its distance from the planet centre, decided in f64 and rounded once; the fragment stage SHALL form up as `normalize(up0 + local x inv_r)` from the tile-local position, so no planet-radius value reaches the stage (CON-11 v1.4). Class and bound of each attribute (ADR 0035): the stored climate texture is a **field** (GEN-021: the GPU decode is exact); the climate at a fragment is presentation data, the bilinear interpolation of the stored texels (the climate views agree with the colour of the analytic climate at the point under each pixel with a mean error of at most 0.02 on the orbit view and 0.005 on the 20 km view (at most 3 % of pixels, the limb, more than 0.1 off)); the up vector is presentation data within 5e-7 rad of the exact radial direction at every tile-local offset up to 1 km from the origin of a tile at 8,848 m or at -11,000 m, anywhere on the planet; temporal stability at 1 m (CON-11): the slope input changes by at most 3e-7 between fragments 1 mm apart at the worst case (a finest-level tile far from the origin), the albedo selected from it by at most 1/255, two tiles that cover one point agree about up within 5e-7 rad, and the shading depends only on tile-local quantities, never on the camera; the albedo, the radiance, the sun direction (f32, 6e-8) and the water level (f32) are presentation data with the bounds of REND-025 and LOOK-003.

Verify: B, C
Status: active
Source: ADR 0039 points 2 and 3, ADR 0035, ADR 0043, ADR 0033 point 4, CON-11, CON-14, REND-016, GEN-025, ADR 0062

#### Scenario: Up vector
- **WHEN** 1,584 tile origins and offsets, anywhere on the planet and at 8,848 m, -11,000 m and sea level, are evaluated by the shader's `up_at` and compared with the exact radial direction
- **THEN** every angle is at most 5e-7 rad (measured 1.4e-7) and every vector is a unit vector

#### Scenario: Stability at 1 m
- **WHEN** 1,001 fragments 1 mm apart along three axes of a tile at 8,848 m are evaluated against a surface at the middle of the soil-to-rock transition
- **THEN** the slope input changes by at most 3e-7 and the albedo by at most 1/255 between neighbours, and two neighbouring tiles agree about up at one point within 5e-7 rad

#### Scenario: Climate views
- **WHEN** the temperature and precipitation views of the orbit and the 20 km view are drawn
- **THEN** their colours are the colours of the world's climate at the point under each pixel within the bounds above

#### Scenario: Formats
- **WHEN** every view is drawn with the tiles in different slots of the pool (the only tile format since ADR 0062 leaves no second format to compare)
- **THEN** the pictures are identical, so the shading does not depend on where a tile sits

