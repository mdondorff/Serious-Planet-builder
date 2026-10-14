# Review of the performance optimization brainstorm

Status: Architect's review, for the owner's decision (not normative; the queue changes it proposes take effect only once
they are in `docs/status.md`)
Date: 2026-10-14
Reviewer: opus-planner (architect session)
Reviews: `docs/background/optimization-brainstorm.md` (Copilot, 2026-10-14)
Evidence: the last valid perf session (`perf/logs/terrain.last-valid.json`, M3 gate window 2026-10-13, Rasierklinge,
2560x1440, cells 32, tau 6), `crates/render/src/terrain.rs`, ADRs 0013, 0014, 0031, 0033, constitution CON-03, CON-07,
CON-14, CON-20, CON-21. Numbers marked "by hand" are worked out, not measured.

## Answer in short

Yes: three properties should be built in M4, before the residency manager and the M4 perf scenes harden the current
shape. Today the CPU, not the GPU, limits the frame, and the CPU cost grows with the number of tiles drawn.

1. **Frame pipelining.** The CPU and GPU take turns instead of working at the same time, and the plan runs on the same
   thread.
2. **One pooled tile buffer with fixed-size slots.** It replaces the current one GPU buffer per tile.
3. **Indirect multi-draw from a CPU-built argument buffer.** It replaces the current three commands per tile.

All three keep every decision on the CPU (CON-03) and are tested by counts and by differential images. The brainstorm's
GPU culling, HZB occlusion, clipmaps, mesh decimation and instancing items either conflict with the constitution and
ADR 0014, or are already covered in a better-fitting form by SPIKE-06 and SPIKE-07.

## What the measurements say (the part the brainstorm could not see)

At ground-1m, the heaviest standard view (median of three runs, 2026-10-13):

| Per frame | ground-1m | ground-200m | aerial-20km | Per node at ground-1m |
|---|---|---|---|---|
| Nodes = draw calls | 8,687 | 4,425 | 1,071 | |
| Triangles | 20.0 M | 10.2 M | 2.5 M | 2,304 |
| CPU plan, warm (`plan_ms`) | 6.58 ms | 2.99 ms | 0.69 ms | 0.76 µs |
| CPU encode + submit (`cpu_ms`) | 5.95-6.38 ms | 2.74-2.86 ms | 0.66-0.68 ms | 0.69 µs |
| GPU (`gpu_ms`, median / p99) | 4.08-4.24 / 4.6-4.8 ms | 2.0 / 2.1-2.5 ms | 0.6 ms | 0.47 µs |
| Frame wall time (median / p99) | 10.5-11.2 / 12.3-13.8 ms | 5.1-5.3 / 6.4-9.6 ms | 1.5-1.6 / 2.4 ms | |

Four conclusions:

- **The CPU costs more than the GPU, and both CPU costs grow with the node count.** Encoding 8,687 draws, each with
  `set_bind_group` (a dynamic offset), `set_vertex_buffer` and `draw_indexed` (`terrain.rs:537-541`), costs about 6 ms.
  That is more than the GPU's 4.1 ms.
- **The frame is serialized.** `TerrainRenderer::render` submits and then waits for the GPU (`device.poll(wait_indefinitely)`),
  so the frame time is encode plus GPU: 6.0 + 4.1 ≈ 10.5 ms, as measured. In the perf scene the camera stands still, so
  the plan is reused. A moving camera adds the plan (6.6 ms warm, 25-29 ms cold) on the same thread. By hand that gives
  6.6 + 6.0 + 4.1 ≈ 16.7 ms, which is the whole 16.6 ms frame budget (CON-21), spent on terrain alone, with no
  atmosphere, shading, ground cover or streaming yet.
- **The 12-14 ms "p99" in the 2026-10-13 log line is the frame p99, not the GPU p99.** The GPU p99 is 4.6-4.8 ms. I
  labelled it wrongly in `docs/log.md`, and a correction line is added there. The GPU has headroom. The CPU-side frame
  does not.
