# Design: m8-horizon-shadows

Status: draft for the planner; the open questions need a decision or a measurement before the step that depends on them. Numbers are starting values from SPIKE-32 (`spikes/realism/results/results.md`) and become spec numbers when each step lands.

## The product
- A horizon map holds, per texel and per azimuth `k` of `A`, the elevation angle of the highest terrain along that azimuth out to the range. The shader compares the sun's elevation with the horizon interpolated between the two nearest azimuths, with a soft edge of fixed width.
- It does not depend on the sun, so any time of day uses the same map (baked once per chunk version).
- Two maps: the chunk's (4 m texels, range 5 km, from the chunk raster plus halo) and the coarse one (global lattice, range about 50 km). The horizon used at a point is the larger elevation of the two at the azimuth.

## Starting numbers (SPIKE-32)
| Quantity | Value |
|---|---|
| texel | 4 m |
| azimuths `A` | 16 (22.5 degree steps; fans near occluders) |
| range | 5 km (chunk), about 50 km (coarse) |
| step law | steps growing 4.5 % + 2 m |
| storage | 1 MB per km2 (RGBA8, four array layers) |
| bake | 0.7 s per km2, 6 threads |
| GPU | +0.06 ms at 1440p (two texture reads per fragment) |
| soft edge | -0.35 to +0.9 degrees around the horizon |

## Decisions (opus-planner, 2026-10-24)
1. **Texture plumbing:** a texture array with one layer per resident chunk slot (chunks are one size); a tile carries the slot index. The material map (M8-6) uses the same slot index in a parallel array. No atlas.
2. **Bake cost:** the horizon map is a second job that lands after the chunk's heights. Criterion 3 is about heights. Until the horizon lands, the coarse horizon from the global run shades the tile and the chunk horizon fades in (the stand-in rule, CON-22). Texel size and azimuth count are parameters (default 4 m, 16); the change reports the bake time per chunk.
3. **Debug view:** a lit `ShadeView` (horizon, shadow), no new key.
4. **Edits:** shadows lag an edit by design. An edit re-queues the horizon job of every chunk whose halo it touches. Precise edit handling belongs to M8-7 (to be noted in its change).

