## Context
Inputs: ADR 0077, SPIKE-34 (`spikes/block-stage/results/results.md`; "block" there is "chunk"), `docs/domain.md`, the M8 plan (criteria 2, 3, 7). SPIKE-34's prototype (`spike/block-stage`, never merged) is a reference for shapes, not code to merge.

## Decisions

### 1. Refactors first
- **R1.** The field chain (base, chunk stage, stamps, water bound) and the world's sources move from `app/worldfield.rs` and `generators::build_field` into one GPU-free module in `generators`; `app` keeps wiring. Done first so the chunk stage has one home. Before writing code, re-read the two assemblies (`app/worldfield.rs:661-700`, `generators::build_field`) and list what differs.
- **R2.** `HeightField` is split into three traits: **value** (`height`, batch forms, `height_at_level`), **key** (seed, implementation id, version, inputs hash, read records, `reads_cells`) and **bounds** (`max_abs_height_m`, `height_bounds`). The 13 implementations change once; the level-aware height lands in the same code. `height_at_level(dir, level)` defaults to `height(dir)`, so every existing field is level-independent and unchanged.

### 2. Scope and key
`Scope::Chunk(ChunkId)` with `words() = [2, id]`. `ChunkId` is face, level 11 and (i, j) of the tile grid, so a chunk is the area of a level-11 tile (about 5 km). The grid is part of the field's definition: the stage key covers grid level, halo in metres (as bits) and step cap (ADR 0077 points 2 and 4); a change is a new field version. `scope_fits` allows `Regional` with `Chunk`; `Regional` with `Tile` stays for tile-scoped fields (ADR 0022's halo fraction).

### 3. Apply level
Per chunk field: the apply level is the coarsest level whose vertex spacing is at most twice the chunk's cell size, computed from the raster (8 m cells give level 15, spacing 9.6 m; 4 m cells give level 16). The apply level is where the raster itself is read unfiltered. The pyramid reaches from there up to a top level (decision of opus-planner, 2026-10-24, option a: default the chunk grid level 11, one chunk per tile at most); tiles above the top level are identity on the base and key as the base (STRM-029), the global run carrying the large scale. The apply level is not a tunable: it follows from the cell size, so changing a cell size cannot leave a pop. The top level is a parameter of the chunk field (`ChunkField::with_top_level`).

### 4. Pyramid and level-aware height
A chunk's result is stored as a change against the base on the chunk's raster, plus a box-filtered pyramid from the raster up to the top level (each level box-filtered from the one below, never from a coarser run: ADR 0077 consequences). `height_at_level(dir, l)` for `l >= top` reads the pyramid at the continuous level log2(vertex spacing of l over the cell size), a blend of the two pyramid levels around it, Catmull-Rom across the cells, and adds it to the base; for `l < top` it is the base. A linear read of a coarse level errs by up to about 1 m on the stand-in; the spline removes it. Target (SPIKE-34 section 3): step against the parent at the parent's vertices p99 at most 0.25 m (criterion 3). A process with sharper detail than the smooth stand-in may exceed it; the test measures with the stand-in and M8-3 re-measures with A-net.

### 5. Plan-side requests and bounds
The plan builder returns a set `B` of chunk ids: those under its nodes at or above the apply level, in plan order. Heights sampled during planning never request chunks (SPIKE-34 section 1: naive sampling asked 435 to 739 chunks for the 2 to 3 needed). Plan bounds for a chunk field widen by the field's stated bound (`chunk_bound_m`; stand-in: 2 x amplitude) whether or not a chunk is ready, so ready or not gives the same plan. A tight bound from a ready chunk's grid minimum and maximum is the last step (M8-2e) and only if it keeps the plan a pure function of the position and the world version; otherwise it is cut. The origin heights cached for tiles under a landed chunk are invalidated (`invalidate_tiles`).

