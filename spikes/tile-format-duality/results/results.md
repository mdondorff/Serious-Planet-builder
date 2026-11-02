# Two tile formats: cost of the duality and whether the vertex format can retire from rendering (exploration, 2026-10-17, sonnet-spikes)

Question from opus-planner (owner's question: does keeping two tile formats spread special cases through the code?). Read-mostly: counts come from grep and function sizes at main 6d061c3, the three measurements are CPU-only (`spikes/tile-format-duality`, branch `docs/tile-format-duality`, no production code touched, no GPU run, no perf run). Line counts for "parallel paths" are rough (function sizes, not a parse); the enum counts are exact greps.

## Short answer
1. The duality is real but bounded: about **700-800 production lines (1.6 % of 45,100)** exist only for the vertex format and about 900 only for the compact one; the fragment shading is shared (spliced once). It leaks into every crate from `generators` to `app`: 145 explicit uses of the format enums in `src`, two pool kinds, three residency forms, two pack payload forms, six pipelines, two climate implementations, and roughly 40 of 805 integration tests.
2. Levels 0-2 are in the vertex format for **one** reason: the compact format's tile-local base surface (3 x 3 control points, biquadratic) is too coarse for tiles that big. Not the 16-bit height range (a step of 0.08 m at level 0 against a 52 km pixel), not cells, not curvature of the heights. A second, separate reason keeps the vertex format as the CON-13 fallback (finest tiles with more than 65 m of relief).
3. Both blockers have cheap answers. Levels 0-2 are drawn in **one** of the ten standard views (orbit: 52 of 11,713 nodes); either a 5 x 5 base surface (error 0.18 px at level 0, 0.017 px at level 1, 0.0016 px at level 2; +192 B per record) or a render floor at level 3 (about 170 tiles in the orbit view instead of 52) removes them. The cliff fallback goes with a 32-bit height texel (32 + 15 + 15 bits fits the same 8 B texel; climate then needs a 324 B per-tile block, +3.7 % bytes per tile).
4. Retiring the vertex format from rendering is **about 5-7 coder days**, removes roughly 750 production lines and touches about 40 tests; of the 18 terrain-view goldens (6 per adapter) the two orbit views change for certain, and the rest only if the new quantisation moves a pixel (they use `Tolerance::Exact`), so plan for all 18 through `golden-review`. I would not do it inside M5; I would do it before item 7's water mask if the schedule allows, otherwise right after the M5 gate (recommendation below).

## 1. Inventory: where the code branches on the format
Exact greps over `crates/*/src` (spike modules excluded), lines that name the enum or marker (tests in the second number):

| marker | generators | cache | streaming | frame | render | app | total (src / tests) |
|---|---|---|---|---|---|---|---|
| `TileFormat::` | . | . | . | . | 10/0 | 24/0 | 34 / 0 |
| `RenderTile::` | 11/3 | 5/0 | . | 2/5 | 2/8 | 1/0 | 21 / 16 |
| `RenderForm::` (pack payload form) | . | 13/21 | . | . | . | 2/0 | 15 / 21 |
| `Place::{Vertex,Atlas,Fallback}` | . | . | . | . | 21/0 | . | 21 / 0 |
| `ResidentFormat::` | . | . | . | . | 3/3 | . | 3 / 3 |
| `DrawBuffer::` | . | . | . | 3/0 | 7/0 | . | 10 / 0 |
| `PoolKind::` | . | . | 7/5 | 4/2 | 16/14 | . | 27 / 21 |
| `TileRef::` | . | . | . | . | 13/0 | . | 13 / 0 |
| `vertex_reason`, `fallback_*`, `FALLBACK_*` (the CON-13 / level 0-2 rule) | 9/9 | 1/0 | 9/0 | 2/2 | 6/12 | 0/2 | 27 / 25 |

Parallel code paths (rough lines; `render/src/terrain.rs` is 2,433 lines of 6,732 in `render`):

| piece | vertex-only | compact-only | shared / note |
|---|---|---|---|
| tile build (`generators`) | `tile_mesh_field` + `TileMesh` (416, but it is also the CPU reference and the canonical definition of positions: stays), GPU packing `gpu_climate`, `GPU_STRIDE` (~40) | `compact.rs` 514 (build, record, quantisation, CPU mirror `shade_vertex`) | `RenderTile`, `vertex_reason` ~60 decide the form per tile |
| pack payloads (`cache/meshpack.rs`, 549) | vertex payload codec ~120 | compact payload codec ~150 | `RenderForm` in the key (param 5), tests 259 + 308 |
| pools (`frame/pool.rs` 88, `streaming/slots.rs` 189, `budget.rs` 228) | `vertex_format_pool`, vertex pool kind, fallback region (256 slots, 13.7 MB reserved) ~90 | atlas pool maths ~70 | `PoolKind::{Main, Vertex}` |
| renderer (`terrain.rs`) | ~365: `with_pools`, `write_vertex_tile`, vertex pool and block buffers, 3 vertex pipelines (debug, lit, lit+atmosphere), fallback placement, pipeline switching in `render_inner`, warm-up | ~550: `make_compact`, `write_compact_tile`, `flush_batch`, `read_compact_pools`, atlases, 3 compact pipelines, frame bind groups | `render_inner` (298 lines) branches at 20+ places |
| shaders | vertex stage in `terrain.rs` ~25 + `tile_climate` 5 | `render/compact.rs` ~200 WGSL + atlas management | fragment text spliced once (cannot drift) |
| draw records (`frame/draw.rs`) | `DrawBuffer::{VertexPool, Block}` runs | `DrawBuffer::Atlas` runs | one multi-draw per run: a mixed list switches pipeline inside one pass |
| app flags | `--tile-format vertex`, software smoke on the vertex format (VIEW-005) | default | ~60 lines in `app` |

Leaks that cost real work, with the concrete example the owner named: **climate** (ADR 0039) exists twice. Vertex format: two f16 per vertex as vertex attribute 5, interpolated by the rasteriser (`gpu_climate`, `MeshVertex::GPU_STRIDE`). Compact format: a 9 x 9 texture in the spare 16-bit channel of 162 texels, bilinear in WGSL (`tile_climate`, 30 lines, `ClimateTexture::write_texels`). The two give slightly different values, so tests compare them within a bound, and the shared `VsOut.aux` field means "climate" in one pipeline and "tile u, v" in the other. Item 7's water mask would be built the same way twice.

Tests (heuristic grep over 805 top-level `#[test]` in `crates/*/tests`): 15 are parametrized over both formats (`for compact in [false, true]`, 535 lines, in `gpu_lit`, `gpu_atmosphere`, `gpu_shading`, `gpu_warmup`, `gpu_compact`, `gpu_indirect`, `frame/managed`), 5 build a vertex-only renderer (129 lines: `gpu_pools`, `gpu_warmup`, `streaming/slots`), 15 concern the level 0-2 / fallback rule (526 lines), 16 are compact-only (454 lines); several tier-C differentials use the vertex GPU path as their reference ("the product's compact image equals it up to edge pixels"). Goldens: 10 PNGs per adapter x 3 adapters = 30 (per adapter 6 terrain views, 3 debug views, the hello triangle; they are images of the product's mixed path, compared with `Tolerance::Exact`), plus the lit and atmosphere goldens under the same policy.

## 2. Why levels 0-2 are vertex format (measured)
ADR 0033 point 4 and SPIKE-06 give the reason in words: the biquadratic base surface through nine tile-local control points differs from the canonical mesh by about 0.018 theta^2 of the tile edge. Reproduced on the CPU (f64 model against `tile_mesh_field`, all 33 x 33 grid vertices of up to 24 sampled tiles per level, `Reference(1)` field; pixel = tile edge / 190, SPIKE-06's convention):

| level | tile edge | metres per pixel | 3 x 3 (now) m / px | 5 x 5 m / px | 9 x 9 m / px |
|---|---|---|---|---|---|
| 0 | 1.0e7 m | 5.3e4 | 1.9e5 / 3.63 | 9.7e3 / 0.18 | 22 / 4.3e-4 |
| 1 | 5.0e6 m | 2.6e4 | 3.2e4 / 1.20 | 440 / 0.017 | 0.51 / 1.9e-5 |
| 2 | 2.5e6 m | 1.3e4 | 6.7e3 / 0.51 | 22 / 0.0016 | 0.085 / 6.4e-6 |
| 3 | 1.25e6 m | 6.6e3 | 1.1e3 / 0.16 | 1.1 / 1.7e-4 | 0.044 / 6.7e-6 |
| 4 | 6.3e5 m | 3.3e3 | 150 / 0.046 | 0.061 / 1.9e-5 | 0.022 / 6.7e-6 |
| 5 | 3.1e5 m | 1.6e3 | 20 / 0.012 | 0.011 / 6.8e-6 | 0.011 / 6.6e-6 |
| 6 | 1.6e5 m | 8.2e2 | 2.6 / 0.0032 | 0.0056 / 6.9e-6 | 0.0056 / 6.8e-6 |

(The floor of about 7e-6 px is the f32 rounding of the mesh being compared; full table with 17 x 17 in `model-error.md`.) The 3 x 3 error is the 0.018 theta^2 law and the level 0-2 rows are not good enough (3.6 px, 1.2 px, 0.5 px; SPIKE-06 saw 73 pixels differ at level 2). Not the reason:
- **16-bit height range.** The worst sampled relief of a tile is 5,472 m at level 0 (up to 18 km for the clamped field); step = range / 65,535 = 0.083 m at level 0, 0.069 m at level 2, against pixels of 52 km to 13 km. The range is irrelevant at coarse levels (`steps.md`).
- **CON-11.** Control points and heights are tile-local; coarse tiles have coordinates up to 7e6 m, f32 spacing 0.5-1 m, far below a pixel. The vertex format has the same tile-local positions.
- **Cells and the morph.** Unchanged by level.

Which levels are drawn at all (planner on the ten standard views, cells 32, finest level 22, tau 6, 2560 x 1440): levels 0-2 appear **only in the orbit view** (52 nodes, all level 2); high-2000km already draws levels 4-6, every other view levels 6 and finer (`levels.md`). Levels 0-2 are otherwise ancestors and cold-start fallbacks (ADR 0045's pre-fill covers levels 0-4).

The other reason the vertex format stays is **CON-13**: a finest-level tile with more than about 65 m of relief (step 1 mm x 65,535) cannot be quantised in 16 bits and is drawn from the vertex format (`vertex_reason`, `FALLBACK_RELIEF_M`, the 256-slot fallback region).

## 3. Could compact cover levels 0-2 (and the cliff fallback)?
| option | what changes | cost | verdict |
|---|---|---|---|
| a. 5 x 5 control points for all tiles | record 128 -> 320 B (+192 B, +2.2 % of the 8.8 KB slot), shader unrolls 25 instead of 9 points | vertex stage is the GPU cost (SPIKE-06: 5x slower in a loop); needs a perf window; all goldens move at the 1e-6 level | error 0.18 px at level 0, 0.017 px at level 1: passes, but a cost on every vertex of every tile |
| b. 9 x 9 for levels 0-2 only | a second record layout (81 x 12 + 20 = 992 B) for 126 tiles in a small table (125 KB), uniform branch per draw | a branch per tile, another code path (smaller than a format) | error 4e-4 px at level 0: passes with margin; still a special case |
| c. render floor at level 3 | the selection never emits levels 0-2 (it refines them unconditionally); no format change | orbit view: about 170 level-3 tiles instead of 52 level-2 (geometric estimate: the 76-degree horizon cap of a 20,000 km view holds about 0.38 x 384 tiles); 384 level-3 tiles total = 3.4 MB; cold start needs them (pre-fill covers levels 0-4); LOD spec, selection and residency (`frame/lod.rs` 922 lines, `managed.rs` tests 1,482 lines assume level-0 roots) change | simplest in code, no new maths; the goldens of the orbit views change (tile ids, levels) |
| d. CON-13 cliff tiles: 32-bit height | texel = height 32 | normal 15 | normal 15 | 2 spare bits (still 8 B); climate leaves the spare channel and moves to a 324 B per-tile block (record 128 -> 452 B) | +324 B per tile = +3.7 % (8,840 -> 9,164 B per slot; ground-1m residency set 95 MB -> 98 MB); removes the relief guard, `vertex_reason`, the fallback region and its tests; one decode (two 16-bit lanes combined) | needs 15-bit normals: measured worst angle 16 bit 0.0037 deg, **15 bit 0.0074 deg**, 14 bit 0.0148 deg (over CON-14's 0.01 deg), 12 bit 0.059 deg (`normals.md`); ADR 0033's "about 0.002 deg" for 16 bit is 0.0037 deg measured, still inside CON-14 |

Within CON-11 and CON-14 all four work; (c) + (d) is the least new machinery. What would remain of the vertex format: the CPU reference `tile_mesh_field` / `TileMesh` (the canonical positions the compact CPU mirror and the tier-B oracles compare against, the mesh keys, the steep-field tests): about 416 lines in `generators`, no GPU path, no pool, no pack payload.

## 4. Removal estimate
Removed from production: the vertex pipelines and WGSL vertex stage (~65 lines), the vertex pool, vertex blocks and fallback region, `Place::{Vertex, Fallback}`, `ResidentFormat`, `TileRef::Vertex`, `RenderTile::Vertex`, `vertex_reason`, `FALLBACK_*`, the vertex pack payload (~120), `PoolKind::Vertex` and its share maths (~90 across `frame` and `streaming`), the `--tile-format` flag and software-smoke special case (~60), `DrawBuffer::{VertexPool, Block}` runs and the mid-pass pipeline switching. About 750 lines in total, about 15 % of `render/terrain.rs`; the spike modules (`spike_compact.rs` 855 + `spike_detail.rs` 649 + their tests) can go with it if wanted. Tests: the 5 vertex-only renderer tests go, the 15 parametrized ones lose a branch and their vertex reference (their differential moves to the CPU mirror at tier B: `oracle_compact_mirror` already does this for the positions), the 15 level 0-2 / fallback tests are replaced by a floor test; about 40 of 805 touched (the four heuristic categories overlap), roughly 1,200 test lines changed or removed. Goldens: the orbit views (tile id, level, probably relief) for certain; with `Tolerance::Exact` a new texel layout can move single pixels anywhere, so plan for all 18 terrain-view goldens through `golden-review`.

Work, as a coder-day estimate from the sizes above (not measured): texel and record layout v2 with its CPU mirror, payload codec, WGSL decode and tests 2 days; render floor at level 3 (selection, residency, LOD spec delta, orbit and cold-start tests) 1.5-2 days; removing the vertex path, pools, flags and the tests 1.5-2 days; goldens, verifier, one perf window to re-measure the vertex stage 1 day. **Total 5-7 days**, one OpenSpec change (or two: the format and the floor). Risks: MESH_VERSION and pack payload bump (prefill rebuild); the 32-bit height decode on the vertex stage is the only GPU-cost unknown (SPIKE-06's lesson: the vertex stage is the cost; needs a perf window at P0); ADR 0033 points 3-5 and the open "tile format" ADR text need an amendment; the CON-13 jitter half of the test is not run through the compact pipeline today.

## 5. Recommendation (for the owner at the M5 gate)
- The duality is a real recurring cost, concentrated in `render/terrain.rs`, the pool kinds and the pack payload, and it will grow with every per-tile attribute (climate now, water mask next). The blockers that keep the vertex format are shallow (one orbit view; one rare cliff case), so retiring it from rendering is feasible at about 5-7 days.
- **Not inside M5.** The gate is close, the retirement moves goldens and needs a perf window; but **before item 7's water mask** if the schedule has room (the mask would otherwise be built twice, and the ocean at orbit is drawn from level-2 vertex tiles). If item 7 cannot wait: build the mask for both paths and accept about one extra day, or give the vertex path a default (no mask) for levels 0-2 and say so.
- Cheapest first step regardless of the decision: put the per-tile low-resolution data (climate now, water mask next) behind one "tile data block" shape in the record (a 324 B block per slot, already the vertex format's shape) read by both pipelines, and stop overloading `VsOut.aux`.
- Order if approved: (1) texel/record layout v2 + 15-bit normals + 32-bit height, (2) render floor at level 3, (3) delete the vertex path, (4) goldens and one perf window.

## Limits
- CPU f64 model and `Reference(1)` field only (smooth, relief up to ~5.5 km); the eroded field adds relief, which does not change the base-surface error. The 5 x 5 and 9 x 9 results are the model, not a shader; f32 rounding of coarse tile-local coordinates (0.5-1 m) is far under a pixel.
- Line counts for "parallel paths" are rough; the test counts are a heuristic grep of top-level test functions and overlap between categories. The "pack compaction" feature (`pack/compact.rs`, a different meaning of "compact") is excluded from the counts.
- No GPU time was measured (the cost of 5 x 5 control points or a 32-bit height decode needs a perf window).
