# SPIKE-56 Shadow architecture

Opened: 2026-10-29
Question: what computes shadows in this engine: terrain as occluder for any receiver (layer 1), and later objects as occluders (layer 2)?
Why it matters: the horizon-map shadows (GEN-044/045, REND-038, STRM-048/060/064) will be redone (owner, 2026-10-29); M9 needs the replacement's architecture before an OpenSpec change.
Time box: phase 1 (desk) about 2 h; phase 2 (GPU prototype, two sites) about 3 h, only on opus-planner's go.
Success criteria: phase 1: a data layout that works under wgpu 30 core features and both CI software adapters, a per-frame cost estimate at 1440p on Rasierklinge with its derivation, an ADR 0083 GPU share, edit latency, CON-11 and CON-14 positions, a proposed ADR draft. Phase 2: measured GPU ms of the march at the two sites within the proposed share, stills.
Fallback if it fails: if the march exceeds the share at half resolution with temporal reuse, keep the clipmap and replace the per-pixel march by a per-texel shadow map baked in the clipmap (one march per clipmap texel, sampled by receivers).
Code location: spikes/m9-shadow-architecture/ (throwaway; not part of the main crates)

Status: phase 1 done (desk only: no builds, no GPU runs). Phase 2 waits for opus-planner's go.

## Inputs read
| Source | Fact used |
|---|---|
| GEN-019, GEN-025 | compact tile: 33x33 texels at cells 32, height `hr_min + f32(q) * hr_step` (tile-local), biquadratic base surface from 9 tile-local control points; slot 9,488 B |
| REND-016, STRM-006 | tiles live in storage-buffer blocks of up to 128 MiB (14,146 slots), 15 (mid) or 23 (high) blocks; read by slot index, no textures |
| STRM-015 | R is a function of camera position, LOD parameters and the horizon cut; view direction is not an input; holds leaves in all directions and their ancestors to the floor (level 3) |
| STRM-006 pins | R with ancestors is at most 10 % of 212,190 slots: about 21k tiles |
| `crates/render/src/graph.rs` | forward lit pass: `terrain` writes `hdr-scene` and `depth`; `sky-composite` applies aerial perspective afterwards; water draws between them |
| ADR 0083 | 16.6 ms frame at 1440p; measured GPU 4.3 ms (terrain 3.4); shares per subsystem not yet in a spec |
| REND-044 | horizon shadows plus material map: at most 1.5 ms of terrain-pass GPU |
| CON-11 v1.4, CON-14 v1.3 | lighting is presentation data: no CPU twin; presentation shading may use planet-scale constants under a declared, tested bound |
| wgpu-types 30.0.1 | `EXPERIMENTAL_RAY_QUERY`, `EXPERIMENTAL_RAY_TRACING_PIPELINES`, `TEXTURE_BINDING_ARRAY` are native-only features; ray query exists in the DX12 and Vulkan hal backends |
| world fixtures | body radius 6,371 km; face edge about 10,000 km; level L tile edge about 10,000 km / 2^L (level 16: 153 m, 4.8 m texels at cells 32) |

## Finding 1: ray length is bounded by curvature
A ray from a receiver toward a sun at elevation `e >= 0` above the receiver's horizon rises above the sphere by `d tan e + d^2 / (2R)`. An occluder at distance `d` must stand that high above the receiver.

| Relief between occluder and receiver | Longest possible occluder distance (e = 0) |
|---|---|
| 1 km | 113 km |
| 3 km | 196 km |
| 5 km | 252 km |
| 9 km | 339 km |

- `d_max = sqrt(2 R dh)`. A sun below the receiver's geometric horizon is the planet's own shadow: analytic, no march.
- So a march needs heights out to about 350 km around each shaded point at most, and to under 50 km for any sun above 5 degrees (`dh / tan e`, 3 km relief).
- Horizon maps buy nothing that a march over 350 km of heights cannot do; they are not needed as a far-field cache (see finding 4 for the one exception, the orbit view).

## Finding 2: data layout: a camera-centred height clipmap, filled from R
The compact tiles cannot be marched directly at an acceptable cost:

| Option | Per step | Problems | Verdict |
|---|---|---|---|
| March the storage-buffer slots through a page table | page lookup, slot header, 4 texel reads, a biquadratic base-surface evaluation, manual bilinear | about 10x a texture fetch; 6 face frames; tile edges | source only |
| Bindless per-tile textures (`TEXTURE_BINDING_ARRAY`) | 1 filtered fetch plus an indirection | native-only feature, non-uniform indexing on WARP and lavapipe unverified; duplicates the pool as textures; still 6 face frames | rejected |
| Tile atlas (33x33 per tile in a large 2D texture) | 1 fetch plus indirection | about 90 MB for 21k tiles of R; still a page table and face frames | rejected |
| **Clipmap**: N nested square levels around the camera's ground point, each a 2D-array layer, texel size doubling per level | 1 fetch (`textureLoad`) | one resample from tiles; one frame for all faces | **chosen** |

Clipmap layout (proposal; phase 2 confirms sizes):