### 6. Stand-in, landing and fade
A tile whose chunk is not ready is built from the base and keyed with chunk word 0 ("not ready"), so a stand-in and an exact tile never share a key (SPIKE-34 section 6). When the chunk lands, tiles under it are refreshed through the stale-tile machinery (REND-032: the old tile draws while the refresh runs). The change fades in: the tile carries a chunk-change weight `w` (a plain f32, so CON-11 holds), the shader draws `base + w x change`, and the renderer raises `w` from 0 to 1 over `chunk_fade_ms` (default 500 ms, injected clock) per tile after the landing. The fade length is a proposal for the owner's look review (M8-4). Whether `w` lives in the compact tile or in a per-tile uniform is decided at step M8-2d (the compact slot layout, GEN-025, changes only if a uniform cannot carry it).

### 7. Residency, eviction, disk cache
Chunk jobs are a class beside tile jobs on the injected spawner (CON-04), ordered by distance to the camera; at most `chunk_workers` run at once (default: half the worker pool, so tiles are not starved). A chunk not wanted for `chunk_evict_s` (5 s, injected clock) is evicted from RAM; its bytes count toward STRM-033's caps as their own pool. The disk cache stores a landed chunk under its stage key (STRM-011) in the pack store, verified on read like tiles (CON-05); a verification failure is a miss and the chunk is recomputed. The stand-in compute can model latency (`cost_s_per_km2`) on the fake clock for tests.

### 8. Prefetch
Chunk prefetch reuses STRM-018's velocity (`window_ms`, `look_ahead_ms`): the chunks that the predicted camera's apply-level nodes need, one chunk ahead along the direction of travel, below the wanted set in priority. Rotation prefetches nothing. Expected (SPIKE-34 section 7): at 100 m/s through a 5 km chunk the next chunk is requested 49 s before it is needed.