## Numbers claimed in the cache (no central registry)
| What | Number |
|---|---|
| `Kind::GlobalHorizon` (key kind and record kind byte) | 9 |
| `PARAM_HORIZON` (output-parameter tag) | 12 |
| coarse horizon codec version | 1 (`HORIZON_CODEC_VERSION`) |
| requirement IDs | GEN-044 to GEN-046, STRM-045, STRM-048, STRM-049, REND-038 (STRM-046 and STRM-047 are explorer's) |

## Measured (indicative, CPU only, not a gated perf run)
| Quantity | Value |
|---|---|
| bake, 16 azimuths, 5 km, 4 m texels, release build, one thread (2026-10-24, the GEN-044 bake) | 21 us per texel |
| chunk of 4.9 km edge (1225 x 1225 texels, 24 km2) | about 32 core-seconds, about 5 s on 6 threads |
| SPIKE-32 for comparison | 0.7 s per km2 on 6 threads (17 s for 24 km2) |

The measured figure is lower than the spike's, so the bake as a second job lands in seconds after the heights; a per-chunk figure on the real chunk raster is reported when step 4 runs the job.

## Questions (1, 2, 3 and 7 are decided above; kept for the reasoning)
1. **Texture plumbing.** The terrain bind group is buffers only today. A texture array of resident chunk horizons (layer per chunk, fixed texel count per chunk) against one atlas. The material map (M8-6) needs the same. Decide with the render owner before step 5; whichever change lands first creates it.
2. **Bake cost against the chunk landing target.** About 17 s for a 24 km2 chunk at 4 m texels and 16 azimuths exceeds the 10 s landing target (criterion 3) when added to the A-net job. Options: horizon texels at 8 m (4 times cheaper, softer shadows), horizon as a second job that lands after the heights (the chunk draws without shadows meanwhile, like the stand-in), 8 azimuths, or a coarser far part of the march. Measure on the real chunk raster in step 1 (CPU timing only, indicative, not a gated perf run).
3. **Azimuth count.** 16 gives fans near occluders; 32 doubles storage and bake. SPIKE-32 suggests 32 or a two-component fit. Decide from the stills of step 6.
4. **Debug view.** CON-18 needs one. Candidate: a new lit shade view (`horizon`: the horizon elevation at the sun's azimuth as grey, and `shadow`: the mask), next to temperature and precipitation (`ShadeView`), so no new key. If a key is wanted, one viewer key in the K pattern.
5. **Determinism and the CPU twin.** SPIKE-32 called the map presentation data without a CPU twin. CON-07 asks for a canonical CPU implementation of any field a GPU implementation matches. The bake is CPU; the shader only reads and interpolates. The interpolation gets a CPU twin (a function with the shader's arithmetic) and a tolerance test; the bake itself is canonical CPU code, bit-exact across threads (CON-14).
6. **Face edges and poles.** The azimuths of a bake are in the raster's own frame (`i` east, `j` north); a raster on another cube face is rotated against its neighbour's, so the lookup maps the sun's azimuth into the chunk's frame (GEN-046) and the same texel on two faces holds its 16 bytes rotated by the face rotation (a quarter turn is 4 indices). The march crosses chunk borders and face edges; the halo is taken through directions on the sphere like the coupled run's neighbours (GEN-040), not through face coordinates. Untested territory as for chunks (the chunk-field proposal's open risk).
7. **Edits.** An edit changes heights, so its chunk and every chunk within the horizon range could change their maps. The proposal follows the chunk invalidation rule (reach plus halo) and treats the range beyond the halo as stale until the next bake (shadows lag an edit; documented, not fixed here). Decision with R3 and M8-7.

## Render design (REND-038; read of `render/src/terrain.rs`, `compact.rs`, `reference.rs`, 2026-10-24)
Where the render PR touches the code, and how to keep it additive. Planner's rules: off by default (existing goldens unchanged), room for the chunk fade's per-node weight (REND-039), explorer is not editing `render/terrain.rs`.
1. **Per-node data in a table of its own, not in the node record.** `NODE_BYTES` (208) is asserted equal to the frame crate's `NODE_RECORD_BYTES` and the record comes from `TerrainDraw`; widening it changes the plan's text and hash. Instead a second storage buffer (binding 4) holds one `vec4` x 2 per plan node, indexed by the same node index: `[0] = (slot as f32, -1 = none; sun azimuth in the chunk's frame; sun elevation at the chunk centre; fade weight, reserved for REND-039)`, `[1] = (u0, v0, du, dv)` from `horizonview::tile_in_chunk`. A node outside a chunk, or whose chunk has no map, has slot -1. The renderer gets it from a setter beside `set_scale_refs` and `set_regions` (`set_horizon(&HorizonFrame)`), never from the plan.
2. **Textures.** Binding 5 `texture_2d_array<f32>` of `capacity x 4` layers of 256 x 256 `Rgba8Unorm` (the slots of `HorizonSlots`, `pack_map`'s layout), binding 6 the coarse map (six faces x 4 layers of the observer lattice as `Rgba8Unorm`, filterable, packed like `pack_map`), binding 7 a linear clamp sampler. Both start as 1 x 1 dummy textures so the group is always complete; the real array is created on the first `set_horizon` with a capacity and written with `queue.write_texture` per upload of `SlotUpdate`.
3. **Every pipeline that shares the shader needs the same layout.** `SHADER_HEAD` is shared by the compact vertex stage (`compact.rs`, built from `terrain.rs`'s `bgl`), the lit pipelines (`lit.rs` entry points) and the test-only reference renderer (`reference.rs` has its own `reference-bgl`, asserted against the product's node binding). Add the three bindings to the head and to both layouts at once; the reference renderer binds the dummies. A group with textures changes the pipeline layout of every terrain pipeline: the existing `gpu_*` terrain goldens are the check that nothing moved.
4. **Shader.** `fn shadow_factor(in: VsOut, up: vec3<f32>, sun: vec3<f32>) -> f32` in the lit fragment path only: the horizon at the tile's texel from the chunk map (`textureSampleLevel`, bilinear in the azimuth by two channel reads of the neighbouring azimuths, GEN-046's interpolation), the coarse map by the fragment's direction (cube face and (s, t) from the node's tile-local numbers and `up`, no planet-radius value, CON-11), the larger of the two, `smoothstep` with the soft edge of GEN-046; multiplies the direct term of `shade_lit` only. A node with slot -1 and no coarse map returns 1.0 (the term is the identity).
5. **Debug view.** `ShadeView::Horizon` and `ShadeView::Shadow` (appended last so the others keep their index; `ShadeView::ALL` and `index()` grow): the horizon elevation at the sun's azimuth as grey, and the lit factor as grey. Selected by `--shade-view` and the viewer's lit path like the temperature view.
6. **Viewer.** `enable_horizon` after `enable_chunks` and `horizon_frame` next to `chunk_frame` in the window's frame; the window builds the `HorizonFrame` (slots from `HorizonSlots`, per-node entries from the plan's nodes and `world.horizon_map`) and calls `set_horizon`; on with the lit path when chunks and the horizon are enabled, so the owner sees shadows at the look review.
7. **Tests.** Tier C: the shader's factor against `horizon_lookup` on read-back texels; a frame with no horizon equals the frame of a build without the term; the horizon and shadow views on a fixed pose (goldens only through `golden-review`; first goldens of a new class go to the owner's /bless). Tier A: the `HorizonFrame` builder (nodes to table, slot -1 cases).
8. **Split.** (a) `HorizonFrame` + per-node table builder in `frame` (GPU-free, tier A); (b) the render plumbing: bindings, dummies, `set_horizon`, uploads, with the term compiled in but the table all -1 (frames identical, tier C check); (c) the shader term and the factor test; (d) the shade views, the viewer call sites and the stills.

