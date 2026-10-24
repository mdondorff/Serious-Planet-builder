## Why
M8's regional stages (A-net erosion at 4 to 10 m, horizon shadows, driver maps) need inputs over kilometres, so they run per chunk of a fixed grid, not per tile (ADR 0077, CON-08 as amended in v1.6). The engine has no chunk path: `Scope` is `Global` or `Tile`, `HeightField::height` is position-only, and streaming, the plan and the cache know only tiles. SPIKE-34 measured that a chunk field fits the engine under four conditions: an apply level, requests from the plan's nodes, a box-filtered pyramid, and a compute of seconds. This change builds that infrastructure with a stand-in compute, so M8-3 can swap in A-net without touching the engine. Owner go and D7 (chunks are generated at runtime, prefetched, cached on disk): 2026-10-23.

## What Changes (if accepted)
- **R1, R2 first** (`docs/briefings/m8-review.md`): one GPU-free home for the field chain (base, chunk, stamps, water bound) and the world's sources, out of `app`; `HeightField` split into value (with the level-aware height), key and bounds.
- **`Scope::Chunk(ChunkId)`** and the chunk key: the stage key gains the chunk grid (level 11, whole coarse cells), the halo in metres and the step cap (ADR 0077 points 2 and 4).
- **Apply level**: chunk detail applies to tiles at or above the level whose vertex spacing the chunk's cell size supports (level 15 for 8 m cells); coarser tiles are identity on the base (STRM-029's rule).
- **Level-aware height**: `height_at_level(dir, level)`; a tile of level l reads the box-filtered pyramid level of its own vertex spacing, which cuts the step against its parent to p99 0.13 m (SPIKE-34 section 3).
- **Plan-side requests**: the plan emits the chunk set `B` from its nodes at the apply level (beside R and D); sampling a height never requests a chunk.
- **Stand-in and fade**: a tile whose chunk is not ready draws the base with its own key; the landing refreshes it and the change fades in over time.
- **Residency**: chunk jobs beside tile jobs on the injected spawner (CON-04), priority by distance, eviction by need over time, a disk cache keyed by the chunk key, prefetch of the next chunk along the direction of travel (STRM-018's shape).
- **The chunk stage's position per layer** (D3, provisional): a per-layer setting, default "edits on top of the eroded ground".
- **A stand-in compute** (SPIKE-34's relaxation) so the whole path runs and tests before A-net exists.

## Impact
- Specs: new GEN-034 to 039, STRM-038 to 042, LOD-013 (deltas in this change, status planned until each lands); GEN-016/017 and STRM-029 are read, not changed.
- Code (explorer lane): `generators/height*.rs`, `generators/stage.rs`, `app/worldfield.rs`, `frame/lod.rs`, `streaming`. Crusader is on `cache/global_store.rs` and `render`; shared lines (`lib.rs`, specs) in small PRs.
- Cost: R1 and R2 about a day each, M8-2 five steps of at most a day each.
- Out of scope: A-net (M8-3), edits over chunks and the single "which chunks does an edit change" function (R3, M8-7), horizon shadows (M8-5), the speed-envelope scene (M8-V; this change exposes its stand-in share count), pre-baking chunks offline (D7).
- Open risks: a real-thread landing writes the chunk table while the plan reads it (SPIKE-34 used one `RwLock`); the plan's bounds widen by the process bound until a ready chunk's grid gives a tight one (about a fifth of near-ground tiles held back, SPIKE-34 section 4); face edges and poles are untested.
