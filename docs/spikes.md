# Spike register

Time-boxed experiments that answer a question before a decision. Opened and closed with `/spike`. Spike code lives in `spikes/<name>/` and is never merged into the main crates as-is.

Planned spikes from the background report (open them when their milestone starts):
- GPU-driven indirect terrain/instance path in wgpu. Milestone: M2. (SPIKE-01, closed 2026-10-10.)
- CDLOD morph quality on steep terrain. Milestone: M2. (SPIKE-02, closed 2026-10-10 for smooth terrain; steep terrain remains open for M3.)
- Global hydrology on the sphere (stream-power on a 1–4 km grid + halo-based local carving). Milestone: M3. Fallback: local erosion filter + rivers on a coarse flow grid.
- City HLOD with 10^5 buildings. Milestone: M7.

## SPIKE-05 Residency counts
Status: closed
Opened: 2026-10-15
Closed: 2026-10-15
Question: How many nodes, tiles and bytes (today's 44-byte vertex) does each standard view need resident for (i) today's frustum-cut draw set and (ii) the position-based surroundings set (no frustum cut, horizon cut kept), at finest level 26, 23 and 22 and at cells 32 and 64; how much of what is drawn produces no visible pixel; what does a tile cost to generate; and how long does the plan take, apart from streaming and upload?
Why it matters: ADR 0031 (residency set versus draw set, request order), ADR 0033 (terrain's VRAM share, pooled slots, finest level), ADR 0032 (teleport plan time). Replaces the hand estimates in ADR 0033's context (about 50 KB per tile, 0.4 GB for the ground-1m draw set, 3-6x for the residency set).
Time box: half a day plus an hour.
Success criteria: a table of nodes and MB per standard view and set, at the three finest levels and cells 32 and 64; the morph interval rechecked at 64 on the eroded field; the hidden-triangle share at ground-1m; tiles per second per thread for the eroded field and `cpu-terrain-v1`; plan time reported apart from streaming time.
Fallback if it fails: n/a (counts).
Code location: `crates/app/examples/residency_counts.rs`, a measurement helper and not production code: it calls the production planner (`plan_terrain_cached`), the production selection (`select_nodes_field_cached`) and the production mesh builder, and adds a small CPU depth pass. Re-run: `cargo run --release -p planet --example residency_counts -- --store DIR --global-res N [--mem-res M] --part counts|hidden|throughput` (needs a bake made by `generate --global-dir`).
Result (2026-10-15, main at 5543a6f, one session; the 360-line count listing was `target/spike05/counts.txt`, not kept; the tables below are the result):
- **Which field.** Every count, every plan time and the store-backed throughput figures use **the eroded field (`cpu-erosion-v1`, GEN-017, the world's default since #110) over the stored 16384-cell global bake** (the M3 acceptance bake, seed 1, read through `StoreSource`). `cpu-terrain-v1` over the same bake was counted as a second field: within 1 % of the eroded counts (ground-1m draw 8,735 against 8,763 nodes), so only the eroded one is tabled. The hidden-triangle pass and the "in-memory" throughput figures read a 1024-cell in-memory bake instead (the store's block cache is a mutex, see the throughput bullet); their draw sets differ from the tabled ones by under 0.1 % (ground-1m 8,757 nodes). The perf scene's reference field (`cpu-ref-height-v2`, what the perf windows plan) was not used. Camera and LOD as the perf scene: the ten scripted views, 2560x1440, vertical fov 60 degrees, tau 6 px (viewport / tau 240), horizon cut on, frustum cut padded by `max_height_m` as in production. Bytes = distinct tiles x vertices x 44 B: 1,221 vertices (53.7 KB) per tile at cells 32, 4,485 (197 KB) at cells 64; the shared index buffer is not counted. Nothing here is a perf-window measurement.
- **Sets at cells 32, finest level 26, per standard view** (draw set = what `plan_terrain_cached` draws today; surroundings = `select_nodes_field_cached`, no frustum cut):

| view | draw nodes | draw MB | surroundings nodes | surroundings MB | surroundings / draw |
|---|---|---|---|---|---|
| orbit-20000km | 52 | 2.8 | 52 | 2.8 | 1.0 |
| high-2000km | 429 | 23.0 | 697 | 37.4 | 1.6 |
| aerial-200km | 655 | 35.2 | 1,969 | 105.8 | 3.0 |
| aerial-20km | 1,106 | 59.4 | 3,192 | 171.5 | 2.9 |
| low-2km | 2,625 | 141.0 | 4,781 | 256.9 | 1.8 |
| ground-200m | 4,498 | 241.7 | 6,514 | 350.0 | 1.45 |
| **ground-1m** | **8,763** | **470.8** | **10,751** | **577.6** | **1.23** |
| north-pole-1km | 2,562 | 137.6 | 4,188 | 225.0 | 1.6 |
| cube-corner-1km | 3,617 | 194.3 | 5,949 | 319.6 | 1.65 |
| face-edge-1km | 3,572 | 191.9 | 5,868 | 315.3 | 1.65 |

- **Finest level 26 / 23 / 22, cells 32.** Only ground-1m is deep enough to change (every other view stops at level 19 or coarser). The spacings of levels 22 and 23 are about 7.5 cm and 3.7 cm (queued SPIKE-07).

| ground-1m, cells 32 | L26 | L23 | L22 |
|---|---|---|---|
| draw nodes / MB | 8,763 / 470.8 | 7,212 / 387.5 | 6,639 / 356.7 |
| surroundings nodes / MB | 10,751 / 577.6 | 9,200 / 494.3 | 8,627 / 463.5 |

  Capping at 23 saves 18 % of the nodes and bytes at ground-1m, capping at 22 saves 24 %; the node count is much less sensitive to the finest level than to cells.
- **Cells 64.** A tile edge twice as long: a quarter of the nodes at the same level, 3.7x the bytes per tile. The same level number means half the vertex spacing, so the fair comparison is equal spacing (cells 64 at L25 / L22 / L21 against cells 32 at L26 / L23 / L22):

| ground-1m, equal vertex spacing | cells 32 draw nodes / MB | cells 64 draw nodes / MB | cells 32 surroundings | cells 64 surroundings |
|---|---|---|---|---|
| finest 26 (cells 32) = 25 (cells 64) | 8,763 / 470.8 | 2,693 / 531.4 | 10,751 / 577.6 | 3,294 / 650.0 |
| finest 23 = 22 | 7,212 / 387.5 | 2,216 / 437.3 | 9,200 / 494.3 | 2,817 / 555.9 |
| finest 22 = 21 | 6,639 / 356.7 | 2,039 / 402.4 | 8,627 / 463.5 | 2,640 / 521.0 |

  Cells 64 cuts the nodes (draw calls, per-node CPU work) by 3.3x at equal spacing, with equal triangles (20.2 M at 32/L26, 23.4 M at 64/L25) and 13 % more bytes (coarser granularity: more tile area at the view's edge is resident). At the same level number (half the spacing) cells 64 L26: aerial-20km 359 draw nodes / 70.8 MB, ground-200m 1,395 / 275.3 MB, ground-1m 2,744 / 541.5 MB.
- **Morph interval at cells 64 on the eroded field: holds.** `LodParams::validate` accepts cells 64 at viewport / tau 240 (the ratio must exceed about 2 x cells = 128), and all 360 plans (two fields, nine parameter sets, ten views, both sets) are accepted, none rejected for steepness; the largest `morph_start / morph_end` of any node is 0.700, the base value 1 - `morph_band`, so no height-extent extension of the interval was needed at 32 or 64. Caveat: the 16384 bake is about 1.2 km per cell, as in the M3 acceptance check.
- **Plan time, apart from streaming and upload** (single thread, eroded field, 16384 store, ad hoc on the 20-thread laptop, not a perf window; cold = fresh `SelectionCache`, warm = the same request again; ms):

| view | cells | draw plan cold / warm | surroundings selection cold / warm |
|---|---|---|---|
| ground-1m | 32 | 184 / 20.0 | 276 / 5.8 |
| ground-1m | 64 | 68 / 5.9 | 90 / 1.65 |
| ground-200m | 32 | 121 / 9.7 | 217 / 2.9 |
| aerial-20km | 32 | 59 / 2.0 | 188 / 1.7 |
| aerial-200km | 32 | 867 / 728 | 2.3 / 0.7 |
| high-2000km | 32 | 5,337 / 5,708 | 1.8 / 0.3 |
| orbit-20000km | 32 | 6,492 / 6,270 | 0.1 / 0.03 |

  (`cpu-terrain-v1`, ground-1m draw plan: 81 cold / 11.0 warm at cells 32, 30.5 / 3.6 at 64.) Reading: (1) the selection is cheap (surroundings, warm, ground-1m: 5.8 ms, in line with the perf windows' 6.6 ms for the reference field). The **draw plan costs 20 ms warm** because `plan_terrain_cached` also evaluates, per node, the field height at the tile centre and the mesh key; by difference that is about 14 ms for 8,763 nodes (1.6 us each, which is the eroded field's cost per sample, measured below), over the 8 ms plan budget. A tile's origin and key are pure functions of the tile and belong in the resident tile, not in the per-frame plan. (2) At altitude the draw plan takes **seconds** (orbit 6.5 s for 52 nodes, high-2000km 5.3-5.7 s, aerial-200km 0.7-0.9 s) while the selection takes under 2.5 ms. With so few nodes the cost can only be the mesh key of the coarse tiles, which reads the bake cells of the tile's footprint (a level-2 tile covers about 16 M cells of this bake) and is not cached across plans. That is inference by elimination, not profiled. The perf windows never saw it because the reference field reads no bake. (3) Cold ground plans (60-280 ms) include first block decodes of the store's block cache; the 25-29 ms cold figure of the perf windows was the reference field. A teleport frame's plan alone is therefore several frames, before any tile is generated (ADR 0032).
- **Generation (streaming) time, apart from plan time.** One tile at cells 32 as `tile_mesh_field` builds it (heights, normal stencil, morph targets, skirts): eroded 8.8 ms (113 tiles/s per thread) over the 16384 store and 5.7 ms (174/s) over a 1024-cell in-memory bake; `cpu-terrain-v1` 4.2 ms (241/s) and 3.7 ms (271/s). At cells 64: eroded 35.3 ms (28/s; 22.1 ms, 45/s in memory), terrain 16.6 ms (60/s; 14.6 ms, 69/s): four times the cost for four times the area, so samples per second stay constant (about 120k per thread for the eroded field over the store, 190k in memory, 255k for terrain). Bare height samples: eroded 1.0-1.7 us, terrain 0.66-0.79 us. On 20 threads over the in-memory bake: eroded 1,217 tiles/s at cells 32 (357/s at 64), terrain 1,530 (523/s); the by-hand 3,800 tiles/s (review item F) was 3x too optimistic for the eroded field. **Over the store-backed 16384 bake the same 20 threads manage only 22 tiles/s (eroded) and 100 (terrain), five times slower than one thread**: `StoreSource` takes one mutex for every lattice read (a 16-block LRU of up-to-16 MB blocks, `crates/cache/src/global_store.rs`). From empty, the ground-1m draw set (8,763 tiles) takes about 7.2 s on 20 threads with a lock-free source (surroundings, 10,751 tiles: 8.8 s), about 77 s on one thread, and about 400 s through `StoreSource` as it is. GPU upload time of the buffers needs the real adapter: **not measured, needs the architect's perf window** (the volume is the MB column).
- **Hidden triangles** (a CPU depth pass over the drawn tiles at 2560x1440: pixel-centre sampling, exact depth equality, near-plane clipping, no backface culling, unmorphed vertex positions, self-checked on a toy scene; eroded field, 1024-cell in-memory bake; any adapter):

| view, cells 32 L26 | drawn triangles | visible | hidden share | tiles with no visible triangle | tiles a tight frustum cut would keep |
|---|---|---|---|---|---|
| ground-1m | 20.18 M | 0.77 M | **96.2 %** | 7,631 of 8,757 (87 %) | 2,913 (33 %) |
| ground-200m | 10.35 M | 0.78 M | 92.4 % | 3,391 of 4,492 (75 %) | 1,702 |
| low-2km | 6.03 M | 0.87 M | 85.6 % | 1,654 of 2,619 (63 %) | 1,287 |
| aerial-20km | 2.52 M | 0.91 M | 63.8 % | 343 of 1,094 (31 %) | 889 |
| ground-1m, cells 64 L26 | 23.86 M | 0.90 M | 96.2 % | 2,362 of 2,741 (86 %) | 934 |

  `cpu-terrain-v1` gives the same shares within 1.5 points (ground-1m 96.3 %). At ground-1m 63 % of the pixels are covered and 8.7 drawn triangles fall on each, but only 0.33 visible triangles per covered pixel: **the 5.4 triangles per pixel of the review are hidden terrain, not over-tessellation at grazing angles** (the visible triangles are about 3 pixels each, as tau 6 px intends). The last column tests each drawn tile's own bounding sphere (the exact radius of its vertices) against the camera's side planes: it loses no tile that has a visible triangle in any row and would drop 67 % of the ground-1m tiles. **Today's frustum cut removes almost nothing near the ground**: `select_impl` pads every node's sphere by the global `max_height_m` (9 km or more) before the plane test, so the draw set is already 81 % of the position-based set at ground-1m. A conservative per-tile bound (LOD-008's height bounds) needs less padding than 9 km, though more than the exact radius used here.
Recommendation (for ADR 0031 and ADR 0033):
1. **ADR 0031: adopt position-based residency; it is cheap.** The surroundings set is 1.23x the draw set at the worst view (ground-1m: 10,751 nodes, 578 MB at today's vertex, cells 32, finest 26; 463 MB with the finest level capped at 22) and at most 3x where totals are small (172 MB at aerial-20km); the by-hand 3-6x was too high. It needs no prediction margin for rotation. Size ADR 0033's share against the ground-1m surroundings, 0.58 GB, with the 2 GB and 3 GB starting shares leaving room. A flight also holds the tiles of earlier frames; that union is not counted here and belongs in the residency manager's tests.
2. **Tighten the draw set's frustum cut** (a change to LOD-007, queue it separately): per-tile height bounds instead of the global `max_height_m` pad would cut about two thirds of the ground-1m draw calls with no visible tile lost, more per unit of work than cells 64, and it makes the draw set a true subset of the residency set. Occlusion would remove most of the rest (87 % of tiles have no visible triangle) but conflicts with CON-03; it stays the owner's decision.
3. **ADR 0033: cells 64 is a real option for the CPU** (3.3x fewer nodes, warm plan 5.9 ms instead of 20) at +13 % bytes and a four times coarser tile (35 ms to build); the morph interval holds at 64 on the eroded field. Decide it together with the slot size of the pooled allocator (197 KB at cells 64, 54 KB at 32). A finest-level cap of 22 or 23 saves 18-24 % of the ground-1m tiles and bytes; whether it is worth the look is for SPIKE-07's frames.
4. **ADR 0032: plan time is its own term.** The eroded field's draw plan is 20 ms warm at ground-1m, 180 ms cold, and seconds at altitude over the 16384 bake; none of it is streaming. Origin and mesh key are computed once per resident tile, not per frame, and the plan's cost has to be bounded before a detail-lag budget means anything.
5. **Generation throughput sets the teleport lag, and `StoreSource` is the bottleneck for the production bake.** 1,217 tiles/s (eroded, 20 threads, lock-free source) fills the ground-1m draw set in about 7 s; through the mutex-guarded store source it takes minutes. Fix the source (per-thread block cache or a shared lock-free one; the regional-field stage with halos, ADR 0026 point 5, is the natural place) before the scheduler is measured. GPU evaluation of the local stage (review item 4b) is not yet justified for the draw set by these numbers; it matters for a world-scale teleport.
Needs the architect (perf window): GPU upload time and per-frame upload cost of the draw and surroundings sets on the RTX; CPU encode time per draw at ground-1m (already queued); the plan times above under the readiness gate (these are ad hoc).
Open points: the hidden-triangle pass uses unmorphed positions and the 1024 bake (the 16384 store would take about 40 minutes through the mutex); one bake seed (1); the union of resident tiles along a dive was not counted; the mesh-key explanation of the seconds-long altitude plans is an inference.

## SPIKE-03 Global hydrology on the sphere
Status: open
Opened: 2026-10-12
Question: Is global stream-power erosion on the cube sphere (flow routing with depression filling across face edges, drainage accumulation, implicit stream-power steps) practical at coarse grids, how does it compare with the fallback (flow routing and rivers on the unmodified base heights, local carving) in the look of the drainage and in visible seams at face edges and cube corners, and what does it cost in time and memory against the 8 GB budget (CON-21)?
Why it matters: M3's hydrology (SHOULD in the report's MVP; the fallback is the local erosion filter plus rivers on a coarse flow grid); the owner decides which to build. Under ADR 0022 option 1 a multi-step chain is one field, so this needs no same-stage reads; if a design turns out to need them (CON-08), stop and report.
Time box: one agent session.
Success criteria: both approaches run on the same base heights at grids of 256, 512 and 1024 cells per face edge (and a short run at 2048) with times and memory; maps and cube-edge crops of both are rendered into `target/review/hydrology-spike/` with a contact sheet; a seam metric (change of drainage area and of height across face edges against inside faces); an extrapolation to the 4-10 km and 1 km grids.
Fallback if it fails: the fallback itself (local erosion filter plus rivers on a coarse flow grid), which is what the report's MVP assumes.
Code location: spikes/hydrology/ (throwaway; branch `spike/hydrology`, never merged).
Result (2026-10-12, one session; WARP is irrelevant here, all CPU; 20 threads for sampling, ROUTING AND EROSION SINGLE-THREADED; seed 7; images in `target/review/hydrology-spike/`):
- Method. Cell-centre lattice of the cube sphere (res cells per face edge, tangent warp), 8 neighbours, neighbours across face edges and corners through the direction of the extended cell centre (`face_to_direction` beyond the face, then `direction_to_face`); base heights are the pointwise global field at the cell centres. Fallback: priority-flood routing from the ocean (depressions filled by construction), drainage area by accumulation in reverse flood order, rivers where the area exceeds a threshold, the base heights untouched (rivers carved a little for the picture). Global stream power: the same routing repeated, with uplift 20 m per step and the implicit n = 1 update (Braun and Willett, K 31.6 per sqrt(km^2), m 0.5) in flood order, 30 steps; it is one field that reads only the base heights (an earlier stage) and produces heights, receivers and areas, so ADR 0022 option 1 holds and no same-stage reads are needed; a regional tile reading the coarse network for local carving reads an earlier stage's output. NOT TUNED: K, m and the uplift were set for a plausible equilibrium slope, not fitted to a landscape.
- Cost (one thread, release): 256 cells (39 km): 0.03 s per step; 512 (19.5 km): 0.15 s; 1024 (9.8 km, 6.3 M cells): 0.72 s per step, 21 s for 30 steps (routing is 80 %: the priority flood), fallback routing 0.56 s; 2048 (4.9 km, 25 M cells): 3.1 s per step, about 93 s for 30 steps, fallback 2.4 s. Arrays about 44 B per cell (1.1 GB at 2048; the flood's visited, receiver, filled, order and heap add the same again at its peak, not measured). Extrapolation: 4-10 km grids (res 1024 to 4096, up to 100 M cells) take minutes and 2-8 GB; the 1 km grid of the report (16384 cells per face edge, 1.6 billion cells) would take about 3.3 s x 64 = 200 s per step (hours for 30 steps) and tens of GB: global stream power is only feasible on a coarse grid (4-10 km), with the finer scales from local carving; the fallback's routing needs the same coarse grid (its one pass is 30 times cheaper).
- Look. Fallback (left column of the seam sheet): rivers follow the unmodified base topography, with long straight diagonal runs in flat areas (the D8 lattice) and no valleys around them. Stream power (middle): dendritic drainage with valleys and sharpened ridges around the rivers, relief that explains the network; its slopes show lattice-aligned hatching (stair steps along the 8 directions) at 10 km cells.
- Seams. The rivers and valleys of both approaches cross the face edges and the cube corner without a visible step (right column, face edges drawn in magenta). Metric over 4-neighbour land pairs at 1024 cells: stream power, inside faces mean |dh|/dist 0.0038 and mean |dlog10 area| 0.035, across edges 0.0060 and 0.094 (x1.6 and x2.7); the fallback's |dlog10 area| across edges is 0.165 against 0.102 inside (x1.6) and its |dh|/dist is equal (0.0013 against 0.0014). The across-edge excess grows with resolution (256 cells: x1.15 and x1.3). Not resolved in this session: part of it is how a cell across an edge is chosen (cell centres on the two faces are half a cell apart and cells there differ in shape), so a mild seam bias at face edges cannot be excluded; the maps show no visible artefact. Receiver links across face edges: 5434 of 2.9 M land cells at 1024.
Recommendation: for the owner. Stream power gives a markedly better drainage look but only at coarse grids, a 30-step chain that is one global field of tens of seconds to minutes (a cached bake product like the global bake, STRM-012 style), needs hatch-free routing (D-infinity or multiple flow direction, not tried) and a seam check at the real resolution; the fallback is cheap and seam-free by construction but looks plain. A hybrid fits ADR 0022: the global stream-power stage at 4-10 km, local carving and the erosion filter (SPIKE-04, variant B) at the fine scales reading the coarse network from the earlier stage.
Kept images (2026-10-13, downscaled JPEG copies of the contact sheets; the full set under `target/review/` is not kept): [world](spikes/spike-03-hydrology-world.jpg), [seams](spikes/spike-03-hydrology-seams.jpg) (columns: fallback, stream power, stream power with the face edges in magenta; rows: mountain at a cube corner, coast across a face edge).

## SPIKE-01 GPU-driven indirect path in wgpu
Status: closed
Opened: 2026-10-10
Closed: 2026-10-10
Question: Does wgpu 30 expose what a GPU-driven instance path needs (indirect indexed draws, first-instance, multi-draw count, binding arrays, immediates, timestamps)?
Why it matters: ADR 0013 (render graph and GPU-driven baseline), report §8, §14 ("wgpu feature gaps").
Time box: 30 minutes.
Success criteria: the features are advertised by the adapters we test on (WARP, lavapipe via CI, RTX 3080 Ti Laptop).
Fallback if it fails: raw Vulkan/D3D12 backend behind the render-HAL seam.
Code location: none (a throw-away query of `Adapter::features()`; not kept).
Result: `INDIRECT_FIRST_INSTANCE`, `MULTI_DRAW_INDIRECT_COUNT`, `TEXTURE_BINDING_ARRAY`, `STORAGE_RESOURCE_BINDING_ARRAY`, `IMMEDIATES`, `TIMESTAMP_QUERY`, `DEPTH32FLOAT_STENCIL8` and `FLOAT32_FILTERABLE` are advertised by `Dx12 Microsoft Basic Render Driver` (WARP) and by `Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU`. Default device limits: 128 MiB storage buffer binding, 256 MiB buffer, 8192 texture dimension, 4 bind groups. Linux lavapipe was not queried (CI has no feature log); the Intel Iris Xe iGPU on this machine lacks the binding-array features (reported by the verifier). The query code was not kept. Features are available but must be requested when the device is created (the context currently requests none); no GPU culling pass was built or timed.
Recommendation: no fallback needed for the instance path; request `INDIRECT_FIRST_INSTANCE` and `MULTI_DRAW_INDIRECT_COUNT` when the first GPU culling pass is added (REND-008), behind its CPU reference (`frame::instances`). Timings need Rasierklinge.

## SPIKE-04 Erosion filter variants
Status: open
Opened: 2026-10-12
Question: Which pure-function-of-the-direction erosion filter (gradient-damped fBm, slope-incised ridged creases, flow-aligned gullies) gives the most convincing terrain in the relief view over the global bake, at what cost per sample, with what height-bound argument, and does cells 32 / ratio 240 still plan?
Why it matters: M3's local erosion filter (the report's fallback to hydrology, SPIKE-03); the owner picks the look.
Time box: one agent session.
Success criteria: three variants rendered in the relief view at 4 cameras (coast, mountain, 1 m, 200 m) into `target/review/erosion-spike/` with a contact sheet; per variant: ns per sample, the bound argument, plan verdict at cells 32 / ratio 240.
Fallback if it fails: keep the plain fBm detail of `cpu-terrain-v1` (GEN-016).
Code location: spikes/erosion-variants/ (throwaway; branch `spike/erosion-variants`, never merged; it makes `layer_noise` and `layer_noise_with_gradient` public on that branch only).
Result (2026-10-12, one session, bake of 512 cells, seed 7, WARP, release build, one thread; images in `target/review/erosion-spike/`: `contact-sheet.png` = relief view, 4 columns 0-baseline / A-damped / B-incised / C-meander, 4 rows coast-1500m / mountain-2km / ground-1m / hills-400m; `contact-sheet-hillshade.png` = top-down hillshade patches (coast 40 km, mountain flank 8 km, mountain 600 m), same columns, which shows the character far better than the relief view, whose mild lighting makes all variants look flat at flight scale):
- 0 baseline (`cpu-terrain-v1`: fBm of 14 octaves from 64 per radius, persistence 0.45, at most 300 m): 735-760 ns per sample; smooth, no visible erosion character at any of the patches. Measured Lipschitz constant 7.1e5 m/rad (x3 safety).
- A damped fBm (octaves from 256 per radius, persistence 0.55, up to 900 m on land; each octave damped by 1/(1 + (|accumulated gradient, bake slope included| / 1.27e6 m per rad)^2)): about 1.2 us per sample (the bake slope costs 4 extra bake reads). Looks like fine granular rock, rough everywhere, no drainage pattern. Lipschitz 3.0e6 m/rad. Bound argument: weights in (0, 1], so |detail| <= a0 sum(amp)/norm; over a cap the per-octave noise bounds times the weight range, the weight range from the variation of the accumulated gradient (Hessian bound times reach plus the Lipschitz constant of the bake's smoothed gradient): tight on small caps, loose on large ones.
- B incised creases (ridged creases (1 - |n|)^3 per octave, amplitude (80 + 900 land) m scaled by 0.15 + 0.85 smoothstep(slope 0.01 to 0.2) of the bake slope): about 1.2-1.3 us. Branching drainage-like crease network, deeper on steep ground. Lipschitz 1.17e6 m/rad. Bound argument: interval arithmetic: `layer_noise_bounds` per octave, exact monotone map of |n| through (1 - |n|)^3 (the kink of |n| is not a problem for a range), the slope factor from a monotone function of the bake gradient bounded over the cap by cell differences, product of intervals: sound and as tight as the terrain field's.
- C meander (B with the lattice position displaced by 0.8 cells of smooth vector noise at 0.35 of the octave frequency): about 2.7-2.9 us. Meandering, river-like creases. Lipschitz 1.15e6 m/rad. Bound argument: B's, with the noise evaluated over the box the displaced positions can reach (the displacement is Lipschitz, about 2.5 times the box of B): looser at mid octaves, equal for tiny caps.
- A flow-aligned variant was tried first (lattice stretched along the downhill direction) and dropped: the stretch needs a local reference point, and a stretch about the point itself is the identity (the direction is tangent, so the position has no component along it); a real one needs a per-cell reference, which seams at cell borders.
- Planning at cells 32 and viewport / tau 240 (640x360, tau 1.5 px), four cameras, `max_height_m` 10,000: all four fields plan at all four cameras (2.5-9.5 thousand nodes, B and C within 1 % of the baseline, A within 3 %). CAVEAT: the spike's `height_bounds` are Lipschitz bounds about the centre value with the measured constant times 3, not proved, and the bake is 512 cells (not 16384): the ADR 0016 check must be repeated with the real bounds and the production bake.
Recommendation: not made; the owner picks the look from the sheets. Costs: B is about the cost of the baseline plus the bake slope; C doubles it; A and B need the bake's gradient (4 to 8 extra bake reads per sample), which a regional field with a halo would read once per tile instead.
Kept images (2026-10-13, downscaled JPEG copies of the contact sheets; the full set under `target/review/` is not kept): [relief view](spikes/spike-04-erosion-relief.jpg), [hillshade](spikes/spike-04-erosion-hillshade.jpg) (columns 0-baseline / A-damped / B-incised / C-meander, as described above). Variant B was adopted (ADR 0030) as `cpu-erosion-v1`.

## SPIKE-02 CDLOD morph quality
Status: closed
Opened: 2026-10-10
Closed: 2026-10-10
Question: Does per-vertex geomorphing with parent-grid targets and skirts give crack-free level transitions on the cube-sphere quadtree (on the smooth test generator; steep terrain not covered)?
Why it matters: ADR 0014 (terrain LOD), LOD-004, LOD-005, REND-009.
Time box: part of the m2-terrain-render change.
Success criteria: zero hole pixels in ten scripted views; border vertices fully morphed next to coarser leaves and unmorphed next to finer ones; fully morphed borders coincide with the coarser neighbour's border across faces.
Fallback if it fails: stitching meshes or a larger skirt.
Code location: `crates/frame/src/lod.rs`, `crates/generators/src/mesh.rs`, `crates/render/src/terrain.rs` (not throw-away: implemented through OpenSpec changes).
Result: all three criteria hold in tests (`morph.rs`, `mesh.rs`, `gpu_terrain.rs`) on WARP. Two defects were found and fixed on the way: morph distance measured to height-including positions (now a height-free reference position) and a morph start earlier than the split distance plus bounding radius (now `LodParams::morph_interval`, validated). Visual quality of the morph on the real GPU and with realistic terrain is unreviewed (six goldens await the owner).
Recommendation: adopt CDLOD with vertex morph and skirts (ADR 0014); revisit meshlets after the M2 gate if node counts (772 draws at a 1 m view with the small test parameters, 3,003 with the production parameters) cost too much. Steep terrain was not tested: the test generator is smooth.
