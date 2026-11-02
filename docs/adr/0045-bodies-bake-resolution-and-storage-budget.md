# 0045 Bodies, bake resolution and the storage budget
Status: proposed (draft by opus-planner, 2026-10-16, at the owner's request; point 3 decided by the owner 2026-10-16, pre-fill levels amended by 0062 and 0063; point 9 added 2026-11-01 for ADR 0088 and ADR 0089 (owner, group 3); the rest is decided when several bodies become a milestone, or earlier if the editor (M6) needs point 7)
Date: 2026-10-16
Source: owner's questions of 2026-10-16 (generation cost, disk per planet, "amount of planets vs quality vs speed", authored regions); M4 perf windows 1 and 2 and sonnet-explorer's measurements (#157, #162); ADR 0006, ADR 0007, ADR 0018, ADR 0019, ADR 0021 (points 2, 5, 6, 8 open), ADR 0026, ADR 0027, ADR 0028, ADR 0029, ADR 0033, ADR 0088, ADR 0089, STRM-065 to STRM-068; constitution CON-05, CON-06, CON-08, CON-14, CON-21

## Context
- **The bake is the per-body world map**, not tiles: four `f32` channels (continentalness, elevation, temperature, precipitation) on a vertex grid of `res` cells per cube-face edge (ADR 0026). At Earth radius and `res` 16384 (about 0.6 km) it takes 370 s on 20 threads and 11.66 GB on disk (perf window 2026-10-12).
- **Tiles are products of the terrain function**, which reads the bake (bilinear and slope stencils, about 94 lattice cells per tile vertex, #157) and adds detail. Finished tiles are kept in the tile pack (ADR 0019, ADR 0029), a cache keyed by content keys (ADR 0018).
- **Coarse tiles are expensive cold.** A level-0 tile reads about 320 bake blocks (5.4 GB decoded), so one-thread generation takes L0 6.6 s, L1 2.1 s, L2 0.74 s, L3 0.35 s, L4 0.21 s per tile (#162). The decode floor is about 21 ms per 16.8 MB block.
- **Scale.** Disk and bake time grow with `res` squared: 16384 is 11.7 GB and about 6 min, 8192 about 2.9 GB and 1.5 min, 4096 about 0.7 GB and 25 s (by scaling, not measured). At a constant 0.6 km cell, the rocky bodies of the Solar System are about 3 Earth surfaces, about 35 GB; at 1.2 km about 9 GB.
- **Everything is deterministic from the definition** (CON-05, CON-14), so bakes and tile packs can be deleted and rebuilt bit for bit. Authored items (ADR 0007) are small TOML files plus blobs; frozen regions store absolute data (M6).
- The owner's framing: a trade-off between the number of planets, quality and speed, with disk as the visible limit. With deterministic caches a fourth axis exists: **disk against first-visit wait**.

## Options (with trade-offs)
1. **Ship every body baked.** Instant first visit; disk grows with the number of bodies and their resolution (tens of GB for a system).
2. **Bakes and tiles are caches with a budget** (proposed): a world ships its definition and authored items; bakes and tile packs are built on first use, kept under a disk budget and evicted per body. The cost is the first visit to an unbaked body (minutes at Earth scale and 0.6 km).
3. **No global stage for minor bodies**: a purely procedural height field (already selectable per world, ADR 0028) needs no bake, with no disk or wait, but loses global coherence (continents, climate, later hydrology).

These options combine: 2 is the default, 3 is a per-body choice, and 1 is a packaging choice of an application.

## Decision (proposed)
1. **Bakes and tile packs are caches, not content.** A world's persistent truth is its definition and authored items (CON-05, ADR 0007). Bakes, pre-filled coarse tiles and streamed tiles are caches under their stage and content keys. They are built on demand and evicted under a **disk budget per installation**: a cap over all bodies, evicting least-recently-visited bodies whole (bake plus tiles), with the current body never evicted. An application may ship caches pre-built (option 1) as a packaging choice; the format is the same.
2. **Bake resolution per body.** A body's `res` is a world-definition parameter with a default from a target cell size: `res = 2^ceil(log2(face_edge / cell))`, with `face_edge` about `pi R / 2` and a default cell of 0.6 km. It is already part of the bake's stage key (ADR 0026 point 6). A body may name the no-bake procedural field instead (option 3). The cell size sets the scale of baked features (continents, climate, erosion at the bake's scale); finer detail is generated live.
3. **Coarse pre-fill (owner, 2026-10-16).** The bake step also writes tile levels 0 to N (default 4) into the tile pack, with the existing keys, format and codec; they are evicted with the body's bake. Estimate at Earth scale: 2,046 tiles, about 25 MB (uncompressed upper bound), about 1-2 min on 20 threads, likely under a minute with a block-ordered pass. The first real run gives the figure. M4's gated dive starts from this state; the cold dive is a tracked, report-only case.
4. **Storage encoding: decided by a spike before the multi-body milestone.** Today it is four `f32` channels, byte-shuffled, LZ4 (STRM-009). Candidates are narrower channels with stated bounds (temperature and precipitation fit 8-16 bits; elevation needs its CON-14 bound), stronger compression (zstd) and predictive coding of the smooth fields; the expected gain is 3-6x (unmeasured). The spike measures size, decode time per block against the 21 ms floor, and bake time. A quantised channel is the field's canonical value (a `GLOBAL_VERSION` bump with new known answers), never a lossy cache of an `f32` truth.
5. **First visit and readiness.** A bake cannot start coarse and refine, because a different `res` is a different field. A first visit therefore bakes the blocks nearest the camera first, so the viewer starts when the blocks of the current view exist and the rest follow in the background. A GPU-free **readiness report** (bake blocks done and total, pre-fill tiles done and total, streaming coverage of the current view from the detail-lag measure) lets an application show a loading screen or start early with coarse terrain. The M5 viewer shows it in the window title; a plug-in interface waits until an application needs one.
6. **Budget figures** (estimates, Earth radius, today's encoding):

   | cell | `res` | bake disk | bake time (20 threads) | pre-fill 0-4 |
   |---|---|---|---|---|
   | 0.6 km | 16384 | 11.7 GB (measured) | 6 min (measured) | ~25 MB, 1-2 min |
   | 1.2 km | 8192 | ~2.9 GB | ~1.5 min | same tile count, faster per tile |
   | 2.4 km | 4096 | ~0.7 GB | ~25 s | same |

7. **Authored edits against pre-filled coarse tiles.** A coarse tile spans thousands of authored cells and today records the whole body in its key (ADR 0021 point 5), so every edit would re-key all pre-filled coarse tiles. **Before the editor ships**, ADR 0021 point 5 is decided with region summary hashes (a quadtree of cell hashes with fixed empty hashes per level), so one edit invalidates about one coarse tile per level (about 10 s of background regeneration at Earth scale, the stale tile shown meanwhile).
8. **Global-scale authoring and hydrology are out of scope here.** Edits that change global context (moving a range that changes rain shadows, a lake that changes ocean connectivity) would re-bake the affected global stage. They are decided with hydrology, whose global data also adds per-body storage (and is where the knobs above matter most).
9. **The M9 layers (ADR 0088, ADR 0089)**, per body, caches under point 1 (rebuilt when absent, evicted with the body's bake); values are start values in the specs:
   - the erosion layer, a separate store beside the bake that never rewrites it (about 7-10 GB at 16384; STRM-065), keyed by the bake key and the channel versions (ADR 0088 point 7), so a class-paint edit invalidates it where the weights change;
   - its min/max pyramid (about 90 MB; STRM-066);
   - the class-weights channel, stored in the bake's store and rebuilt from the seed and the class-paint items, which are authored content in the definition (STRM-067);
   - the relief channels `k` and `m` with block ranges, together with the weights under 1 GB (STRM-068).

## Consequences
- A system of many bodies costs disk only for the bodies visited recently; the price is a first-visit wait that the readiness report makes visible.
- Knobs and where they live: `res` per body and the field choice (world definition, easy); the disk budget and eviction (cache policy, easy to moderate: ADR 0029 has the size cap and compaction); the pre-fill depth N (parameter); the codec (one codec version and an ADR, moderate); the finest level, tile size, tau and the VRAM share (LOD and budget parameters, ADR 0033). The cube-sphere tiling, precision rules, determinism and the stage model are fixed and do not limit the number of bodies.
- Tests when built: pre-filled tiles bit-identical to on-demand tiles; eviction never removes the current body; a re-bake after eviction reproduces the stage key and every block hash; the readiness report counts against a fake clock.

## Licences of adopted code or techniques
None yet; zstd (BSD-3-Clause) would be recorded if adopted.
