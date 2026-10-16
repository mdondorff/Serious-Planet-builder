## MODIFIED Requirements

### Requirement: REND-015 Indirect multi-draw
The terrain renderer SHALL be able to submit a plan's draws as indirect multi-draws built from the plan on the CPU: one `DrawIndexedIndirect` record per planned node in the plan's order (`first_instance` = the node's index, `base_vertex` = the first vertex of the node's slot in the buffer the draw names, `instance_count` 1, the index count of the tile mesh), grouped into runs of consecutive nodes that sit in one pooled vertex buffer, one `multi_draw_indexed_indirect` per run, with the per-node records in a storage buffer the shader indexes with the instance index. The per-draw loop SHALL remain, selectable (`--draw per-draw|indirect`), as the fallback where the adapter lacks `INDIRECT_FIRST_INSTANCE` or indirect execution, and as the reference: both paths SHALL draw identical images for the same plan. The records SHALL come from the plan and the slots alone (no GPU-side culling or decisions, CON-03); per-frame globals, node records and argument records SHALL live in a ring so a frame in flight is never overwritten (STRM-016); shaders SHALL read only camera-relative and tile-local values (REND-005). The bytes written per frame for node and argument records SHALL be reported (TEST-011).

Verify: A, C
Status: active
Source: CON-03, CON-11, ADR 0033, M4 item 7 (review item C)

#### Scenario: Records equal the plan
- **WHEN** the draw list of a standard view's plan is built from the slots of its tiles
- **THEN** there is one record per node in the plan's order, `first_instance` equals the node index, `base_vertex` equals the slot's index in its buffer times the vertices per slot, the runs cover the records, and the bytes per frame (208 B node record, the 192 B of origin, colour, morph interval and grid phases and the 16 B up vector of ADR 0039, plus 20 B argument record per node) stay within nodes x 228 B (the pin moved from 212 with the record) for each of the ten views (a declared addition to the pinned counts)

#### Scenario: Identical images
- **WHEN** the ten scripted views are rendered in the level and relief views, with the tiles placed by the free list and scattered over blocks and the vertex pool, once per-draw and once indirect, on the software adapter and on the discrete GPU
- **THEN** the images are identical and the renderer reports the same draws, triangles and node-record bytes, one argument record per node, and one multi-draw per run (more than one where the nodes sit in several buffers)

#### Scenario: Unsupported adapter
- **WHEN** indirect mode is requested on an adapter without the feature
- **THEN** the renderer keeps the per-draw loop and reports the mode in effect

#### Scenario: Frames in flight
- **WHEN** more frames than the ring is deep are submitted without waiting, alternating two plans
- **THEN** the last frame shows its own plan, and `--stream` and `--pipelined` runs on either path settle on the same image

### Requirement: REND-018 Batched tile writes
The terrain renderer SHALL write a frame's tiles to the pools in batches: `TerrainRenderer::upload_tiles_at` places the frame's tiles in the order given (the upload order of STRM-006), each exactly as `upload_tile_at` does (the manager's slot, the same checks, errors and bookkeeping), and issues the data writes of the compact tiles after the last one, sorted by slot, with one `write_texture` per run of adjacent slots in one row of one atlas (the run's tiles interleaved row by row into one rectangle) and one `write_buffer` per run of adjacent slot indices for the slot records (across blocks, the records being one buffer); a slot written twice in a batch SHALL hold the last tile. Vertex-format tiles (levels 0-2, fallback tiles, the vertex reference format) SHALL be written at once, one call each, because a call carries 58,608 bytes (53,724 B of vertices and 4,884 B of vertex climate, ADR 0039) and the call is not the cost (a coalescing copy would add a pass over the data). The calls a batch makes for compact tiles SHALL be exactly the number of those runs (texel runs plus record runs), so at most the per-slot path's two per tile for any slots, and for adjacent slots the atlas rows touched plus one record run. The reference path (`UploadMode::PerSlot`: `--upload per-slot`, two calls per compact tile) SHALL stay selectable, the default being batched, and the pool contents (every atlas and the slot records, read back), the images and the bookkeeping SHALL be identical in both modes. The renderer SHALL count the queue calls (`write_calls`) and the bytes handed to them (`staging_bytes`) of its tile writes; the managed loop SHALL hand the frame's tiles to its sink as one batch (`TileSink::upload_batch`, whose default is the loop over `upload_at`, one call per tile, so a sink that does not batch keeps working), in the upload order, and report the sink's calls in the frame's counts (`upload_calls`, TEST-012). The tile bytes of the 1 MiB cap (STRM-006) and the placement of the manager SHALL NOT change. The atlas textures and the slot-record buffer carry `COPY_SRC` so that tests can read the pools back (`read_compact_pools`; no change to the allocated bytes). The conversion of a tile to bytes (vertex floats, texels) SHALL NOT go through a byte-wise iterator (about eight times the CPU of copying the same bytes in slices). The staging of a batch SHALL NOT allocate per frame once the buffers have grown to the largest batch: the converted texels, the lists of writes, their sort orders, the rectangle of a run, the bytes of a run of records and the bytes of a vertex-format tile live in buffers the renderer keeps and clears between batches (`TerrainRenderer::batch_scratch_capacity` stays constant over frames of the same or a smaller size), because the frame thread shares the heap with the generation threads and the time of an allocation there varies with their load. The managed loop SHALL keep its list of waiting tiles and its batch between frames, SHALL rebuild the draw class of the upload ranking (the draw tiles with their ancestors and plan priorities) only when the draw wants changed and the prefetch closure only when the prefetch wants changed (`ManagedLoop::ranking_rebuilds`), and SHALL rank the residency class by looking the tile up in R, which is closed under parents, never by building a set over R (R is sorted and closed under parents, `ResidencySet::is_sorted_and_closed`, which the loop checks in debug builds; it costs the size of R, about 11,000 tiles at the ground), so that a frame whose wants stayed builds no set over them whatever it uploads, and the draw walk SHALL stop at an ancestor already holding a priority at most the walked tile's (the sets equal those of the full walk), so that the ranking costs the draw set's size and not its size times the depth.

