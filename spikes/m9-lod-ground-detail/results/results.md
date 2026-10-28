Opened: 2026-10-28
Question: How should M9 make ground detail at walking height anywhere (relief below today's finest octave of about 12 m down to 0.5 to 1 m, finer by shading): does the quadtree LOD stay or does another mesh scheme pay, and does the current reading of the GPU rules (CON-03, CON-07, CON-11, CON-14) cost more than it protects?
Why it matters: M9 scope (owner, 2026-10-28; `docs/roadmap.md` "M9 design constraints"); owner finding 13 (low-res ground at walking height).
Time box: 1.5 to 2.5 h wall clock (planned by opus-planner)
Success criteria: (1) coverage check of SPIKE-52; (2) LOD and mesh schemes at planet scale; (3) how shipped systems make sub-metre detail; (4) candidates for this project with cost, determinism, bounds, shadows and cheap experiments; (5) the GPU rules: what each protects and costs, how shipped systems stay deterministic on the GPU, keep / reinterpret / amend
Fallback if it fails: none (desk study)
Code location: none (research only)
Closed: 2026-10-28

## Method
- Three Sonnet research lanes (web, HTML text only, no file downloads): mesh LOD schemes, sub-metre ground detail, GPU-driven generation and determinism. Source checking, comparison and conclusions: Opus.
- Marks: **M** measured, **C** claimed, **I** inference of this study; **abs** abstract or summary only; **sec** community wiki or press. Sources that exist only as PDF are cited at abstract level.
- Engine numbers: SPIKE-44 (`spikes/compute-inventory/results/results.md`), ADR 0083, SPIKE-51, `docs/learnings.md` (`tile_cost_report`), `docs/status.md` and the gate with the chunk stage off (#847, #851).
- Process note: one lane's fetch of a PDF URL made the fetch tool save a copy outside the repo (the SIGGRAPH 2021 terrain-tessellation slides, unread); reported to the planner.

## Headline answers
1. **The quadtree stays.** It already reaches 7.46 cm vertex spacing (LOD-010) and, with the chunk stage off, holds back 0 tiles and shows no level steps on the real store (gate, #847). The gap is the height function (finest octave about 12 m), not the mesh. No surveyed scheme gives walking-height detail that the quadtree cannot, and the one measured planet-scale alternative (CBT) needs GPU-side LOD and fp64 on the GPU, which conflict with CON-03 and CON-11.
2. **Recommended split:** relief from 12 m down to about 0.5 to 1 m as CPU field octaves (the same function physics reads); below that, shading only (detail normals and procedural material detail in the pixel shader, presentation data under CON-14). Ground clutter (pebbles, rocks) later as GPU instances, which CON-03 already allows behind a CPU differential test.
3. **The GPU rules are not what kept the GPU idle.** CON-07 and CON-14 already allow a GPU implementation of a field within a stated tolerance; CON-14 already exempts presentation detail below the finest level. What filled the cores was the chunk stage (4 jobs x 4 threads = 16 of 20 threads, ADR 0083), now off by default (#851). Keep all four rules; one spec-level reading is worth writing down (part 5).
4. **Within 1 to 2 months:** candidates G1 (CPU octaves) and G3 (shader detail) fit; G2 (GPU vertex displacement) fits only as presentation and leaves physics a decimetre off; a mesh-scheme change (CBT, clusters) does not fit the horizon.

## 1. Coverage check of SPIKE-52
| Class of source | SPIKE-52 | Gap |
|---|---|---|
| Academic hydrology and erosion (multi-scale, stream power, network-first) | covered (16 papers) | none for its question |
| Engines and tools (planets, AAA, offline tiled builds) | covered at abstract or summary level | talk content behind GDC Vault or PDF |
| GIS tiled drainage (Barnes) | covered | none |
| **Rendering-side LOD and mesh schemes** (CBT, clipmaps, clusters, tessellation) | **not covered**: out of its question (hydrology against LOD consistency) | Benyoub and Dupuy 2024 [1] among them; assessed here |
| GPU-driven generation and placement | touched (HZD placement) | assessed here (part 5) |
| ML terrain (diffusion, super-resolution) | one abstract (InfiniteDiffusion) | not assessed; not in either question |
| Erosion on spheres or cube-spheres | searched, not found | still open |

- I: SPIKE-52's verdict (bake + per-point detail) does not depend on the mesh scheme; [1] does not change it (it renders a given displacement and says nothing on drainage).

## 2. LOD and mesh schemes at planet scale
| Scheme | Cracks | Precision at planet scale | Where LOD is decided | Budget / timing | Streaming, editing | Fit here |
|---|---|---|---|---|---|---|
| Quadtree + CDLOD morph (this engine; Strugar [2]) | morph to the parent grid; ADR 0074 stitch masks | tile-local f32 (CON-11) | CPU (`frame`) | this engine: GPU 4.3 ms per frame at 1 m, terrain 3.4 ms (SPIKE-44, M) | tile-keyed; edits re-plan locally | in place; 0 held back, no level steps without chunks |
| Concurrent binary trees, longest-edge bisection (Benyoub and Dupuy 2024 [1]; Dupuy 2020 [3] abs) | conforming bisection, no skirts or morph (C) | **fp64 on the GPU** for vertex positions; "otherwise discretization artifacts appear" (C) | entirely on the GPU; the CPU issues about 25 dispatches | M (RX 6800 XT): Earth-sized planet, 64k triangles, update 0.1 ms, render 1.745 ms; 7 MiB pool (D=17); "centimetric detail" C | not discussed | conflicts with CON-03 (GPU decides) and CON-11 (planet-scale positions in shaders); per-tile bounds and tile-keyed streaming do not map onto bisectors (I) |
| Geometry clipmaps (Losasso and Hoppe 2004 [4]) | not on the page | not on the page | implicit (camera rings) | C: 60 fps flight, 40 GB US heightmap compressed about 100x (2004) | incremental refill | flat-terrain design; camera rings conflict with tile-addressed generation (I) |
| Chunked LOD with skirts (Ulrich 2002 [5]) | geomorphs; skirts (paper, not on the page) | not covered | CPU | M: 16k^2 heightmap, 214 MB at 4 m error, about 30 fps (2002) | disk paging friendly | the ancestor of this design |
| Nanite-style cluster DAG [6] sec; Bevy meshlets [7] | locked group borders | not addressed | GPU traversal | M (Bevy, RTX 3080): 3,092 x 144k-triangle instances, about 2.78 ms GPU | streamed pages; static meshes | GPU LOD (CON-03); offline simplification clashes with on-demand generation and edits; wgpu lacks mesh shaders and 64-bit atomics (I) |
| GPU tessellation, mesh shaders (Englert 2020 [8] abs; Khoury et al. 2018 abs) | per technique | per technique | GPU | none measured | n/a | wgpu exposes neither hardware tessellation nor mesh shaders (I) |
| Far Cry 5 terrain [9] abs | GPU stitching | n/a | GPU compute pipeline (LOD, culling, stitching) | none | n/a | a shipped GPU-LOD example; abstract only |
| Planet engines: Outerra [10], Proland [11] abs, MSFS [12] abs, Star Citizen [13] sec | not documented | Outerra: a local origin per patch (C, forum) | not documented | no measured walking-height budgets found | MSFS: parent fallback while children load (C) | quadtree family is the norm (C) |

## 3. Sub-metre ground detail in shipped systems
| System | Geometry | Shading | Numbers | Depth |
|---|---|---|---|---|
| Outerra [10][14] | fractal refinement "down to centimeter details" on the GPU | pixel texturing from elevation, slope, curvature and fractal data | none | C, sec |
| SpaceEngine [15] | multi-octave noise on the GPU, read back to the CPU for the mesh and normals; detail to about 10 m | detail texture atlas by elevation and slope | C: generation 100 to 200 times faster on the GPU | full (blog) |
| Far Cry 4 adaptive virtual texture [16] | heightfield | virtual texture of 10 texels per cm over 10 x 10 km, terrain textures to about 300 m | C | abs |
| Frostbite, Battlefield 3 [17] | hierarchical heightfield | shader splatting, procedural virtual texture | none | abs |
| Far Cry 5 [9][18] | procedural cliffs and displacement geometry | n/a | none | abs |
| Horizon Zero Dawn [19] | GPU placement of vegetation and objects, "deterministic and locally stable" | n/a | none | abs |
| UE Nanite tessellation [20] | programmable displacement (map or procedural), landscape supported | n/a | limits: opaque/masked only, clamped WPO; a culling-bounds bug gives floating geometry when the output is scaled | full (docs) |
| No Man's Sky, Elite, Star Citizen | not documented beyond seeds and blended terrain types | n/a | none | sec |

- Common pattern (C across sources, I as a pattern): geometry carries relief to about the mesh's comfortable spacing; below that, detail is shading (normals, splatted or virtual textures, pixel noise) and instances (rocks, grass). No source documents collision for sub-metre displacement.

## 4. Candidates for this project
| Id | Approach | Scales | Frame cost (estimate) | Determinism | Bounds and held-back tiles | Horizon shadows | Physics / walking |
|---|---|---|---|---|---|---|---|
| G1 | **CPU field octaves**: extend the height function from 12 m to about 0.5 to 1 m (4 to 5 octaves, integer-hash gradient noise, `libm`), band-limited per level | 12 m to 0.5 m | not on the frame: tile build 2 ms per tile today (M); +5 octaves at 30 to 60 ns each per vertex (I) is +0.16 to 0.33 ms per 33 x 33 tile, 8 to 16 % | bit-exact (CON-14) | exact bounds grow by the octaves' summed amplitude (decimetres); risk: fine slopes raise neighbours' extents and could hold tiles back at the finest levels | no change (horizon maps are coarse) | the same CPU function: walking is exact |
| G2 | **GPU vertex displacement as presentation** (detail below the finest field octave, CON-14 carve-out) | 1 m to 10 cm | vertex shader per fine-level vertex, about 2.5 M vertices at 30 m (I) | integer-hash noise on the GPU; no CPU twin; within a stated bound | tile bounds must include the displacement amplitude | none | **physics sees the field without it: feet off by the amplitude**; a CPU-read ground makes it a field (CPU twin, ADR) |
| G3 | **Shader-only detail**: procedural detail normals and material noise in the pixel shader (presentation) | under 1 m | a few texture or noise taps per pixel; GPU has headroom (4.3 of 16.6 ms, M) | presentation bound and goldens | none | none (normals only) | none |
| G4 | **New mesh scheme** (CBT or clusters) | any | [1]: 0.1 ms update, 1.7 ms render (M, another GPU) | needs GPU LOD decisions and fp64 on the GPU | per-tile bounds lost | rework | rework |

- I: G1 + G3 is the roadmap's own split (geometry to 0.5 to 1 m, finer through the shader) and the architecture report's §4 plan; G2 is worth it only if G1's cost or held-back tiles fail; G4 is outside the 1 to 2 month horizon.

### Cheapest falsifying experiments
| Id | Experiment | Falsifies if | Cost |
|---|---|---|---|
| E1 | G1 on a spike branch: 4 to 5 octaves to 0.5 m; `tile_cost_report` per level; SPIKE-51's `held_flights_report` at margin 0 (chunks off) at 30 m and the owner poses; the gate's level steps | tile cost rises over about 25 %, any held-back tile, or a level step over 0.25 m p99 | half a day |
| E2 | G3: detail normals plus material noise; 1.8 m stills at the fixed sites (owner judges) and GPU time per pass | stills still read as smooth, or GPU time rises over 1 ms | half a day |
| E3 | G1 + G3 together at the same stills | the owner prefers a deeper geometric octave (then G2) | with E1 and E2 |
| E4 | G2 only if E1 fails: vertex displacement on the finest levels, GPU time and the foot error against the CPU field | the foot error is visible at 1.8 m | 1 day |

## 5. The GPU rules
### 5a. What each rule protects and what its reading costs
| Rule | Protects | Current reading | Cost measured here |
|---|---|---|---|
| CON-03: node selection is a pure CPU function; GPU culling only for instances behind a differential test | one plan for residency, cover, draw and tests; deterministic plans; plan text and hashes in repro bundles (CON-19) | terrain LOD on the CPU | plan job 1 thread; SPIKE-44 tail: plan and residency among the frames over 30 ms (ADR 0083 context) |
| CON-07: one implementation per field layer; CPU canonical; GPU within CON-14's tolerance | CPU systems (collision, picking, anchors) read the same data the eye sees | every field generated on the CPU | none forced by the rule: no GPU field was proposed |
| CON-11: f64 authoritative, f32 tile-local on the GPU | jitter-free geometry at 1 m anywhere | tile-local vertices | none; it rules out [1]'s fp64 positions |
| CON-14: bit-exact CPU; CPU/GPU agree per field within a bound; presentation (incl. "detail below the finest level") has no CPU twin | reproducible tiles, cross-OS hashes, goldens | heavy work on CPU threads | cores: median 4.7 to 5.4 of 20, bursts to 17.5 (SPIKE-44, M), mostly chunk jobs; GPU 4.3 of 16.6 ms (M) |

- I: the GPU headroom (about 12 ms a frame) was left by choice of work, not by a rule: no M8 work was a GPU candidate except the chunk solve, which is now off.

### 5b. GPU-driven systems and their testability
- GPU culling of instances is the common shipped pattern (AC Unity [21] sec, Frostbite [22] abs); it is visibility, so it needs no CPU twin. This is what CON-03 already allows.
- GPU-placed objects without a CPU replica are not physical: UE PCG GPU spawns instances that live only in GPU memory, unsupported for collision, navigation and ray tracing [23] (full, docs). This is the published precedent for "presentation has no CPU twin".
- GPU-generated ground with CPU physics uses readback: Outerra reads generated tiles back for CPU ray-terrain collision [24] (C, 2008 blog); SpaceEngine reads its GPU height maps back to build the mesh [15].
- GPU LOD on terrain ships (Far Cry 5 [9] abs; CBT [1]) but no source describes how such plans are tested against a reference.
- Star Citizen [13] sec: Planet Tech v4 shown 2019, delivered from Alpha 3.8; v5 (generation and spawning on the GPU) still planned in 2026 (C, community wiki and press); no measured numbers. About 7 years.
- GPU float results are not bit-exact against the CPU: shader `fma` is not required to be fused or exact (Vulkan [25], snippet); the WGSL spec's floating-point section (§15.7) was not read here (too long for the text fetch). I: a CPU/GPU tolerance with integer-hash lattices is the sound route, as CON-14 already states.

### 5c. Keep, reinterpret or amend
| Rule | Verdict | Reason |
|---|---|---|
| CON-03 | **keep** | the quadtree stays (part 2); ground clutter fits "GPU culling for instances"; no evidence the CPU plan limits ground detail |
| CON-07 | **keep** | already allows a GPU field within tolerance; physics needs the CPU function anyway (5d) |
| CON-11 | **keep** | G1 to G3 need no planet-scale values in shaders; it correctly rules out [1]'s fp64 positions |
| CON-14 | **keep; reinterpret in a spec** | state that shader-only detail (G3) is presentation with a bound and goldens, and that a GPU vertex displacement (G2) that CPU systems do not read is presentation only while its amplitude is in the tile bounds and the foot error is stated; any CPU reader of it makes it a field |

- No amendment proposed: none of the candidates that fit 1 to 2 months needs one.

### 5d. Where the ground must have a CPU evaluation
- Walking, physics, picking, placement anchors and collision read the field (CON-07); they must evaluate the same function the mesh shows, to its finest geometric octave. G1 meets this by construction.
- Detail that only shades (G3) needs no CPU evaluation; a GPU displacement (G2) does once anything walks on it.

## Findings
1. The quadtree is re-confirmed: 7.46 cm spacing, 0 held-back tiles and no level steps with chunks off; the missing detail is the height function.
2. The only measured planet-scale alternative, CBT [1], moves LOD decisions and fp64 positions onto the GPU and drops per-tile bounds: outside CON-03 and CON-11 and the 1 to 2 month horizon.
3. Shipped systems carry relief in geometry to about the mesh's comfortable spacing and the rest in shading and instances; none documents collision on sub-metre displacement.
4. The GPU stayed lightly loaded because the heavy M8 work (chunk solves) was a CPU design, not because of the rules; all four rules stay; CON-14 gets a spec-level statement for shader and displacement detail.
5. Next: E1 and E2 (half a day each) before any design.
6. Limits: no measured walking-height budgets for Outerra, Star Citizen or MSFS; talks behind GDC Vault or PDF only at abstract level; the WGSL floating-point rules not read; the frame and tile cost estimates of G1 and G2 are inferences.

## Sources
1. Benyoub and Dupuy 2024, Concurrent Binary Trees for Large-Scale Game Components: https://arxiv.org/abs/2407.02215 (HTML: https://arxiv.org/html/2407.02215v1)
2. Strugar, CDLOD: https://github.com/fstrugar/CDLOD
3. Dupuy 2020, Concurrent Binary Trees: https://diglib.eg.org/handle/10.1145/3406186 (abs)
4. Losasso and Hoppe 2004, geometry clipmaps: https://hhoppe.com/proj/geomclipmap/
5. Ulrich 2002, chunked LOD: https://tulrich.com/geekstuff/chunklod.html
6. Nanite course notes: https://cs418.cs.illinois.edu/website/text/nanite.html (sec)
7. Bevy virtual geometry: https://jms55.github.io/posts/2024-06-09-virtual-geometry-bevy-0-14/
8. Englert 2020, mesh shaders for CLOD terrain: https://history.siggraph.org/learning/using-mesh-shaders-for-continuous-level-of-detail-terrain-rendering-by-englert (abs)
9. Far Cry 5 terrain rendering: https://gdcvault.com/play/1025261/Terrain-Rendering-in-Far-Cry (abs)
10. Outerra: https://outerra.blogspot.com/2009/02/procedural-terrain-algorithm.html ; https://outerra.blogspot.com/2012/11/maximizing-depth-buffer-range-and.html
11. Bruneton and Neyret 2008: https://evasion.inrialpes.fr/Publications/2008/BN08 (abs)
12. Asobo terrain system (MSFS): PDF only, cited at summary level
13. Star Citizen Planet Tech v4 / v5: https://starcitizen.tools/Planet_Tech_v4 ; https://starcitizen.tools/Update%3AStar_Citizen_Alpha_4.11.0 (sec)
14. Outerra interview: https://gamedeveloper.com/business/-i-outerra-i-a-seamless-planet-rendering-engine
15. SpaceEngine: https://spaceengine.org/news/blog100824/
16. Far Cry 4 adaptive virtual texture: https://gdcvault.com/play/1021760/Adaptive-Virtual-Texture-Rendering-in (abs)
17. Terrain in Battlefield 3: https://gdcvault.com/play/1015414/Terrain-in-Battlefield-3-A (abs)
18. Procedural world generation of Far Cry 5: https://gdcvault.com/play/1025215/Procedural-World-Generation-of-Far (abs)
19. Horizon Zero Dawn GPU placement: https://www.guerrilla-games.com/read/gpu-based-procedural-placement-in-horizon-zero-dawn (abs)
20. UE Nanite: https://dev.epicgames.com/documentation/unreal-engine/nanite-virtualized-geometry-in-unreal-engine
21. AC Unity GPU-driven pipeline: https://www.dsogaming.com/?p=80038 (sec)
22. Wihlidal 2016, Frostbite: https://gdcvault.com/play/1023109/Optimizing-the-Graphics-Pipeline-With (abs)
23. UE PCG GPU: https://dev.epicgames.com/documentation/en-us/unreal-engine/using-pcg-with-gpu-processing-in-unreal-engine
24. Outerra collisions: https://outerra.blogspot.com/search?q=collision
25. Vulkan shader fma: https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_shader_fma.html (snippet)

## Visuals
Nothing to look at: a desk study, no renders.