| Item | Value |
|---|---|
| Levels | 10 to 12; level k texel `s0 * 2^k`, `s0` = 1 m near the ground (follows the finest resident tile under the camera), 1024 x 1024 texels a level |
| Coverage | level 0: 1 km; level 9: 512 km square (256 km radius); level 10: 1,024 km |
| Value | height above the sphere, `R32Float`, metres (no planet-scale value) |
| Max mips | per level, a max-height pyramid (`R32Float`, mips 1..10) for empty-space skipping |
| Memory | 1024^2 x 4 B x 1.33 = 5.6 MB a level; 12 levels 67 MB; coarse layer (finding 4) 8 MB; compare today's 56 MB coarse horizon plus chunk maps |
| Frame | a tangent plane at the camera's ground point (camera-relative f32); level centres snap to their own texel grid (toroidal addressing) |
| Projection | azimuthal around the anchor: crosses cube faces and edges without a seam, because tiles are resampled into it |
| wgpu | 2D array texture, `R32Float` storage write and `textureLoad`: core WebGPU, no native feature, works on WARP and lavapipe; no filtering needed (`FLOAT32_FILTERABLE` avoided) |

Filling it (gather, compute pass):
- The CPU (frame crate, GPU-free, CON-03) lists for each level the resident tiles of R that cover it, the finest available per region, and for each a texel rectangle and a small mapping from clipmap-local texel coordinates to the tile's `(u, v)`, built in f64 and handed over as tile-local f32 (CON-11).
- The compute pass reads the slot's texels from the existing storage-buffer blocks (no copy of the pool), bilinear in `(u, v)`, and writes the height. Then it rebuilds the max mips of the touched region.
- Work per frame: only regions whose tile changed (landing, eviction to an ancestor, edit refresh) and the strips a level scrolls. At the upload cap of 110 tiles a frame that is about 110 x 33^2 = 120k texels: under 0.05 ms. A teleport rebuilds all levels: 12 x 1M texels, about 0.3 ms once.
- R's horizon cut can drop an occluder that sits just below the camera's horizon yet shades a visible point (only for a sun within about 0.3 degrees of the receiver's horizon, at 50 to 100 km). The coarse layer covers it (finding 4).

## Finding 3: per-frame cost at 1440p on the RTX 3080 Ti Laptop
Hardware assumed: 58 SMs, 232 TMUs at about 1.4 GHz (about 325 Gtexel/s peak), 512 GB/s. Estimates; phase 2 measures.

| Variant | Rays | Steps a ray (mean, max-mip skipping) | Fetches | Peak time | Expected (20-30 % of peak, dependent divergent loop) |
|---|---|---|---|---|---|
| Full resolution | 3.69 M | 48 | 177 M | 0.54 ms | 1.8 - 2.7 ms |
| **Half resolution** | 0.92 M | 48 | 44 M | 0.14 ms | **0.45 - 0.7 ms** |
| Half resolution, 2-frame temporal stagger | 0.46 M | 48 | 22 M | 0.07 ms | 0.25 - 0.35 ms |

- Steps: a min-max (here max-only) quadtree march takes about 2-4 steps per clipmap level it crosses plus a refinement at the hit; a low sun crosses 8-10 levels (24-48 steps), a midday sun leaves the level's max height within a few steps. 48 is the planning worst-case mean.
- Soft edge for free: track the smallest `(ray height - terrain height) / distance` along the march and map it to the sun's 0.53 degree disc (a penumbra, no extra fetches).
- Upsample (depth-aware, 4 taps) and temporal accumulation (reprojection with a clamp to the neighbourhood): about 0.1 ms each at 1440p.
- Clipmap upkeep: under 0.05 ms a frame, 0.3 ms on a teleport.
- CPU main thread: the per-level tile lists change only when R or a tile changes: under 0.1 ms a frame, on the plan worker where possible.

Integration with the forward pass (`terrain` writes the lit colour before `sky-composite`):
1. **Inline**: march in the terrain fragment shader at full resolution. No new pass, but no half resolution and no temporal reuse; 1.8-2.7 ms. Usable as the phase 2 baseline only.
2. **Split sun term (recommended)**: `terrain` writes the unshadowed direct-sun radiance into a second target (`rg11b10` or `rgba16float`, 15-30 MB at 1440p, about 0.1 ms of bandwidth); a compute pass marches at half resolution from `depth`; `sky-composite` (which already reads `hdr-scene` and `depth`) adds `shadow x sun` before the aerial perspective. Water surfaces and seabed read the same shadow mask.

## Finding 4: the coarse layer and the orbit view
- From orbit the visible disc is thousands of km wide and a pixel covers kilometres; shadows matter only near the terminator.
- The coarse layer is a global height cube from the floor tiles (levels 3-4 are always present, LOD-011): 6 x 512^2 `R32Float` with max mips, about 8 MB, roughly 20 km texels.
- Rays that leave the outermost clipmap level continue in the coarse layer (same march, other texture). Beyond 350 km a ray can only hit relief above 9 km (finding 1), so the coarse layer mostly serves the orbit view and R's horizon-cut gap.
- Replaces GEN-045's coarse horizon (56 MB) and REND-043's distance fade.

## Finding 5: ADR 0083 share
| Share (1440p, Rasierklinge) | GPU | Main-thread CPU | VRAM |
|---|---|---|---|
| Layer 1, terrain occluders (clipmap upkeep, march, upsample, temporal) | 1.0 ms | 0.1 ms | 80 MB |
| Layer 2, object occluders (reserved, M10+) | 1.0 ms | 0.3 ms | 67 MB (4 cascades of 2048^2 depth32) |

- The 1.0 ms of layer 1 fits the expected 0.45-0.7 ms with margin; REND-044's 1.5 ms for horizon shadows plus material map then reads 1.0 ms for shadows outside the terrain pass and the material map's own share.

## Finding 6: edit latency
- Path: edit, tile refresh (STRM, 0.15-1.5 s per SPIKE-29), tile lands, the clipmap gathers its rectangle in the same frame, the next march sees it. Shadows follow the drawn terrain in the frame the tile lands; today's horizon maps re-bake per chunk in seconds after the neighbourhood settles.
- Temporal history must not hold the old shadow: the clipmap upkeep marks touched regions, and history is rejected where a ray's footprint crosses one (or, simpler, the history weight is capped so a change settles in 2-3 frames).
- Preview stamps drawn before the refresh: the shadow lags until the tile refresh lands. Evaluating the preview stamp in the gather removes the lag (later, if wanted).

## Finding 7: CON-11 and CON-14
- CON-11: the clipmap stores heights above the sphere (metres, up to about 10 km, f32 step 1 mm); positions are camera-relative f32 in the anchor's tangent frame; the tile mapping is built in f64 and handed over tile-local. The ray's curvature term `|p_h|^2 / (2R)` uses the constant `1/(2R)` (7.8e-8 per m), not a planet-scale value; shadows are presentation shading, so CON-11 v1.4 allows it under a declared bound tested at the worst case (1 m altitude, far from the origin, under camera motion). f32 error at 350 km camera-relative: about 2 cm.
- CON-14: shadows are lighting, presentation data with no CPU twin. The march has no atomics and no order-dependent reduction, so a given adapter draws the same pixels every run; goldens per adapter (ADR 0061) suffice. **The owner's exception (cross-machine identity) is not needed for layer 1.** It may be needed for hardware ray tracing (BVH builds differ by driver version).

## Finding 8: layer 2 (objects, M10+)
- **Receivers**: any pixel with a depth (terrain, a tower, a tree) marches layer 1 the same way; objects need nothing extra to receive terrain shadow.
- **Large static objects as layer-1 occluders**: buildings and forest canopy can splat a max height into the clipmap gather (an occluder height layer), so a tower or a forest edge casts long shadows at any distance at no march cost. Overhangs (bridges, arches) cannot be a heightfield: their far shadows are a stated limit.
- **Near objects**: cascaded shadow maps of objects only (terrain is not drawn into them, so no terrain acne and no terrain draw cost), 3-4 cascades out to about 2 km, combined as `min(layer 1, layer 2)`. Matrices camera-relative (CON-11).
- **Hardware ray tracing**: wgpu 30 has `EXPERIMENTAL_RAY_QUERY` (DX12 and Vulkan hal). Lavapipe exposes no ray query and WARP's support is unverified, and the feature is marked experimental. So it can only be an optional quality path beside the CSM, verified on the real GPU (`test gpu --real`), with CI goldens on the CSM path. Re-evaluate at M10.

## Recommendation
1. Layer 1: a per-pixel heightfield march at half resolution with max-mip skipping, a penumbra from the closest approach, temporal accumulation, over a camera-centred height clipmap (10-12 levels of 1024^2) gathered from the resident tiles of R, plus a global coarse height cube from the floor tiles; the planet's own shadow is analytic.
2. Retire the horizon maps (GEN-044/045, REND-038/041/043, STRM-048/060/064) once layer 1 lands; no far-field cache is needed (finding 1).
3. Split the sun term out of the terrain pass and apply the shadow in `sky-composite`.
4. Layer 2: object-only CSM near the camera; static large objects splat into the clipmap; hardware ray tracing optional, re-evaluated at M10.
5. Phase 2 measures the march at a sunset ridge and a gorge, full and half resolution, and confirms the clipmap sizes.

ADR draft: `adr-draft.md` (proposed; the architect files it under `docs/adr/`).

## Phase 2 plan (on opus-planner's go)
- Standalone prototype under `spikes/m9-shadow-architecture/proto/` (not committed as code in the spike PR): fill a clipmap from heights sampled from the store at two sites, march from a depth buffer of the same view, time with GPU timestamps.
- Sites: a sunset ridge (sun 1-3 degrees) and a gorge (sun 20-40 degrees, narrow shadow).
- Numbers: ms at full and half resolution, mean steps per ray, clipmap fill ms; stills of the shadow mask and the lit view.

## Visuals
Phase 1 is a desk study: nothing rendered. Phase 2 adds stills of the two sites.