Verify: A, C
Status: active
Source: CON-20, CON-21, STRM-006, REND-013, REND-016, ADR 0033 point 9, TEST-018, perf window 1 (F5)

#### Scenario: Calls follow runs
- **WHEN** a frame writes 1, 10 and 118 compact tiles into adjacent slots, from the middle of a row, every third slot, and in a shuffled order
- **THEN** the batched path makes the number of runs of adjacent slots in calls (118 adjacent tiles in an atlas 8 slots wide: 16 calls against 236 per slot; in the product atlas, 124 slots wide, 2 calls), never more than the per-slot path, and the same staging bytes

#### Scenario: Same pools and images
- **WHEN** the same tiles are placed per slot and batched, including a slot written twice in one batch and the ten standard views with levels 0-2 in the vertex pool
- **THEN** every atlas and the slot records read back equal, the images are identical, and a batch that fails writes the tiles placed before the error

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

### Requirement: REND-024 Lit plan
The `FramePlan` (`TerrainPlan`) SHALL carry the lit path's decisions in an optional `LitPlan`: the exposure mode (fixed with its EV100, or adaptive with its range, speeds and the CPU-chosen pre-exposure EV100), `dt` in seconds from the injected clock, the sun direction, a reset flag, the tone-map mode, the look's scene-unit scale and histogram axis and band (LOOK-003), and the base shading's albedo table, water level and debug view (ADR 0039). The sun direction comes from the look's sun (LOOK-001) or from a time of day: `sun_direction(hours, declination)` is the unit vector `(cos d cos l, cos d sin l, sin d)` with `l = -15 degrees (hours - 12)`, in COORD's axes (+z north pole, +x at longitude 0, +y at 90 degrees east), decided in f64 with `libm` and rounded once to f32. Its text form SHALL be exact (floats as bit patterns) and round-trip, a plan without it SHALL keep its text and hash, and a lit plan made with the Earth-default look SHALL keep the text it had before the look file existed (the look's tokens, the albedo table, the water level and the debug view are written, in that order, only when they differ from the default). A plan with it SHALL be refused by a renderer that has not enabled the lit path, with an error naming `enable_lit`.

Verify: A
Status: active
Source: ADR 0042 point 3, ADR 0048 point 3, CON-03, CON-04, CON-19

#### Scenario: Round trip
- **WHEN** a fixed and an adaptive lit plan are written to text and read back
- **THEN** they are equal

#### Scenario: Debug path unchanged
- **WHEN** a plan has no lit part
- **THEN** its text has no `lit` line and equals the text it had before, and a lit plan has exactly one more line

#### Scenario: Not enabled
- **WHEN** a lit plan is rendered by a renderer without the lit path
- **THEN** the render fails with an error that names `enable_lit`

#### Scenario: Sun from a time of day
- **WHEN** the sun is taken at 12:00, 18:00, 6:00 and 0:00 at declination 0, at declination 90, and at assorted times and declinations
- **THEN** it is +x, -y, +y and -x, +z, a unit vector each time, and a time 24 hours later gives the same direction