### 9. The chunk stage's position per layer (D3)
A layer setting `chunk_stage` with two values, default `OnTop`: the chain is `bake -> chunk -> stamps` (edits on top of the eroded ground; an edit never invalidates a chunk). `Under` (`bake -> stamps -> chunk`) is accepted by the key shape (the chunk's input hash would include the stamps within reach plus halo) but its invalidation and runtime are built in M8-7, after the owner confirms D3; until then a layer set to `Under` is refused with an error naming the layer. Only the key shape and the refusal are built here, so the choice stays reversible.

### 10. Stand-in compute
`ChunkStandIn` (SPIKE-34's relaxation: ridged relief of 800, 200 and 50 m, amplitude 30 m, then `steps` 3x3 relaxations with the halo border fixed), deterministic (fixed order, `libm`, one thread per chunk; CON-14), with a halo of `steps x reach` cells. It exists to test the infrastructure and stays behind a test feature; M8-3's A-net implements the same chunk-stage interface (raster in, change raster out, stated halo, step cap, bound).

## Risks and open items
- Real threads: the chunk table is written by landings and read by the plan. The implementation swaps an immutable snapshot so a plan reads one consistent ready set; a test with 1, 2 and 8 threads and a fake spawner checks identical plans for an identical ready set.
- Face edges and poles: chunks across a face edge use the extended cube plane for the halo; a test at an edge and at a pole is part of GEN-038.
- Memory with more channels (drainage, sediment) multiplies bytes per chunk by the channel count; the budget is revisited in M8-3.
- The tight-bound path (section 5) is the one piece SPIKE-34 did not measure; if it complicates plan determinism it is cut and the wide bound stays.

### 11. Measured with the stand-in (step 3b)
Step between a tile level and its parent at the parent's vertices, stand-in of 30 m amplitude, 8 m cells, 8 steps (p99, `tests/chunkfield.rs`; SPIKE-34 section 3 measured 0.13 m at 15 against 14):

| Levels | p99 step |
|---|---|
| 15 against 14 | 0.18 m (criterion 3 holds) |
| 14 against 13 | 0.72 m |
| 13 against 12 | 2.0 m |
| 12 against 11 | 3.3 m |
| 11 against 10 (the top against the base) | 21.8 m |

The steps grow upward because the stand-in's 200 m and 800 m relief is wider than the coarse levels' filters and the top level enters from the base in one step. A real process should carry little at those scales (the global run does), so M8-3 re-measures with A-net; until then a top level above 14 is not shown to meet criterion 3 on the stand-in.

Chunk demand when the pyramid reaches up to level `top` (estimate; the plan's request set B is step 3c): with `tau_px` 6, 1080p and 60 degrees, a node of level L is a leaf from about `spacing(L) / (6 x 1.07e-3)` to twice that; tiles at or below `top` lie within twice the leaf distance of `top`:

| Top level | Radius | Chunks (5 km tiles) |
|---|---|---|
| 11 | 48 km | about 285 |
| 12 | 24 km | about 70 |
| 13 | 12 km | about 18 |
| 14 | 6 km | about 5 |
| 15 | 3 km | 1 to 2 |

At the stand-in's 0.25 s per km2, 285 chunks are about 1800 s of thread time, so criterion 3 (a chunk within 10 s at a 300 m pan) and criterion 7 (speed envelope) decide the top level; the default of 11 is the decision's, a lower one is a parameter.

### 12. Findings of the numerics review (step 3b), carried to 3c and 3d
- **Keys must chain the neighbours' ready state.** A tile near a chunk border reads up to two cells of the neighbouring chunk (Catmull-Rom) when it is ready and the nearest cell of its own chunk when it is not, so its heights depend on the ready state of up to eight neighbours. The tile key of step 3c chains the ready state (content hash or word 0) of every chunk within that reach, not only the tile's own chunk, or a tile built before a neighbour lands is never refreshed and cracks against the neighbour's tiles. A test builds the field with A ready, then with A and B, and requires different heights and different keys for a tile at the border.
- **The tie-break across a face edge.** Near a cube corner the centre of the cell beyond a face edge lies within about 1e-9 cells of a boundary of the other face's lattice (1e-12 at 8 m cells), where the rounding of the direction maths would choose the neighbour; `cell_at` reads a position within 1e-6 cells below a boundary as above it. The known answers of a 16-cell and an 8 m corner chunk pin it.
- **The halo.** One ring more than the step cap gives the chunk's own cells and one apron ring exact; at the cap itself the own cells are exact and the apron is not (tested). A chunk is refused below the rule.

### 13. Step 3c as built
- **Default top level 14** (planner, 2026-10-24, provisional); `PYRAMID_TOP_LEVEL`.
- **Keys.** `ChunkField` is a stage over its base like the stamp stage: `upstream` is the base, `stage_params` is ("chunk", 0), a tile above the top passes through (the base's key). A tile at or below the top carries a `ReadRecord::Chunk` (query kind 5) for each chunk it reads, own chunk first and then the neighbours within the read reach (`2 << pyramid levels` raster cells of its edge), with the state word of the chunk (0 not ready, else the hash of its result with the low bit set). Both key paths (128-bit records and the 64-bit `inputs_hash_for`) read the same records. The process's own definition enters through the result hash, so while no chunk has landed a tile is the base's content under a stage key.
- **Tile generation reads the level.** `generate_tile_field` and `heights_and_climate` (mesh, compact) read `height_at_level` with the tile's level; the plan's origin heights are read per tile level (`heights_threaded_at_level`, which keeps the batch path for fields that are not `level_aware`), so the origin of a tile is the height it is drawn at.
- **Requests.** `NodeSelection::chunk_requests(top)` (the chunks under nodes at or finer than the top, each once, in node order) and `chunk_requests_with(reads)` (a chunk field's `chunks_read_by`, which adds the neighbours); `SelectionCache::invalidate_chunk` drops the cached tiles that read a landed chunk.

### 14. Step 3d as built
- **Jobs.** `streaming::chunks::ChunkManager` (injected spawner and clock): `update(wanted)` notes the wanted ready chunks, evicts the ones not wanted for `evict_ms` (and the least recently wanted beyond `cap_bytes`, the chunk pool), starts jobs for the nearest wanted chunks that are neither ready nor running (at most `workers`), reports what landed, and never waits for a job. A job reads the disk cache first. Tested on `ManualSpawner` (order, cap, no wait) and on a `TimedSpawner` flight at 0.25 s per km2 on 6 workers (every chunk lands, the first within 10 s of the pan, the share falls to zero).
- **Disk.** `cache::chunk_store`: `Kind::Chunk` (8), `ChunkSpec` (body, process hash, grid, top level, radius, base key) gives the stage key, `PackChunkDisk` stores `ChunkResult::to_bytes` in the pack store; a damaged store or entry is an error naming it, so the manager logs it, recomputes and gets the identical result.
- **Share and fade.** `frame::chunkshare`: `ChunkView` (the field's top level, reads and ready set; `ManagerView` over a manager), `ChunkShare` (columns `chunk_nodes`, `chunk_stand_in` of `FrameCounts::chunk`, outside `FrameCounts::NAMES` so the scheduler digests keep their width) and `ChunkFade` (linear weight over `chunk_fade_ms`, restarts on a new landing).
- **Fade limit.** The drawn height `base + w x change` steps by the weight step times the change per frame: 0.25 m per frame at 60 fps needs `max |change| / (0.25 x 60)` seconds of fade, so the 500 ms default fits changes up to 7 m and the stand-in's 30 m needs 2 s. The shader hookup (a per-vertex change channel in the compact format) follows 3e.

### 15. Step 3e as built
- **Prefetch.** `frame::chunkshare::prefetch_chunks(position, predicted, radius)`: the chunk one chunk edge ahead of the camera along its travel over the ground, then the chunk of the predicted position, each when it is not the chunk under the camera; empty for no prediction (stationary, teleport, or only rotating: the estimator never sees rotation) and for a pure dive. `ChunkManager::update_with(wanted, prefetch)` starts prefetch jobs only on workers no wanted chunk needs and treats prefetched chunks as wanted for eviction. At 100 m/s the next chunk is requested a chunk's travel time (about 50 s) before the plan wants it (tested over 60 s of travel).
- **Layer setting.** `WorldDefinition::{set_chunk_stage, chunk_stage, check_chunk_stages}` with `ChunkStagePosition::{OnTop, Under}`; the default is not stored (a definition without a setting hashes as before), `Under` changes the hash and `field_chain` refuses it with the layer named.
- **Tight bound cut (3e.3).** A bound from a ready chunk's grid minimum and maximum would make the plan depend on which chunks are ready, against LOD-013 (a plan with no chunk ready equals the plan with all ready). The wide bound stays: the stage's stated `bound_m`.
- **Flight check.** `oracle_chunk_flight` (tier B): a descent from 30 km to 1.8 m, a hold and a 300 m pan on the fake clock with 6 workers at 0.25 s per km2; every frame's plan equals the plan of the same field with no chunk ready, no more jobs than workers, the share of nodes on the base falls from all to none. The rendered flight on the software adapter follows the shader hookup of the fade.

### 16. The chunk stage in the world (after 3e)
- `field_chain_with(.., Some(&ChunkLayer))` builds base, chunk stage, stamps, water bound (GEN-039 `on-top`); `field_chain` is it with no layer. `impl HeightField for Box<T>` lets the stage wrap the boxed chain.
- `WorldTerrain::enable_chunks(stage, cells_per_chunk, spawner, clock, disk, cfg)` gives a world its `ChunkManager`; every field the world builds afterwards carries the chunks ready at that moment. `chunk_view()` is the `ManagerView` a frame counts its stand-in share against (crusader's `chunk_view_of` in `envelopeperf.rs` returns it), `chunk_frame(tiles, camera, prefetch)` is one frame of the jobs (wanted chunks nearest first, prefetch below them, what landed and what was evicted), and `ManagedLoop::chunk_landed(landed)` refreshes the resident tiles that read a landed chunk through the stale-tile machinery (STRM-040).
- Nothing enables chunks by default: the window and the viewer turn the stage on when M8-3's A-net replaces the stand-in (the stand-in is a test process).