- **The scene has about 5.4 triangles per pixel** (20.0 M triangles for 3.7 M pixels at tau 6 px). Either a lot of
  hidden terrain is drawn, or distance-based CDLOD over-tessellates surfaces seen at grazing angles. Tiny triangles
  make the GPU shade many extra pixels at triangle edges, which would explain why the larger fragment shader of #65
  cost 40 % rather than 10 %. That is a hypothesis: the queued ground-1m A/B tests it.

Hence the priorities: first stop the CPU and GPU from waiting on each other, then cut the CPU cost per node, then cut the
nodes. GPU-side tricks come last.

## The brainstorm's items, one by one

| # | Brainstorm item | Verdict | Why |
|---|---|---|---|
| 1 | GPU-driven culling + multi-draw-indirect | **Split.** Multi-draw-indirect from CPU-built arguments: **adopt in M4.** GPU culling of terrain: **no.** | CON-03 and ADR 0013 forbid GPU culling of terrain nodes; it is allowed only for instances. The CPU already culls (LOD-007), so the gain is in submission. Writing the 8,687 draw records into an indirect buffer on the CPU and issuing one `multi_draw_indexed_indirect` removes most of the 6 ms encode. It needs the pooled buffer (item B below) and node data in a storage buffer indexed by `first_instance` instead of dynamic uniform offsets. |
| 2 | HZB occlusion | **Not now; owner's decision if ever.** | GPU occlusion of terrain nodes is GPU culling under CON-03 and would need a constitution amendment. The GPU has headroom today (4.1 ms of 16.6). Measure the fraction of hidden triangles first (item D). Reconsider if GPU time becomes the limit and hidden terrain is a large share. |
| 3 | Geometry clipmaps | **No.** | ADR 0014 rejected them: they conflict with per-tile generation, the tile cache (ADR 0018, 0019) and edits anchored to tiles. Their useful idea, one shared grid mesh plus per-tile height data, is SPIKE-06 (queued, M4 item 2). |
| 4 | Height textures with GPU-side mesh expansion | **Already queued** (SPIKE-06), keep. | This is SPIKE-06's compact tile format: today's vertex is 44 B, a 16-bit tile-local height is 2 B. It cuts the 467 MB per frame of vertex fetch, the upload volume and the VRAM share (ADR 0033) together. CON-11 and CON-13 must hold; the spike checks them. |
| 4b | GPU *generation* (compute generators) | **Later (M5 or after), CPU reference first.** | CON-07 allows a GPU implementation that matches the CPU within CON-14's bounds, and CON-14 requires neighbouring tiles to share one implementation. The local stage (per point) is the natural candidate. SPIKE-07's shader detail below the finest level is already a small step that way. Decide it once generation throughput is measured (item F). |
| 5 | Mesh simplification / adaptive sampling | **No** (in that form). | Decimated meshes break CDLOD's fixed grid, its per-vertex geomorph and its crack-free seams (ADR 0014, LOD tests). The same goal is reached by the knobs that keep the grid: the finest level (SPIKE-07), tile size (cells 64, item D) and tau. |
| 6 | Async generation + predictive prefetch | **Already decided** (ADR 0031, sketch). | Workers exist (`streaming::ThreadPool`, injected `Spawner`, CON-04). The owner decided on 2026-10-13 that residency is position-based and prediction from velocity is only an extra (ADR 0031 class 3). |
| 7 | Instancing of identical meshes | **No** (procedural tiles are unique). | The real form of mesh sharing is SPIKE-06's one shared grid for all tiles. |
| 8 | Compressed vertices / Draco | **Quantization yes (SPIKE-06), Draco no.** | CPU decoding would cost time on every upload. Quantized tile-local heights are the cheap form; CON-13's 1 mm bound at the finest level decides the bit depth. |
| — | Reuse `SelectionCache` | **Done** (#52). | The perf scene plans through `plan_terrain_cached`. There is no interactive frame loop yet: A below designs it with the cache. Cold plans (25-29 ms) remain: they are a teleport hitch (ADR 0032 separates plan time from streaming). |
| — | Async, rate-limited uploads | **Already queued** (M4 item 7, upload budget per frame, CON-21 ≤ 2 ms, STRM-002 parent fallback). | |

## What to build, and when

These are ordered by what becomes expensive to change once the residency manager and the M4 perf scenes are built on
the current shape.

### A. Frame pipelining (M4, before the perf scenes)

- **What:** the interactive loop never waits for the GPU of the frame it just submitted (one or two frames in flight).
  The plan for frame N+1 is computed on a worker while frame N encodes and the GPU runs frame N-1. `FramePlan` is
  already a value built in GPU-free crates (CON-03), so it can be handed between threads. `test-render` and the goldens
  keep the synchronous path: the pipeline changes *when* work happens, not *what* is drawn.
- **Gain:** by hand, the critical path at ground-1m goes from about 16.7 ms (plan + encode + GPU in a row) to about the
  largest of them, about 6.6 ms today.
- **Why now:** no interactive frame loop exists yet (only the perf scene and `test-render` call `render`), so this is
  the cheapest moment to design one. The M4 scheduler, the upload budget and the dive and teleport perf scenes all assume a frame loop. If
  they are built on "render and wait", every one of them must be redone later.
- **Tests:** a tier-A test that the pipelined loop issues the same plans in the same order as the synchronous loop, on a
  fake clock (CON-04); a count-based assertion of frames in flight; the perf scenes measure a moving camera with plan,
  encode and GPU separately.
- **Cost to reverse:** low; the synchronous path stays as the reference.

### B. One pooled tile buffer with fixed-size slots (M4, decide in ADR 0033, build with the residency manager)

- **What:** every tile mesh (or, after SPIKE-06, every tile's height texture or array layer) lives in a slot of a few
  large GPU buffers. Today each tile gets its own `create_buffer_init`. Tiles have a fixed size at fixed `cells`, so a
  slot allocator is a free list with no fragmentation.
- **Gain:** exact count-based VRAM accounting for the terrain share (ADR 0033 point 1, CON-20), no allocator churn while
  tens of thousands of tiles stream in and out, and it is the precondition for C (one vertex buffer, draws by
  `base_vertex`).
- **Why now:** the residency manager (STRM-006/007) is about to be written, and the VRAM share is defined by what it
  allocates. Building it on per-tile buffers and changing it afterwards means writing it twice.
- **Tests:** tier A: slot accounting and eviction against the share on a fake device budget; tier C: the existing
  goldens are unchanged (no MESH_VERSION change unless SPIKE-06's format is adopted at the same time).
- **Cost to reverse:** low now, medium after the residency manager exists.

### C. Indirect multi-draw from a CPU-built argument buffer (M4, right after B)

- **What:** the CPU writes one draw record per planned node (index count, `base_vertex` = the tile's slot, and
  `first_instance` = the node's index into a storage buffer of per-node data) and the pass issues one
  `multi_draw_indexed_indirect`. The adapter's features (`MULTI_DRAW_INDIRECT`, `INDIRECT_FIRST_INSTANCE`; SPIKE-01
  confirmed indirect support on both adapters for instances) decide whether this path is used. The per-draw loop stays as
  the fallback and the reference.
- **Gain:** most of the 6 ms encode at ground-1m (0.69 µs per node today). By hand it falls below 1 ms; a perf window
  measures it.
- **Constitution:** compliant. The CPU decides every draw; the GPU only executes the list (CON-03). No GPU culling.
- **Tests:** tier C: the indirect and loop paths render identical images on WARP and on the RTX (differential, like
  REND-008); tier A: one submit, draw-record count equals plan nodes (CON-20).
- **Cost to reverse:** low; the loop path remains.

### D. Fewer, larger nodes (M4, as extra columns in SPIKE-05, about an hour)

- **What:** CPU plan and encode cost grow with *nodes*, the GPU's with *triangles*. At the same tau, cells 64 gives
  about a quarter of the nodes for roughly the same triangles. LOD-004's morph rule still allows it at 1440p (240 >
  1.9 x 64 ≈ 122). Add to SPIKE-05's table: nodes, triangles and MB at cells 32 and 64, and the finest level 26, 23 and
  22 (already planned). Also add one count-based diagnostic: the share of drawn triangles that produce no visible pixel
  (a depth-test pass count or an overdraw debug view). It tells whether item 2 (occlusion) could ever pay, and whether
  the 5.4 triangles per pixel come from hidden terrain or from grazing angles.
- **Risk:** cells 64 makes tiles four times larger, so residency becomes coarser grained and each tile costs more to
  generate (a teleport waits longer for its first fine tile). The learning of 2026-10-11 (no morph interval on the
  steep field at cells 32, ratio 240) must be rechecked at 64 on the eroded field.
- **Cost to reverse:** cells is a parameter; it changes goldens and plan snapshots (through `golden-review`).

### E. Decide whether shader detail is visual only (M4, SPIKE-07 / ADR 0033)

If SPIKE-07's detail term below the finest level changes heights (not only normals), then it is a field implementation
under CON-07 and CON-14. It then needs a CPU reference, and `height(dir)` for collision and picking must include it.
If it is visual only (normals and shading), none of that applies. This decides how cheaply the finest level can be
lifted, so SPIKE-07's report should state which of the two it is.

### F. Measure generation throughput (M4, in SPIKE-05 or the first scheduler perf scene)

There is no measured tiles-per-second figure for the eroded field, and teleport and dive fill times depend on it. The
only data point is the global bake: 4.7 M samples/s on 20 threads. By hand, *assuming* a field sample costs about as
much as a bake sample: a 32-cell tile with its halo has about 1,200 samples, so about 3,800 tiles/s. The ground-1m draw
set (8,687 tiles) would then take about 2.3 s from empty, and the position-based residency set (3-6x) 7-14 s. The disk
cache helps only on revisits. If the measurement confirms numbers of this size, GPU evaluation of the local stage
(item 4b) moves up, and ADR 0032's teleport detail-lag budget must be set with this in mind.

### Not on the list

GPU culling and HZB for terrain (CON-03; owner amendment only), clipmaps (ADR 0014), mesh decimation, instancing of
unique tiles, Draco compression.

## Testability and agentic development

Each recommended change keeps the properties the project depends on:

- Decisions stay in GPU-free crates and remain pure, so the tier-A selection tests and plan snapshots are unaffected.
- Every change has a count-based acceptance (frames in flight, draw records, slots allocated against the share) that
  runs on any adapter (CON-20).
- Each new GPU path has a reference path kept beside it and an image differential (WARP and RTX), the same pattern
  ADR 0013 set for instances.
- Time effects are measured only in the architect's perf windows (ADR 0020). The perf scene should record plan, encode
  and GPU separately, as it already does, plus the frame time of a moving camera.

## Queue changes (decided by the owner, 2026-10-14; in `docs/status.md`)

Into the M4 queue in `docs/status.md`:

1. SPIKE-05 gains the cells-64 columns, the hidden-triangle count and a tiles-per-second measurement per field (D, F).
2. ADR 0033 decides the pooled slot allocator (B) and records whether shader detail is visual only (E).
3. M4 item 7 (implementation) runs in this order: scheduler; residency manager on pooled slots (B); **frame pipelining
   (A)**; **indirect multi-draw (C)**; compact tile format if adopted; prefetch; upload budget; perf scenes.
4. The first M4 perf window, which already runs the ground-1m A/B, also records encode time per draw, to set a baseline
   for C.

M4's exit criteria (no blank tiles, p99 ≤ 20 ms, VRAM within the tier cap, RAM ≤ 8 GB) are unchanged. These items are
how M4 meets them with headroom for the passes that come after terrain.
