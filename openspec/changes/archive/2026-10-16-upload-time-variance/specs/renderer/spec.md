## MODIFIED Requirements

### Requirement: REND-018 Batched tile writes
The terrain renderer SHALL write a frame's tiles to the pools in batches: `TerrainRenderer::upload_tiles_at` places the frame's tiles in the order given (the upload order of STRM-006), each exactly as `upload_tile_at` does (the manager's slot, the same checks, errors and bookkeeping), and issues the data writes of the compact tiles after the last one, sorted by slot, with one `write_texture` per run of adjacent slots in one row of one atlas (the run's tiles interleaved row by row into one rectangle) and one `write_buffer` per run of adjacent slot indices for the slot records (across blocks, the records being one buffer); a slot written twice in a batch SHALL hold the last tile. Vertex-format tiles (levels 0-2, fallback tiles, the vertex reference format) SHALL be written at once, one call each, because a call carries 53,724 bytes and the call is not the cost (a coalescing copy would add a pass over the data). The calls a batch makes for compact tiles SHALL be exactly the number of those runs (texel runs plus record runs), so at most the per-slot path's two per tile for any slots, and for adjacent slots the atlas rows touched plus one record run. The reference path (`UploadMode::PerSlot`: `--upload per-slot`, two calls per compact tile) SHALL stay selectable, the default being batched, and the pool contents (every atlas and the slot records, read back), the images and the bookkeeping SHALL be identical in both modes. The renderer SHALL count the queue calls (`write_calls`) and the bytes handed to them (`staging_bytes`) of its tile writes; the managed loop SHALL hand the frame's tiles to its sink as one batch (`TileSink::upload_batch`, whose default is the loop over `upload_at`, one call per tile, so a sink that does not batch keeps working), in the upload order, and report the sink's calls in the frame's counts (`upload_calls`, TEST-012). The tile bytes of the 1 MiB cap (STRM-006) and the placement of the manager SHALL NOT change. The atlas textures and the slot-record buffer carry `COPY_SRC` so that tests can read the pools back (`read_compact_pools`; no change to the allocated bytes). The conversion of a tile to bytes (vertex floats, texels) SHALL NOT go through a byte-wise iterator (about eight times the CPU of copying the same bytes in slices). The staging of a batch SHALL NOT allocate per frame once the buffers have grown to the largest batch: the converted texels, the lists of writes, their sort orders, the rectangle of a run, the bytes of a run of records and the bytes of a vertex-format tile live in buffers the renderer keeps and clears between batches (`TerrainRenderer::batch_scratch_capacity` stays constant over frames of the same or a smaller size), because the frame thread shares the heap with the generation threads and the time of an allocation there varies with their load. The managed loop SHALL keep its list of waiting tiles and its batch between frames, SHALL rebuild the draw class of the upload ranking (the draw tiles with their ancestors and plan priorities) only when the draw wants changed and the prefetch closure only when the prefetch wants changed (`ManagedLoop::ranking_rebuilds`), and SHALL rank the residency class by looking the tile up in R, which is closed under parents, never by building a set over R (R is sorted and closed under parents, `ResidencySet::is_sorted_and_closed`, which the loop checks in debug builds; it costs the size of R, about 11,000 tiles at the ground), so that a frame whose wants stayed builds no set over them whatever it uploads, and the draw walk SHALL stop at an ancestor already holding a priority at most the walked tile's (the sets equal those of the full walk), so that the ranking costs the draw set's size and not its size times the depth.

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