### Requirement: REND-025 Lit terrain radiance
The terrain fragment stage of the lit path SHALL compute radiance in units of the sun's illuminance as Lambert diffuse `albedo x max(n . sun, 0) / pi` of the normalised shading normal under the plan's sun direction (ADR 0039 point 4: no ambient, no terrain shadows, no specular; the sky's light comes with ADR 0040), where the albedo is `AlbedoParams::albedo` of the climate at the fragment, the cosine of the slope (the angle between the normal and up) and the height above the plan's water level W (ADR 0039 point 3, LOOK-001's `albedo` table): ground from the climate (desert, grass, forest, tundra), bare rock on steep ground, snow by temperature and slope, a beach band above W and the seabed below it. It SHALL multiply by the plan's pre-exposure factor and clamp each channel to [0, 60000] (ADR 0038 point 1). Debug views (CON-18) of the plan SHALL show the albedo alone, the lighting alone (a white surface), the temperature and the precipitation (false colours, written as if lit by a sun facing the surface). Class: presentation data (ADR 0035). Bound: per channel `|GPU - f64 reference| <= max(2e-4 |v|, 2e-5 x factor)` on probe inputs that include unnormalised normals, climates over the whole range, heights above and far below the water, grazing and back-facing suns, two albedo tables and pre-exposures that reach the clamp (the reference sees the f32 inputs and the f32 table the shader receives).

Verify: B
Status: active
Source: ADR 0038 point 1, ADR 0039 points 1, 3 and 4, ADR 0035, ADR 0043, CON-11, CON-18

#### Scenario: Probe set
- **WHEN** 970 probe inputs in all five views are evaluated by the shader's own function (`shade_lit`) and by `look::shading::terrain_radiance` with `look::written_value`
- **THEN** every channel is within the bound, and the set covers clamped, dark and lit results and every view

#### Scenario: Known answers
- **WHEN** a flat warm dry surface, a humid one, a wet one, a steep one, a cold flat one, a cold steep one, a shore and a seabed are evaluated, and a surface faces the sun, faces away or is tilted by 60 degrees
- **THEN** the albedo is exactly the desert, grass, forest, rock, snow, rock, beach and seabed colour, halfway through a transition it is halfway between the two colours, and the radiance is albedo/pi, zero and half


### Requirement: REND-026 Lit terrain frame
A plan with a `LitPlan` SHALL be drawn into the `Rgba16Float` scene target with the same geometry and the same reversed-Z `Depth32Float` test as the debug path (the same pixels covered), every written value finite and within [0, 60000], and tone mapped into the renderer's colour target; the debug path of the same renderer SHALL stay byte-identical, before and after lit frames and compared with a renderer that never enabled the lit path. A lit frame at a fixed exposure SHALL be a pure function of its plan; the sun and the exposure SHALL come from the plan. Lit frames of the orbit, aerial and ground views, the EV-band view and the base shading's debug views (temperature, precipitation, albedo and lighting) SHALL match the adapter's goldens (class lit, first goldens of M5: blessed by `golden-review` under ADR 0037's policy, escalated to the owner's `/bless` otherwise). After `enable_lit` no frame SHALL create a pipeline, target or ring.

Verify: C
Status: active
Source: ADR 0038, ADR 0042 points 1-3, ADR 0037, ADR 0048, CON-12, CON-18, REND-017

#### Scenario: Coverage and range
- **WHEN** the scripted views are drawn in the vertex and compact formats on the lit path and on the debug path
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

## ADDED Requirements

### Requirement: REND-027 Climate and slope reach the fragment stage
Both tile formats SHALL deliver the tile's climate field (GEN-021) to the fragment stage: the compact tile's fragment stage reads the 9 x 9 texture from the fourth channel of the atlas texels of the tile's slot (the tile reference and the atlas are visible to the fragment stage) and interpolates it bilinearly at the fragment's tile coordinates; the vertex format's GPU vertex is 48 bytes (the 44-byte vertex and the vertex's climate as two f16, `MeshVertex::GPU_STRIDE`; 58,608 B per tile at cells 32), the value at each vertex being the stored texture sampled at the vertex (`TileMesh::gpu_climate`) and interpolated across the triangles. The node record (208 bytes) SHALL carry the unit up vector of the tile origin and the inverse of its distance from the planet centre, decided in f64 and rounded once; the fragment stage SHALL form up as `normalize(up0 + local x inv_r)` from the tile-local position, so no planet-radius value reaches the stage (CON-11 v1.4). Class and bound of each attribute (ADR 0035): the stored climate texture is a **field** (GEN-021: the GPU decode is exact); the climate at a fragment is presentation data, the bilinear interpolation of the stored texels (the climate views agree with the colour of the analytic climate at the point under each pixel with a mean error of at most 0.02 on the orbit view and 0.005 on the 20 km view (at most 3 % of pixels, the limb, more than 0.1 off), and the two formats shade the same picture: at most 0.5 % of covered pixels differ by more than 5 % of a lit unit); the up vector is presentation data within 5e-7 rad of the exact radial direction at every tile-local offset up to 1 km from the origin of a tile at 8,848 m or at -11,000 m, anywhere on the planet; temporal stability at 1 m (CON-11): the slope input changes by at most 3e-7 between fragments 1 mm apart at the worst case (a finest-level tile far from the origin), the albedo selected from it by at most 1/255, two tiles that cover one point agree about up within 5e-7 rad, and the shading depends only on tile-local quantities, never on the camera; the albedo, the radiance, the sun direction (f32, 6e-8) and the water level (f32) are presentation data with the bounds of REND-025 and LOOK-003.

Verify: B, C
Status: active
Source: ADR 0039 points 2 and 3, ADR 0035, ADR 0043, ADR 0033 point 4, CON-11, CON-14, REND-016

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