## Known approximation (numerics review of GEN-046, 2026-10-24)
- The bake (GEN-044) treats a chunk raster as square: it steps `texel_m` metres per texel along `i` and along `j`, and the direction it looks along is fixed in index space. Under the tangent warp a texel of a face-coordinate raster is up to 1.41 times as long along `i` as along `j` near the middle of a face edge (1.0 at the face centre), so the marched distances, and with them the horizon angle, are off by up to that factor along one axis there. The lookup is exact for the direction (the sun's azimuth is solved in index space, GEN-046); the distance error is a bias of the angle of a few degrees at the worst chunks. Fixing it means marching in metres through the true tangent vectors (`face_frame`), a change to GEN-044's march; it waits for the stills (step 6) to show whether the shadows of face-edge chunks need it.

- Shader reads (REND-038, step 5c): 16 azimuths exactly (the packers refuse other counts); both maps are read with a bilinear sampler, so between texel centres the shader blends neighbouring cells where the reference (`horizon_lookup`) takes the cell; the tests compare at texel centres (chunk horizon within 2e-5 rad, coarse within 3e-4 rad, factor within 8e-3 and 2e-2: the adapter's asin and texture weights). Across a cube-face edge the clamp sampler stops the blend, leaving a step of one cell's difference along the edge in the far-field mask. Judge both on the stills (step 6).
- The coarse map's azimuths are in the geographic frame of each cell's centre; the lookup uses the fragment's own geographic frame, so within about 5 coarse cells of a pole (about 100 km at 512) the frames differ by up to the longitude difference. CPU and GPU share this; a pole-free frame (the face raster frame) is the fix if the stills show it.
- A map arriving switches a fragment from the identity to `lit_factor(el, 0)` (a sun under 0.9 degrees on flat ground dims at once); the chunk fade does not hide it.

## Step 6 plan (opus-planner, 2026-10-25; reversible)
- 6a: `ShadeView::Horizon` and `Shadow`, tier-C views, pending goldens. 6c (own PR): the world loads the stored coarse horizon channel (kind 9) on a thread beside the store, never on the frame path (CON-22); making it from the coupled run is an explicit generate/xtask step; no stored channel gives chunk horizon only and one line saying how to make it; tier A on load and absence. 6b: viewer wiring in `window.rs` after explorer's field-stage change is on main, hooked into that same path. Then a small PR for `stills.rs` (horizon jobs, `set_horizon`, wait per pose for chunk and coarse maps), after explorer's hilly-site anchor change; stills through golden-review.

## Risks
- The sunset look needs shadows from terrain 20 km away; the coarse horizon at a lattice step of 4.9 km (2048 cells) can show a blocky outline near the combination. Check on stills.
- The texture array limits (layers per array, 2048 on common hardware) cap resident chunks; eviction is already by need.
- Seams: two adjacent chunks must agree at their border to the texel; the shared halo makes the values identical by construction (tier A).
