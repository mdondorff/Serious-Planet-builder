## Context
Inputs: ADR 0077, SPIKE-34 (`spikes/block-stage/results/results.md`; "block" there is "chunk"), `docs/domain.md`, the M8 plan (criteria 2, 3, 7). SPIKE-34's prototype (`spike/block-stage`, never merged) is a reference for shapes, not code to merge.

## Decisions

### 1. Refactors first
- **R1.** The field chain (base, chunk stage, stamps, water bound) and the world's sources move from `app/worldfield.rs` and `generators::build_field` into one GPU-free module in `generators`; `app` keeps wiring. Done first so the chunk stage has one home. Before writing code, re-read the two assemblies (`app/worldfield.rs:661-700`, `generators::build_field`) and list what differs.
- **R2.** `HeightField` is split into three traits: **value** (`height`, batch forms, `height_at_level`), **key** (seed, implementation id, version, inputs hash, read records, `reads_cells`) and **bounds** (`max_abs_height_m`, `height_bounds`). The 13 implementations change once; the level-aware height lands in the same code. `height_at_level(dir, level)` defaults to `height(dir)`, so every existing field is level-independent and unchanged.

### 2. Scope and key
`Scope::Chunk(ChunkId)` with `words() = [2, id]`. `ChunkId` is face, level 11 and (i, j) of the tile grid, so a chunk is the area of a level-11 tile (about 5 km). The grid is part of the field's definition: the stage key covers grid level, halo in metres (as bits) and step cap (ADR 0077 points 2 and 4); a change is a new field version. `scope_fits` allows `Regional` with `Chunk`; `Regional` with `Tile` stays for tile-scoped fields (ADR 0022's halo fraction).

### 3. Apply level
Per chunk field: the apply level is the coarsest level whose vertex spacing is at most twice the chunk's cell size, computed from the raster (8 m cells give level 15, spacing 9.6 m; 4 m cells give level 16). Tiles above the apply level are identity on the base and key as the base (STRM-029); tiles at or below read the chunk. It is not a tunable: it follows from the cell size, so changing a cell size cannot leave a pop.

### 4. Pyramid and level-aware height
A chunk's result is stored as a change against the base on the chunk's raster, plus a box-filtered pyramid down to the apply level (each level box-filtered from the one below, never from a coarser run: ADR 0077 consequences). `height_at_level(dir, l)` for `l >= apply_level` reads the pyramid level whose spacing matches level l and adds it to the base; for `l < apply_level` it is the base. Target (SPIKE-34 section 3): step against the parent at the parent's vertices p99 at most 0.25 m (criterion 3). A process with sharper detail than the smooth stand-in may exceed it; the test measures with the stand-in and M8-3 re-measures with A-net.

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
