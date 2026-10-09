# Planet-Scale Generator + Editor + Real-Time Renderer: Architecture Survey and Decision Report

**Status:** Revision 4 (2026-10-09).
- Revision 1 was the original research output.
- Revision 2 (2026-10-06) changed the development workflow to **Windows-native development (VS Code + Claude Code), no WSL**, with Linux coverage from CI.
- Revision 3 (2026-10-09) adds the **testing and iteration design** (new Section 16: verification tiers, the frame-plan boundary that keeps the renderer a thin executor, debug views, repro bundles, contract and metamorphic tests, loop time budgets) and a **de-risking review** (new Section 17). Sections changed: TL;DR, Section 8 (GPU-driven note), Section 9 (verification now points to Section 16), Section 12 (spec traceability, new ADRs), Section 14 (new risks), Section 15 (walking skeleton in M0/M1, minimal async path in M2, go/no-go gates), Recommendations, Caveats.
- Revision 4 (2026-10-09) adds Rasierklinge's GPU (RTX 3080 Ti Laptop, 16 GB) as the reference machine with laptop-specific harness rules (Section 8, risk register, Section 17 item 14), and picks OpenSpec as the spec tool (Section 12). A separate starter doc covers tooling setup, `CLAUDE.md`, project agents and skills.

**Recommended direction:** build **one application with three run modes (interactive editor/viewer, headless generator CLI, headless test/render harness) on top of a stack of strictly layered library crates, written in Rust on wgpu**. Use a **cube-sphere quadtree with a CDLOD-style continuous LOD**. Store positions as **planet-fixed float64 (or integer-cell + float offset)**, render **camera-relative in float32** with **reversed-Z and an infinite far plane**. Treat generation as **pure, versioned functions of (WorldDefinition hash, generator version, tile id)** behind a content-addressed cache. Model authored content as **intent-level, geo-anchored layers (splines, stamps, masks, constraints)** stored separately from the procedural base, with **strict generator-version pinning plus freeze/bake** as the guaranteed fallback (surviving generator changes is a nice-to-have, not a requirement). Develop **Windows-native** on "Rasierklinge". Confidence: high on the coordinate, LOD and pure-generation choices. Medium on Rust + wgpu over C++/Vulkan (it depends on wgpu feature maturity for bindless and sparse resources). Low-to-medium on erosion and city strategies, which need spikes.

## TL;DR

- **Architecture:** one binary with modes (editor, `generate` CLI, `test-render`) over layered crates (`core-math/coords → world-def/fields → generators → cache/streaming → render → editor → app`). Generation is a pure function of `(definition hash, generator version, tile id)`, cached by content address. This keeps a later cloud tile service a deployment change rather than a rewrite.
- **Hard-to-reverse decisions to make now:** float64 planet-fixed coordinates with camera-relative float32 rendering, and reversed-Z with infinite far. Also: the cube-sphere face mapping and tile addressing, the generator-versioning/determinism policy, the authored-layer data model (geo-anchored, intent-level, separate from the base), GPU-driven rendering assumptions, and the graphics API layer. These are the things Star Citizen, per Chris Roberts' "Letter from the Chairman" on RSI, spent "eight months converting the engine to 64 bit precision" to retrofit.
- **Dev workflow (revised):** one environment, **Windows native**: VS Code + rust-analyzer + Claude Code on Rasierklinge, MSVC Rust toolchain, repo on local NTFS. WSL is not part of the workflow. WSL2 Vulkan goes through Mesa's non-conformant "dzn" (Vulkan 1.2 over D3D12), which is unsuitable for GPU development, and a second Linux checkout adds sync cost for no benefit. Linux coverage (build, clippy, CPU tests, lavapipe golden images) runs in CI. Claude verifies its work with CPU reference implementations, numeric precision tests, golden images rendered in software (lavapipe/WARP, which is how wgpu's own CI runs GPU tests), and a performance harness that runs on the real GPU locally.
- **Testability by construction (Revision 3):** everything that decides *what* to draw is computed in GPU-free code and handed to the renderer as a plain **frame plan**. The renderer only executes it. Requirements are verified at the cheapest of four tiers (pure, CPU oracle, pixels, human), so the hard-to-test visual surface stays small and integration work focuses on that gap. See Section 16.
- **Biggest de-risking moves (Revision 3):** keep the MVP free of research-grade pieces (global hydrology and cities come after the vertical slice), make cross-platform determinism real (pure-Rust math for transcendentals, one implementation per field layer per cache), make cache keys verifiable, and add go/no-go gates after M2 and M5. See Section 17.

## Key Findings

1. **Precision is solved, but only if designed in from day one.** Float32 world positions at Earth radius quantize to about 0.5 m, so they are unusable for ground detail. Every successful system converges on high-precision CPU coordinates plus camera-relative float32 rendering. UE5 Large World Coordinates raised maximum world size "from 21km to 88,000,000km". Epic's first GPU approach was tile-based and "has a number of limitations including imprecision and jittering near tile boundaries", so UE 5.4 refactored to a DoubleFloat representation and recommends "translated world space" (camera-relative) for performance. Star Citizen describes 64-bit world space that keeps "32-bit precision for rendering" via camera-relative rendering. KSP fixed the "Deep-Space Kraken" with a floating origin (Krakensbane) plus a separate "scaled space" scene. Bevy's big_space uses integer grid cells plus f32 offsets ("32 bits to address into a large integer grid, and 32 bits of floating point precision within a grid cell").
2. **GPU double precision is not an escape hatch on consumer cards.** NVIDIA Ada (RTX 40) has 2 FP64 cores vs 128 FP32 cores per SM (1:64; RTX 4090 ≈ 82.6 TFLOPS FP32 vs ≈ 1.29 TFLOPS FP64). AMD RDNA 3 (RX 7900 XTX) is 1:32 against its dual-issue FP32 figure. Sources disagree on RDNA 4 (Wikipedia/TechPowerUp-derived data imply 1:64, Geeks3d says 1:32). Design shaders to be float32 everywhere, with doubles only on the CPU (or emulated double-float in rare shader paths).
3. **Depth precision:** reversed-Z float32 is the default answer. Outerra, which pioneered the logarithmic depth buffer for planets, later wrote that a "reversed 32bit floating point depth buffer brings slightly better resolution than a 24bit logarithmic buffer… consistent across the whole 9 decades". Writing log depth in the fragment shader "disables several depth buffer optimizations". Log depth is therefore a fallback, not the plan.
4. **Terrain LOD:** a cube-sphere quadtree of regular grid patches with continuous morphing (CDLOD) is the proven planetary workhorse. Strugar's CDLOD uses a LOD function "based on the precise three-dimensional distance between the observer and the terrain", with "no need for stitching meshes". Elite Dangerous similarly uses fixed-size patches "in the same square arrangement… We blend between detail levels of patches". Face mapping matters. In Lambers & Kolb 2012 ("Ellipsoidal Cube Maps for Accurate Rendering of Planetary-Scale Terrain Data"), maximum angular distortion "is 31.1° for both Gnomonic and Adjusted Gnomonic projection, and 24.9° for QSC projection", and Adjusted Gnomonic reduces area distortion "to a maximum ratio of 1.41". The 2016 Facta Universitatis comparison puts the tangential spherical cube at about 5.2.
5. **Generation must be local, and erosion is not.** Real fluvial erosion is non-local: drainage area depends on the whole upstream basin. The literature offers two complementary escapes. One is coarse global simulation in an "uplift domain" with stream-power erosion, as in Schott, Paris, Fournier, Guérin & Galin (ACM TOG 42(5), 2023), where "the resolution of the cells in the stream power erosion simulation ranges from 80 to 600 meters" and which accepts "point and curve elevation constraints". The other is local, per-point erosion-like filters. Rune Skovbo Johansen's erosion filter, released March 31, 2026 with shader source and the blog post "Fast and Gorgeous Erosion Filter", "emulates erosion without simulation, so it's fast, GPU friendly, and trivial to generate in chunks". The recommended pipeline uses both: global coarse hydrology baked once per definition, then local amplification per tile.
6. **Data volume forces procedural-first.** A full Earth at 1 m elevation spacing is about 1 PB of raw 16-bit heights (before textures). For comparison, Jorge Neumann, head of Microsoft Flight Simulator, told PC Gamer "We have two and half petabytes (2,500,000 GB) of aerial data, obviously we can't install that, so we stream that", and Microsoft's Xbox Wire preview put the MSFS 2024 install at "about 30 GB". Locally, bake only coarse global fields (≤1 km to 100 m) plus authored regions, and synthesize everything finer on demand.
7. **Atmosphere:** Hillaire 2020 ("A Scalable and Production Ready Sky and Atmosphere Rendering Technique", EGSR/CGF, the technique used in Unreal) works from ground to space with small LUTs (transmittance, multi-scattering, sky-view, aerial-perspective froxels) and "does not require any high dimensional Lookup Tables". It is the right first atmosphere. Parameters are physical, so non-Earth planets come cheaply.
8. **WSL2 GPU reality (time-sensitive, as of 2025–2026), and why the workflow drops it:** WSL exposes the GPU "as D3D12/CUDA, with no Linux NVIDIA driver or ICD". Ubuntu's stock Mesa omits dzn, so Vulkan falls back to llvmpipe unless you use the kisak-mesa PPA. dzn prints "not a conformant Vulkan implementation, testing use only", reports Vulkan 1.2, ran at about 2/3 of native speed in one llama.cpp report, and has reported vkCreateDevice crashes and rendering artifacts. CUDA in WSL works well. Conclusion: WSL gives no GPU-development benefit, and keeping a second Linux checkout in sync adds cost. Its only plausible role was Linux parity, which CI covers without any local setup. WSL would come back only for CUDA-based offline ML experiments (for example terrain super-resolution) or a Linux-only tool.
9. **Agent verification is achievable without a GPU.** wgpu's CI runs its GPU tests on "Windows/DX12 using WARP", "Linux/Vulkan using lavapipe" and "Linux/OpenGL ES using llvmpipe". No Man's Sky's team built "automated probes to visit the various planets and take images to review". That is exactly the pattern needed for Claude: scripted camera paths, then offscreen PNGs, then metric and golden-image checks. With Claude Code running locally on Rasierklinge, the same harness can also run on the real GPU and Claude can read the screenshots and performance logs directly.
10. **Spec-driven tooling has converged on "constitution → specify → plan → tasks → implement".** GitHub Spec Kit describes a "constitution", a document that captures "the non-negotiable principles for a project", run once per project, followed by per-feature specify → plan → tasks → implement → converge. For this project, use that skeleton, but add ADRs, a spike register and a learnings log so specs stay evergreen.

## Details

### 1. Case studies

| System | Terrain representation | LOD / streaming | Precision strategy | Authored content | Lesson for this project |
|---|---|---|---|---|---|
| **Outerra (Anteworld)** | Real elevation data refined fractally at runtime on a cube-ish planet (approx. equal-area cube projection per the 2016 SCM comparison) | Quadtree tiles generated/refined on GPU, streamed coarse real data | Camera-relative; pioneered log depth, later recommended reversed float depth | Roads/objects placed on refined terrain | Real data + procedural amplification works at 1:1 Earth; depth strategy evolved toward reversed-Z |
| **Space Engine** | Procedural planets, cube-sphere quadtree, heavy GPU generation | Generated tiles cached on demand | Hierarchical frames (universe → system → planet) | Minimal | Pure procedural can scale to universes; authoring is the weak spot |
| **Kerbal Space Program** | PQS (procedural quad sphere) per body + low-poly "scaled space" proxy | Quad subdivision near camera; "PQS subvision methods" tuned for object count | Floating origin (Krakensbane), separate scaled-space scene with its own floating origin | Hand-placed KSC etc. on PQS | Multi-scene/multi-scale split is a cheap, effective far-field trick; precision bugs surface as physics explosions |
| **Star Citizen (Star Engine/CryEngine)** | Procedural planets + authored "object containers" | Object Container Streaming (client, then server) | 64-bit world, 32-bit camera-relative rendering, Zone system, local physics grids | Containers hand-authored, placed into procedural planets | Retrofitting 64-bit took eight months per Chris Roberts' Letter from the Chairman; design zones/local grids from the start |
| **Elite Dangerous (Stellar Forge)** | Physically motivated generation from system formation down to craters and tectonics | Fixed-size patches with LOD blending, used for both render and physics meshes | Hierarchical frames | POIs placed after "the entire stellar body must be fully generated… tens of seconds" | Global dependencies (site placement on full topography) cost latency; precompute coarse global fields |
| **No Man's Sky** | Voxels: octrees on cube faces projected to spheres; polygonized | Continuous generation pipeline: voxel gen → polygonize → texture → populate | Local frames | Base building as voxel edits | Voxel gives caves/overhangs at large cost; automated probe screenshots for QA |
| **MS Flight Simulator 2020/2024** | Real DEM + Bing imagery + photogrammetry + AI-generated buildings/trees | Cloud streaming of 2.5 PB (per Jorge Neumann); thin client about 30 GB per Microsoft's Xbox Wire preview | Double/camera-relative (engine internals not public) | Handcrafted airports/cities as overlay packages | Proves the cloud-tile model; also proves local-only at 1 m Earth is impossible without procedural |
| **Cesium / 3D Tiles / cesium-native** | Quantized-mesh TIN terrain; 3D Tiles HLOD trees; implicit quadtree/octree tiling with availability bitstreams | Screen-space-error driven HLOD refinement | WGS84 ECEF globally; per-tile RTC_CENTER / transforms for float32 | Tilesets layered over terrain | Use its tile-tree/geometric-error concepts and implicit tiling; adopt ECEF + local ENU frames |
| **UE5** | Landscape (flat heightfield), World Partition grid, HLOD, PCG, Nanite | World Partition cells + HLOD; Nanite cluster LOD | LWC: double CPU; GPU moved from tile-offset to DoubleFloat + translated world space (5.4) | Data layers, One File Per Actor, PCG graphs | Best reference for editor/data-layer/PCG design; not natively a sphere engine |
| **Godot 4** | Heightmap plugins (Terrain3D, Zylann) | Clipmap/chunked | `precision=double` build; docs note most AAA open worlds still use float | Scene files | Double builds exist but cost perf; planetary terrain is community-only |
| **Bevy / big_space** | Ecosystem crates (bevy_terrain etc.) | ECS-based | Integer grid (i8–i128) + f32 offset; nested reference frames; bevy_a5 puts pentagonal geo-cells under the world | ECS scenes | big_space's model is the cleanest published coordinate pattern; directly reusable as design |
| **Proland / Bruneton** | Quadtree producers with GPU-generated tiles; precomputed scattering (2008), ocean (2010) | Producer/consumer tile cache | Camera-relative | Vector data (roads/rivers) rasterized into tiles | The "producer graph + tile cache" architecture is the ancestor of what's proposed here |
| **Houdini / Gaea / World Machine / WorldCreator** | Heightfield node graphs with erosion nodes | Offline, tiled builds | n/a | Masks, stamps, splines as graph inputs | Node-graph-with-masks is the authoring UI users expect; erosion is a bounded-region offline op |
| **Space/Medieval Engineers, Minecraft/Vintage Story** | Voxel planets / voxel columns | Chunked voxel streaming | Local frames | Player edits stored as voxel deltas | Voxels make edits trivially local but rendering/memory far costlier from orbit |

*Caveat:* the Outerra, Space Engine and MSFS internals are partly inferred from blogs and press, not formal documentation. MSFS's "1.5 billion buildings / 2 trillion trees" figures come from secondary summaries.

### 2. Terrain representation and LOD

| Option | Strengths | Weaknesses | Verdict |
|---|---|---|---|
| Cube-sphere quadtree (6 roots) | Simple quadtree math, regular grids, GPU-friendly, trivial tile ids | Face-edge seams, mapping distortion | **Choose.** Use a tangent-adjusted or approx. equal-area mapping (e.g., the JCGT 2018 approximately equal-area cube-to-sphere warps) |
| HEALPix / S2 / QSC | Equal-area or well-studied cells, GIS tooling (S2) | More complex per-pixel math; S2 cells aren't regular grids per face | Borrow S2's Hilbert-curve cell ids for storage ordering, not for rendering |
| Icosahedral / H3 / A5 | Low distortion, hex/pentagon neighborhoods | Awkward for texture grids and quadtrees | Non-goal for terrain; possible for gameplay cells later |
| Lat-long / TMS / quantized-mesh | Interop with GIS data | Pole singularities, huge distortion | Import-only (reproject into cube faces) |
| Geometry clipmaps (spherical) | Constant memory, great streaming | Hard with per-tile procedural generation and edits; awkward on sphere | Use for *textures* (virtual texture clipmaps), not geometry |
| CDLOD on the quadtree | Predictable 3D-distance LOD, smooth morph, no stitching | Vertex texture fetch per vertex | **Choose for MVP** |
| Mesh shaders / meshlet cluster LOD (Nanite-like) | Lowest CPU cost, fine culling | Feature availability (wgpu mesh shaders are experimental), complexity | Design the data path so a later meshlet renderer can consume the same tiles |
| GPU tessellation | Adaptive detail | Poor on some vendors, not in WebGPU | Skip |
| Voxel / transvoxel / dual contouring | Caves, overhangs, arbitrary edits | 10–100× memory, hard far-field | COULD-tier: local volumetric patches attached to heightfield tiles |

Design notes:
- **Seams:** sample heights on a shared border (tile n+1 samples with one-sample overlap). Compute normals from a skirt or overlap ring. At cube-face edges, compute everything from the *3D unit-sphere direction*, never from face UV. Then neighboring tiles on different faces evaluate identical functions at identical points. Keep CDLOD morphing plus skirts as a belt-and-braces approach against T-junction cracks.
- **Poles:** the cube sphere has no pole singularity. Face corners (8 points where 3 faces meet) are the only special cases.
- **Surface precision:** generate tile vertices relative to a per-tile origin (the tile center in planet-fixed float64). Upload float32 offsets. Each frame, compute `tile_origin − camera_position` in float64 on the CPU and pass it as a float32 uniform. Vertex precision is then bounded by the tile size, not the planet size.
- **Collision:** derive collision meshes from the same tile height samples at a fixed LOD around physics bodies. Physics runs in a local frame (Jolt offers `JPH_DOUBLE_PRECISION` with double positions and float elsewhere).

### 3. Numerics and coordinates

**Precision table (spacing between adjacent representable values, i.e. ULP):**

| Magnitude | Example | float32 ULP | float64 ULP |
|---|---|---|---|
| 1 km | local scene | ~0.06 mm | ~1e-13 m |
| 10 km | big level | ~1 mm | ~2e-12 m |
| 100 km | regional | ~8 mm | ~1.5e-11 m |
| 6,371 km | planet radius (ECEF) | **0.5 m** | ~1 nm |
| 384,000 km | Moon distance | 32 m | ~60 nm |
| 1.5e11 m | 1 AU | 16 km | ~30 µm |

Implication: float32 is fine within about 10 km of the origin for 1 mm work. Float64 planet-fixed gives nanometers at the surface and still about 30 µm at solar-system scale. So float64 is enough for the eventual multi-planet extension. Integer-cell schemes (big_space i64 cell + f32) are only needed for galaxy scale.

**Depth:** reversed-Z, D32F, infinite far plane, near ≈ 0.05–0.1 m. Relative depth error is about 2^-23 of distance, i.e. about 1 m at 10,000 km. That is ample, because at that range only the planet limb and atmosphere are visible. Keep a log-depth shader path as a documented fallback. Avoid multi-frustum splitting (KSP style) unless profiling shows z-fighting on huge near/far ranges.

**Frames (recommended):**
- `Universe/System` (float64 or i64 cell; later) → `PlanetInertial` (float64; rotation applied for day/night and orbit) → `PlanetFixed` (float64 ECEF-like, the authoritative frame for all world data) → `TileLocal` (float32, origin = tile center) → `CameraRelative` (float32; what the GPU sees) → `Local tangent ENU frame` (float64 origin + float32 offsets; used for authored objects, physics islands and editor gizmos).
- Geo-coordinates (lat, lon, height above reference ellipsoid or sphere) are the *serialization* format for authored anchors. That keeps them stable if the planet's internal frame conventions change.

**Precision budget (constitution-level invariant):** render jitter < 0.25 px at 1 m camera distance. Tile-vertex quantization ≤ 1 mm at the finest LOD. Authored anchor round-trip error ≤ 1 mm. Generator determinism: bit-exact on CPU across runs and machines for integer/hash stages. ≤ 1e-4 relative tolerance for float stages across CPU/GPU, with tests asserting both.

**Determinism policy:** make the hash/RNG and the lattice/noise lookups integer-based (e.g., PCG/xxhash on integer lattice coordinates derived from float64 positions *snapped to the tile grid*). Avoid transcendental functions in shaders where bit-equality matters, because GPU `sin/exp` differ by vendor. On the CPU, use the pure-Rust `libm` crate for transcendentals in generator code, because the platform maths libraries differ between Windows (MSVC) and Linux (glibc); no fast-math; fixed-order reductions (Revision 3). Treat GPU-generated tiles as *cacheable but not canonical*. Canonical outputs (for tests, the cloud cache and authored-edit baselines) come from the CPU reference implementation, or from a GPU path validated against it within tolerance.

### 4. Streaming and data management

**Size model (Earth surface ≈ 5.1×10^8 km², land ≈ 29%; one 16-bit height channel, +33% for the pyramid, before compression):**

| Spacing | Samples | Raw height | Notes |
|---|---|---|---|
| 1 km | 5.1e8 | ~1.4 GB | Bake globally (all fields: height, climate, biome, flow) |
| 100 m | 5.1e10 | ~136 GB | Bake land only (~40 GB), or generate on demand; compresses well |
| 10 m | 5.1e12 | ~13.6 TB | Procedural on demand; cache only visited regions |
| 1 m | 5.1e14 | ~1.4 PB | Procedural only (MSFS-scale data volume) |
| 10 cm | 5.1e16 | ~136 PB | Procedural micro-detail in shaders (displacement/normal) only |

Quadtree depth: a cube face edge is about 10,000 km. With 256-sample tiles, 1 m spacing needs about 16 levels, and 10 cm about 19 levels below the face root.

**Cache tiers:** VRAM (tile atlas + page tables) ← RAM LRU (decoded tiles) ← disk cache (content-addressed, compressed, keyed by `hash(def_subgraph, generator_version, implementation_id, tile_id)`; the implementation id keeps CPU- and GPU-generated tiles of one layer from mixing, Revision 3) ← generator (CPU/GPU) ← optional remote tile server (later). Use the same key everywhere. That single decision is what makes cloud support "deferrable but cheap".

**Formats:**
- Baked tiles go in a custom pack format: an append-only blob file plus an index (LMDB or SQLite), with Zstd/LZ4 for heights and fields and BC7/BC5 for textures (GDeflate or DirectStorage later).
- Definitions are text (TOML/JSON), diffable in git.
- Interop: import GeoTIFF/COG, GeoJSON, OSM PBF and glTF. Export glTF and possibly 3D Tiles for viewing in Cesium.

**Scheduling:** a priority queue keyed by screen-space error × visibility, with velocity-based prefetch along the camera trajectory. During fast descent, request the parent chain first: always have *some* LOD, and never block. Jobs are cancellable (generation jobs check a token). Uploads are frame-budgeted (e.g., ≤ 2 ms/frame, ≤ 64 MB/frame of uploads). Use a work-stealing pool (Rayon or a custom task graph) with separate queues for I/O, CPU generation and GPU generation.

**Sparse residency:** do *not* depend on Vulkan sparse or D3D12 tiled resources, which wgpu does not expose. Implement software virtual texturing: a physical page atlas plus an indirection texture, or simply per-tile texture-array slices for MVP.

### 5. Procedural generation pipeline

**Field abstraction (keep from the exploration doc):** `Field<T>: eval(position_planet_fixed, lod) -> T`, composable, with declared footprint (local vs needs-neighborhood vs global). Three classes:
1. **Global-coarse fields** (continents, plates/uplift, climate, drainage basins, rivers as graphs, coastlines, settlement suitability). These are computed once per definition at 1–10 km on the whole sphere (a 1 km cube-sphere grid is about 5e8 cells, feasible on the GPU in minutes) and baked.
2. **Regional fields** needing a neighborhood (erosion amplification, river carving, road cuts). These are computed per tile with an apron or halo (e.g., tile + 25% border) from global-coarse inputs, which makes them deterministic and tile-independent.
3. **Local fields** (fBm/ridged/domain-warped detail, Johansen-style erosion filter, micro-detail). These are pure per-point and can run in shaders.

**Erosion recommendation:**
- Run a Schott-style uplift + stream-power simulation on the global-coarse grid. Its 80–600 m resolution range matches.
- Extract river networks as **graphs/splines** (data the exploration doc already wants).
- Then carve rivers locally and apply the local erosion filter for gullies at the finest scales.
- Avoid particle hydraulic erosion at runtime. Use it only offline, inside authored bounded regions, as a baked layer.

**Real data:** support importing SRTM/Copernicus DEM/GEBCO as a "base layer" reprojected into cube-face tiles. Procedural amplification then adds detail, as Outerra does. This is a COULD for v1, but it shares the layer machinery.

**Incremental invalidation:** treat the WorldDefinition as a DAG. Each node has a content hash, and each tile key includes the hashes of the nodes it reads (Bazel/Salsa-style). Editing a stamp in Europe invalidates only tiles whose footprint intersects the stamp's bounds at each LOD. Use a spatial index (R-tree over the planet-fixed AABB, or the cube-face quadtree) over authored items.

### 6. Authored content and the algorithm-change question

**Requirement level (user decision, 2026-10-06):** surviving generator-algorithm changes is a **nice-to-have**. "Generator version changes do not support prior authored data" is an acceptable fallback. The design should keep the door open at low cost, not pay heavily for it.

**Layer stack (non-destructive, ordered, each with blend mode and mask):**
`Base procedural (pinned generator version)` → `Imported data layers` → `Intent constraints (consumed by generators)` → `Stamps/brushes (height delta, flatten, carve)` → `Splines (roads/rivers/rails with profiles)` → `Placed objects/landmarks (geo-anchored)` → `Frozen/baked regions (authoritative rasters)`.

**Options spectrum for surviving generator changes:**

| Strategy | What's stored | Survives algorithm change? | Cost |
|---|---|---|---|
| A. Version pinning | `generator_version` in the definition; old generator kept runnable | Not across versions; old worlds keep working | Low (keep old code paths or binaries) |
| B. Absolute rasters (height overrides) | Final heights in a region | Yes, but seams at borders with the new base | Medium; big data |
| C. Relative deltas | Δheight relative to base | Partially; shape preserved, absolute heights drift | Low |
| D. Geo-anchored vector intent | Splines/polygons/points with semantics ("road", "river", "city center", "flatten to h") | Mostly; regenerated against new base | Medium; generators must consume intent |
| E. Constraint-based | "River passes here", "peak ≥ 3,000 m here", "coast here" | Best, if the new generator honors constraints | High; generators become solvers (Schott 2023's point/curve constraints show feasibility at the coarse level) |
| F. Freeze/bake | Region baked to authoritative tiles at all LODs | Yes (by definition) | Storage; blending ring at border |
| G. Re-fit/migrate | Tool re-projects edits onto the new base (e.g., re-solve stamp so result matches old target) | Approximate | High; per-change tooling |

**Recommendation (minimal cost, door open):** A + C + D + F now; E for a few high-value intents (river paths, city centers, coastline nudges) once generators exist; G deferred. If a generator change breaks old authored data, the accepted answer is: keep the old version pinned, or freeze/bake affected regions first.
- Store every authored item with: geo-anchor (lat/lon/h + heading), semantic type, parameters, the base `generator_version` and `definition hash` it was authored against, and optionally a small "expected result" snapshot (e.g., height samples along a spline). The snapshot is cheap and enables future re-fitting.
- Prefer *relative* and *semantic* edits over absolute ones in the UI. "Flatten to local average + 2 m" survives better than "set to 412.7 m".

**Editor model:**
- Command pattern over the WorldDefinition (not over runtime tiles). Undo/redo is replaying or inverting definition commands. Runtime tiles are just invalidated and regenerated, so undo stays cheap on a streamed world.
- Each command is serialized to an append-only edit log (event-sourced), which doubles as collaboration and crash recovery.
- Storage: one file per authored item or per spatial cell (UE One File Per Actor idea) in TOML/JSON for git-merge-friendliness. Binary payloads (masks, baked regions) go in content-addressed blobs referenced by hash.
- Picking: ray-cast against CPU-side height tiles in float64 planet-fixed. GPU picking returns a tile id and local offset, which is then refined on the CPU.
- Camera: altitude-adaptive speed (speed ∝ height above terrain), with orbit, fly and walk modes.
- OpenUSD: worth an interop export later, not as the core format. It is heavy for an agent-built Rust codebase.

### 7. Cities, settlements, roads

- **Placement:** a suitability field (slope, water proximity, coast, climate, river confluences) produces settlement seeds by Poisson-disk sampling weighted by suitability, at the global-coarse stage. An inter-settlement road network comes from least-cost paths on a coarse cost field, stored as splines (the same data type as authored roads). Authored "city center here" intents simply inject or override seeds.
- **Street networks:** use tensor-field street modelling (Chen et al. 2008) for controllable, terrain-aligned grids, or L-system/agent growth (Parish & Müller 2001 / CityEngine). Tensor fields are easier to make deterministic per city and to steer with authored constraints.
- **Parcels and buildings:** subdivide blocks into lots (OBB recursive split / straight skeleton). Buildings come from footprint extrusion plus grammar-based facades (CGA/split grammars) or modular kits.
- **Scale:**
  - Earth-like planets carry on the order of 10^9 buildings (MSFS reportedly generates ~1.5 billion).
  - Aerial views over a metropolis can contain 10^5–10^6 buildings.
  - This requires a city HLOD: district-level merged meshes or impostor textures at >5 km, then instanced footprint extrusions with atlas textures at 0.5–5 km, then full LOD meshes below about 500 m.
  - Everything is GPU-driven: one indirect draw per material bucket, GPU frustum + Hi-Z culling. The budget target is ≤ 2,000 draw calls/frame, with 10^5–10^6 instances culled on the GPU.
- **Terrain interaction:** roads and city plots emit terrain stamps (flatten, cut/fill along the road profile) into the regional-field stage. One spline drives terrain, graphics and later gameplay, as the exploration doc proposes.

### 8. Rendering: what must be in the first architecture vs later

| Must be designed in from M2 (expensive to retrofit) | Can be added later |
|---|---|
| Camera-relative float32 everywhere; reversed-Z infinite far | Volumetric clouds (Nubis-style), cloud shadows |
| HDR linear pipeline with physical sun units + auto-exposure (10+ orders of magnitude brightness) | FFT ocean detail (start with a sphere-level ocean surface + Gerstner) |
| Render graph (pass/resource declaration, transient allocation) | Virtual shadow maps; ray-traced shadows/GI |
| GPU-driven submission (instance buffers, indirect draw, GPU culling) | Mesh-shader/meshlet path |
| Atmosphere as a "participating medium" service (Hillaire LUTs + aerial perspective applied to *all* geometry) | Night lights, stars catalogue, eclipses |
| Terrain material system with per-tile material/biome weights, triplanar on steep slopes | Parallax/displacement close-up, anti-tiling refinements |
| Temporal AA with camera-relative reprojection (motion vectors computed from float64 camera deltas) | DLSS/FSR/XeSS integration (vendor SDKs; availability via wgpu is limited and time-sensitive) |
| Shadow cascades fit in camera-relative space | Vegetation impostors and wind |

**GPU-driven, but decisions stay testable (Revision 3):** GPU-driven submission is designed in for *instances* (vegetation, city buildings, scatter), where counts make CPU culling impractical. Terrain node selection (which quadtree nodes at which LOD) stays a pure CPU function of camera and parameters. It is cheap (hundreds to a few thousand nodes) and fully testable at tier A. Any GPU culling path is added behind a differential test against a CPU reference (Section 16).

**Frame budget (60 FPS, 1440p, mid-range 8–12 GB GPU, internal render resolution can scale):**

| Pass | Budget (ms) |
|---|---|
| GPU culling + terrain/instance prep | 1.0 |
| Depth prepass + terrain + objects (G-buffer or forward+) | 5.0 |
| Shadows (CSM, 3–4 cascades) | 2.0 |
| Atmosphere LUTs + sky + aerial perspective | 1.0 |
| Lighting/composite, SSAO/GTAO | 2.5 |
| Ocean (when visible) | 1.0 |
| Post (TAA, bloom, tonemap) | 1.0 |
| Async tile generation on GPU (compute) | 1.5 |
| Headroom | 1.6 |

**VRAM budget (8 GB tier):** terrain tile atlases about 1.5 GB, material textures about 2 GB, instance data and city meshes about 1.5 GB, render targets about 0.8 GB, atmosphere/ocean about 0.2 GB, with the rest held as reserve. Quality tiers scale atlas sizes and LOD distance. RAM budget: 4–8 GB for decoded caches. Disk cache: user-configurable, default 20–50 GB.

**Reference hardware: Rasierklinge (Revision 4).** NVIDIA GeForce RTX 3080 Ti Laptop GPU: GA103 (Ampere), 7,424 shaders, 16 GB GDDR6 on a 256-bit bus (512 GB/s), laptop power limit (TGP) between 80 and 150–175 W depending on the laptop model (Notebookcheck). Consequences:
- **Performance tier:** depending on the power limit, roughly between a desktop RTX 3060 Ti and RTX 3070 (my estimate from Notebookcheck's 3DMark Time Spy figures: about 9,500 at 105 W and 13,500 at 175 W). That makes Rasierklinge a reasonable stand-in for the "mid-range" target. It becomes the **reference machine**: the frame budgets above are measured on it at 1440p internal resolution.
- **VRAM:** 16 GB is above the 8 GB tier. The residency manager gets a configurable hard VRAM cap. The performance harness runs with the cap at 8 GB (mid-range tier) and at 14 GB (high tier), so the 8 GB budget is enforced even though the card has more.
- **Laptop variance:** clocks depend on power mode, temperature and whether the charger is connected. The performance harness protocol is: charger connected, a fixed Windows and vendor power profile, a warm-up pass, several runs reporting medians and p99, and GPU clock, power and temperature logged with each run (via `nvidia-smi`/NVML). Regression thresholds are set from the run-to-run noise measured in M0, not guessed.
- **Hybrid graphics:** laptops with NVIDIA Optimus can hand the app the integrated GPU. The app requests the high-performance adapter, logs the adapter name at start-up, and the performance harness refuses to run on anything other than the RTX 3080 Ti. If the laptop has a MUX switch, use the discrete-GPU mode for performance runs, because Optimus routes the internal display through the integrated GPU.
- **Features:** Ampere supports mesh shaders and hardware ray tracing in Vulkan and D3D12, so those later paths can be tried on this machine. FP64 throughput is low (1:64), consistent with Key Finding 2. DLSS super resolution is available on this generation; DLSS frame generation is not.

### 9. Engine, language, API and development workflow

| Option | Pros | Cons | Fit for Claude-driven SDD |
|---|---|---|---|
| **Rust + wgpu (custom engine)** | Strong compile-time checks give the agent good feedback; Cargo/test tooling; wgpu runs on D3D12/Vulkan/Metal; v28 shipped mesh shaders ("fully supported on Vulkan") and experimental ray queries; headless tests on WARP/lavapipe | No sparse residency; bindless less mature; API churn each major release (v30 exists by Oct 2026); some features experimental | **Best overall.** Recommended |
| Rust + ash (raw Vulkan) | All features | Huge unsafe surface; more code to keep correct | Fallback HAL behind the same render-graph interface if wgpu blocks a feature |
| C++20 + Vulkan/D3D12 (+NVRHI/Diligent, VMA, Slang) | Maximum control, best tools (PIX/Nsight) | UB-prone, CMake/vcpkg friction; agent verification weaker | Viable but costlier to keep correct |
| Bevy (+big_space, bevy_terrain) | ECS, ecosystem, wgpu underneath | Engine churn every release; renderer not designed for planetary GPU-driven terrain; editor immature | Use as a source of crates and patterns, not as the base |
| Unreal Engine 5 (+Cesium for Unreal, PCG) | LWC, World Partition, PCG, Nanite, sky atmosphere out of the box | Flat Landscape (sphere needs custom or Cesium); massive C++ codebase; slow iteration; hard for agent to verify; licensing terms | Strong if the goal were a game fast; poor fit for "own generator + agent builds it" |
| Godot 4 (double build, GDExtension) | Lightweight, open | Double build perf penalty; planetary terrain is DIY | Middle ground; less control than custom |
| Unity / Stride / O3DE | Mature editors (Unity) | Float coordinates + rebasing; licensing (Unity) | Not recommended |

- **Shaders:** WGSL via naga for portability. Consider Slang → SPIR-V if WGSL limits bite (wgpu accepts SPIR-V passthrough on Vulkan).
- **Editor UI:** egui (immediate mode, Rust-native, testable). Dear ImGui is the C++ equivalent.
- **Profiling:** Tracy (CPU/GPU zones), plus RenderDoc and PIX on Windows.
- **Build:** a Cargo workspace. CI runs Windows (native, WARP) and Linux (lavapipe).

**Development workflow (revised: Windows-native, no WSL):**
- **One environment:** Windows native on Rasierklinge. VS Code with rust-analyzer, and Claude Code in the integrated terminal. The Rust toolchain is `x86_64-pc-windows-msvc`, which needs the Visual Studio Build Tools (C++ workload). WSL is not installed or used for this project.
- **Why not WSL:** the GPU is reachable from WSL2 only through D3D12 passthrough, and Vulkan goes through Mesa's non-conformant dzn layer (Vulkan 1.2, "testing use only"). That is a poor target for tuning frame time and VRAM. A second Linux checkout also means two toolchains and a sync step. Linux coverage is available from CI without any local setup. Running everything on Windows lets Claude Code run the real-GPU performance harness and read screenshots and logs directly.
- **Repo location:** a local NTFS drive, not a OneDrive-synced folder. A Windows Dev Drive (ReFS) and a Defender exclusion for `target/` speed up Cargo noticeably. Add a `.gitattributes` that forces LF line endings, and enable long paths.
- **Cross-platform tooling:** put build, test and golden-image commands in `cargo xtask` (or `just`), not in bash or PowerShell scripts. Local Windows, Linux CI and Claude's cloud sandbox then run identical commands.
- **When WSL would return:** only for CUDA-based offline ML experiments (such as terrain super-resolution) or a Linux-only tool. Neither is on the roadmap. If it does return, keep that repo clone on the Linux ext4 filesystem and never build across `/mnt/c` (cross-boundary 9P access is an order of magnitude slower for many-small-file workloads like `cargo` and `git`).

**Verification (three layers, plus the agent's own sandbox):**
1. **Local on Rasierklinge (real GPU):** the performance harness records frame-time percentiles, VRAM and streaming latency along standard flight paths into a committed JSON log that Claude reads. Real-GPU screenshots from scripted camera paths are checked against metrics and, where stable, golden images.
2. **CI Windows job:** native build, tests and headless wgpu rendering on WARP.
3. **CI Linux job:** build, clippy, CPU-only tests and headless rendering on lavapipe, which mirrors how wgpu's own CI runs GPU tests.
4. **Claude's cloud sandbox (no GPU):** can run the CPU-only parts (generators, precision tests, seam property tests) and Linux-side software-rendered golden images.

What the CPU/software layers check:
- CPU reference implementations for every generator stage (bit-exact tests, property tests: seam continuity across tile/face borders, determinism across thread counts).
- Numeric precision tests (round-trip geo ↔ planet-fixed ↔ camera-relative; jitter simulation).
- Headless renders to PNG from scripted camera paths, compared to goldens with perceptual tolerances (FLIP/SSIM), plus image-metric checks (no NaN pixels, sky luminance ranges, horizon position).
- wgpu/validation layers always on in debug builds.

How these layers map to verification tiers, and the mechanics that keep the loops fast, are specified in Section 16.

### 10. Application architecture

```
┌────────────────────────────── app (single binary) ──────────────────────────────┐
│  modes:  editor/viewer  |  generate (headless CLI)  |  test-render (headless)    │
└───────────────┬──────────────────────┬─────────────────────────┬─────────────────┘
          editor (egui, commands, undo, gizmos, picking)         tools/cli
                │                      │
          render (render graph, GPU-driven terrain/instances, atmosphere, ocean)
                │        executes a FramePlan; makes no decisions
          frame (GPU-free: node selection, LOD/morph, batches, params → FramePlan)
                │                      │
          streaming (priority scheduler, cancellation, upload budget, residency)
                │
          cache (content-addressed tiles: RAM LRU, disk pack, [remote later])
                │
          generators (fields → tiles; global-coarse, regional, local; cities, scatter)
                │
          world-def (WorldDefinition DAG, layers, authored items, versioning, hashing)
                │
          core (math, f64 frames, geo, cube-sphere addressing, RNG/hash, jobs, IO)
```

Rules:
- Dependencies point only downward. `generators` must not depend on `render`; GPU generation is an implementation behind a `Generator` trait with a CPU reference.
- `world-def` and `generators` compile without any graphics crate. That is what makes the headless CLI, the cloud tile server and agent tests possible.
- ECS is useful for *placed objects and simulation* (consider bevy_ecs or hecs as a library). Terrain tiles, though, are a "world as database": quadtree + cache, not entities.
- **Frame-plan boundary (Revision 3):** a GPU-free `frame` layer (between `streaming` and `render`) turns camera, world state and settings into a serializable `FramePlan`: visible terrain nodes with LOD and morph factors, resident tile handles, material/biome weights, instance batches, atmosphere and exposure parameters, debug-view selection. `render` consumes a `FramePlan` and makes no decisions of its own. A `FramePlan` can be snapshot-tested, diffed between versions and saved into a repro bundle.
- **Time and I/O are injected.** Streaming and scheduling code takes a `Clock`, an `Io` and a `Spawner` interface. Tests use a fake clock, fake I/O with injectable latency and failures, and a deterministic single-threaded spawner, so streaming behaviour is reproducible without a GPU.

**One app vs many:** one *binary* is right for users and dogfooding. Bloat is controlled by crate boundaries and Cargo features (`--no-default-features` builds a GPU-free `planetgen` CLI from the same crates). Splitting into multiple apps adds IPC and versioning costs without benefit until a cloud tile server exists, and that server is just another thin binary over `generators + cache`.

### 11. Critique and extension of "Exploration: large scale 3d applications"

**Keep:**
- WorldDefinition vs disposable runtime representation.
- Additive height fields.
- Fields consumed by content generators.
- Splines as shared data.
- Deterministic (seed, cell) generation with caching tiers.
- Async producer/consumer streaming.
- Double-precision global coordinates with local origins.
- Hierarchical coordinate frames.

**Change:**
- *"H = continent + mountain + … + erosion" as a pure sum is insufficient.* Erosion and rivers are non-local and multiplicative or conditional. Restructure as a staged pipeline (global-coarse → regional with halo → local) where later stages read earlier fields.
- *Seed alone isn't enough for determinism.* Key tiles by `(definition hash of the read set, generator version, tile id)`.
- *The learning path should put precision and the test harness first*, not "infinite terrain" first. Retrofitting precision is the most documented failure (Star Citizen's 8-month conversion; UE's tile-based LWC rework).
- *UE World Partition/HLOD* is a good reference for authored-object streaming, but terrain itself should follow Proland/Cesium-style tile trees, not WP grid cells.

**Add (missing):**
- Atmosphere, ocean, clouds.
- HDR/exposure.
- GPU-driven rendering and a render graph.
- Virtual texturing.
- CPU/GPU determinism policy.
- Editor command/undo model.
- Data formats and versioning/migration policy.
- Cube-face mapping choice.
- Depth strategy.
- Test/verification strategy for an AI implementer.
- Performance budgets.
- Observability (LOD/streaming heatmaps, tile-generation timelines in Tracy).
- Spec process.

### 12. Spec structure (evergreen, with volatility tiers)

| Tier | Artifact | Volatility | Content |
|---|---|---|---|
| 0 | `constitution.md` | Very stable | Principles/invariants: layering rules, purity of generators, precision budget, determinism policy, "every feature has a headless test", performance budgets as gates |
| 1 | `adr/NNNN-*.md` | Stable once accepted; superseded, never edited | Up-front decisions with options, decision, consequences |
| 2 | `specs/subsystems/*.md` | Medium | coordinates, world-definition & fields, terrain representation & LOD, streaming & cache, generator pipeline, authored layers & editor model, cities, renderer, atmosphere/ocean, build/tooling/test |
| 3 | `specs/milestones/Mx.md` | Per milestone | Scope, acceptance criteria (executable), budgets, linked ADRs |
| 4 | `research/spikes.md` | High | Open questions, spike plans, results, then promotion to ADR |
| 5 | `learnings.md` | Append-only | Implementation findings; each entry links to the spec sections it changes |
| — | `CLAUDE.md` | Medium | Agent operating manual: Windows-native build/test commands (`cargo xtask ...`), where the specs live, verification commands, how to run the local performance harness |

Process: spike → result in the spike register → ADR (accepted/superseded) → subsystem-spec diff → milestone acceptance test. Each spec section carries `status` (draft/accepted/deprecated), an `assumptions` list with an expiry check, and trace IDs that tests reference (`// spec: COORD-003`). A spec change requires updating the tests that cite it.

**Spec tooling (Revision 4): OpenSpec, not Spec Kit.** OpenSpec keeps the current truth per capability in `openspec/specs/` and puts each piece of work in `openspec/changes/<change>/` (proposal, design, tasks, and spec deltas such as "ADDED Requirements"). Archiving a finished change merges its deltas into the current specs. That is exactly the "evergreen specs updated as implementation proceeds" model this project wants. Spec Kit is organised around one spec folder per feature with stronger phase gates, which suits greenfield feature delivery but leaves the "current state" spread across features. Mapping:
- Tier 2 subsystem specs → `openspec/specs/<subsystem>/spec.md`.
- Tier 3 milestone work → one or more OpenSpec changes per milestone; milestone acceptance criteria live in the change and become requirements on archive.
- Tiers 0, 1, 4, 5 (constitution, ADRs, spikes, learnings) are plain Markdown in `docs/` and referenced from `CLAUDE.md`; neither tool manages these.
- Trace IDs go into requirement headings (for example `### Requirement: COORD-003 Geo round-trip`), and each requirement states its verification tier.
The full rationale is in the starter doc ("Starter: tooling and project setup").

**Traceability and verification tier (Revision 3):**
- Every requirement declares its verification tier: `verify: A | B | C | D` (Section 16). Writing the tier forces each requirement to be testable, and requirements that can only be verified at D are visibly the gap.
- `cargo xtask spec-lint` checks that every requirement ID is cited by at least one test, every cited ID exists, and every tier-A/B/C requirement has an automated test. Tier-D requirements must name their review artifact (a contact sheet or approved golden).
- Keep specs small: a typical task should need at most two or three spec files plus `CLAUDE.md`. That keeps the agent's context focused and makes spec drift visible.
- `subsystems/testing.md` is a new subsystem spec holding the tier model, the frame-plan contract, debug views, repro bundles and loop time budgets.

**ADR candidates:**

| ADR | Decide | Options | Leaning |
|---|---|---|---|
| Coordinate frames & precision | **Now** | f64 planet-fixed; i64 cell + f32; f32 + rebasing | f64 planet-fixed + camera-relative f32; cell scheme reserved for multi-system |
| Depth | **Now** | Reversed-Z D32F; log depth; multi-frustum | Reversed-Z infinite far |
| Planet topology & tile addressing | **Now** | Cube-sphere + mapping variant; HEALPix; icosa | Cube-sphere, approx-equal-area warp, 64-bit tile ids (face, level, Morton/Hilbert) |
| Language & graphics layer | **Now** | Rust+wgpu; Rust+ash; C++ | Rust+wgpu with a thin render-HAL seam |
| Development environment & workflow | **Now** (decided 2026-10-06) | Windows-native; Windows + WSL; WSL-primary | Windows-native (VS Code + Claude Code), no WSL; Linux via CI; `cargo xtask` for cross-platform commands |
| Generator purity, versioning, cache keys | **Now** | Seeded only; content-addressed DAG | Content-addressed DAG keys + generator_version pinning |
| Authored-layer data model | **Now** | Absolute rasters; relative; intent/vector | Geo-anchored intent + relative deltas + freeze/bake; cross-version survival is nice-to-have |
| App structure | **Now** | One binary + modes; multiple apps | One binary, GPU-free core crates |
| Testing architecture & render boundary (Revision 3) | **Now** | Test mostly through rendered images; thin executor with GPU-free frame plan | GPU-free `FramePlan` + four verification tiers; CPU terrain node selection; injected clock/I/O |
| Cross-platform numeric determinism (Revision 3) | **Now** | Platform libm; pure-Rust math; fixed-point | Pure-Rust `libm` for transcendentals in generator code, no fast-math, order-fixed reductions; Windows and Linux CI compare tile hashes |
| Reference surface (Revision 3) | **Now** | Sphere; ellipsoid | Sphere for MVP, but geo conversions written against a `ReferenceSurface` interface so an ellipsoid can be added without changing stored data |
| Spec tooling (Revision 4) | **Now** | GitHub Spec Kit; OpenSpec; plain Markdown | OpenSpec for subsystem specs and changes; constitution, ADRs, spikes and learnings as plain Markdown |
| WorldDefinition schema versioning (Revision 3) | **M3** | Ad hoc; schema version + migrations | Schema version field + tested migration functions, separate from generator versioning |
| Render graph & GPU-driven baseline | **M2** | Custom graph; none | Small custom render graph, indirect draws |
| Terrain LOD algorithm | M2 spike | CDLOD; clipmaps; meshlets | CDLOD now, meshlets later |
| Tile storage format | M3–M4 | SQLite/LMDB + blobs; PMTiles-like single file | LMDB/SQLite index + pack files |
| Erosion strategy | M3 spike | Global stream-power + local filter; local only; ML | Global coarse + local filter |
| Atmosphere model | M5 | Hillaire 2020; Bruneton 2008 | Hillaire 2020 |
| Ocean model | M5 | Gerstner; FFT; Bruneton 2010 | Gerstner→FFT |
| Editor UI toolkit | M6 | egui; ImGui; web UI | egui |
| City street model | M7 spike | Tensor field; L-system; agent | Tensor field |
| Cloud/tile server | M9 | Not now | Defer; keep keys and purity |

### 13. Feature set

- **MUST (MVP vertical slice):**
  - Earth-radius cube-sphere planet with CDLOD.
  - f64 frames and camera-relative rendering; reversed-Z.
  - Procedural height/biome fields (global-coarse + local). For the MVP, global-coarse means continents, elevation and climate only. Global hydrology (stream-power erosion, river networks) is SHOULD, so the vertical slice does not depend on a research-grade piece.
  - Content-addressed tile cache with async streaming and cancellation.
  - Hillaire atmosphere with aerial perspective.
  - HDR + exposure.
  - Seamless space-to-ground flight with an adaptive camera.
  - Headless CLI and test-render modes; golden-image + precision test suites.
  - Perf HUD and LOD/streaming debug overlays.
- **SHOULD:**
  - Authored layers (stamps, splines, geo-anchored objects, masks) with undo and git-friendly storage.
  - Generator version pinning + freeze/bake.
  - Simple ocean with shoreline.
  - Biome materials and texture splatting.
  - Global river network + local erosion filter.
  - Vegetation scatter with instancing.
  - Basic editor (select/place/edit/inspect).
- **COULD:**
  - Cities (placement, roads, parcels, extruded/grammar buildings, city HLOD).
  - Volumetric clouds.
  - Collision and walking (Jolt-style local frames).
  - Real-DEM import.
  - Caves via local volumetric patches.
  - Night side/city lights.
  - FFT ocean.
  - Upscalers.
  - Cross-generator-version migration of authored data (re-fit tooling).
- **LATER / NON-GOALS (v1):**
  - Cloud tile serving and rendering.
  - Multiplayer.
  - Full climate/ecology simulation.
  - Building interiors.
  - Multi-planet N-body systems. The frame hierarchy must remain compatible with them.

### 14. Risk register and spikes

| Risk | Likelihood/impact | Spike to retire it (milestone) |
|---|---|---|
| Precision artifacts/seams at surface and cube edges | Med/High | M1: CPU seam-continuity property tests across faces; M2: jitter test at 1 m view, 6,371 km from origin |
| Non-local erosion vs tile independence | High/High | M3: global 1–4 km stream-power on sphere + halo-based local carving; measure visual seams and gen time |
| LOD popping / morph artifacts on steep terrain | Med/Med | M2: CDLOD morph with screen-space-error metric, A/B captures |
| Fast-descent streaming stalls (orbit to ground in <30 s) | High/High | M4: scripted dive path; target zero frames without some LOD; p99 tile latency ≤ 200 ms |
| VRAM overrun on 8 GB cards | Med/High | M4: residency manager with hard budgets, eviction tests |
| wgpu feature gaps (bindless, sparse, mesh shaders) | Med/Med | M2: prototype GPU-driven indirect terrain in wgpu; fallback HAL seam |
| Editor on a streamed world (undo, picking, invalidation latency) | Med/Med | M6: edit a stamp → regenerated tiles visible ≤ 1 s at nearby LODs |
| City LOD transitions & instance counts | High/Med | M7: 10^5-building city, HLOD transitions, ≤ 4 ms city budget |
| AI agent can't judge visuals | High/Med | M1: golden-image + metric harness on lavapipe/WARP (and real GPU locally), NMS-style probe screenshots per milestone |
| Shader complexity/compile times | Med/Med | M2: shader module structure + hot reload |
| Windows/Linux divergence (path, line-ending, tooling) | Low/Low | M0: `cargo xtask` as the only command surface; CI runs both OSes from day one |
| *Added in Revision 3 (see Section 17):* | | |
| Scope too large to reach a satisfying vertical slice | High/High | Go/no-go gates after M2 and M5; MVP excludes global hydrology and cities; spikes are time-boxed |
| Cross-platform float divergence breaks "bit-exact" tiles | High/Med | M1: pure-Rust `libm`, Windows-vs-Linux tile-hash comparison in CI |
| Mixed CPU/GPU tile sources cause seams | Med/High | M3: one implementation per field layer per cache; implementation id in cache key; neighbour-seam test across sources |
| Stale cache from incomplete cache keys | Med/High | M3: generators read inputs only through a read-recording context; `cache-verify` regenerates from scratch and compares |
| Shader-side planet-scale magnitudes reintroduce jitter | Med/High | M2: lint and review rule that no shader computes values of planet-radius magnitude; precision test at 1 m altitude |
| Coastline flicker when LOD changes near sea level | Med/Med | M5: morph-aware shoreline blending; scripted coastal flight with debug-view golden |
| Performance budgets measured on a non-representative GPU | Low/Med (Revision 4: Rasierklinge's RTX 3080 Ti Laptop is roughly mid-range) | M0: Rasierklinge is the reference machine; VRAM cap at 8 GB for the mid-range tier |
| Laptop power and thermal variance makes timings noisy (Revision 4) | High/Med | M0: fixed performance-harness protocol (charger, power profile, warm-up, medians, logged clocks); thresholds from measured noise |
| App runs on the integrated GPU under Optimus (Revision 4) | Med/Med | M0: request high-performance adapter, log adapter, harness refuses the wrong adapter |
| Software-rendered goldens slow or flaky in CI | Med/Low | M0: small resolutions (256–512 px), per-adapter goldens, debug views with exact comparison |
| Spec bloat and drift | Med/Med | M0: `spec-lint`, small spec files, trace IDs in tests |
| Third-party technique or code licences | Low/Med | Each adoption ADR records the licence (e.g. erosion-filter shader source, Hillaire reference code) |

*Removed in Revision 2:* "WSL GPU dev friction" (no longer applies, since WSL is not in the workflow).

### 15. Milestone roadmap

| Milestone | Scope | Exit criteria | ADRs |
|---|---|---|---|
| **M0 Foundations** | Repo, Cargo workspace, Windows-native toolchain setup (MSVC, VS Code, rust-analyzer), `cargo xtask` (`test fast`, `test gpu`, `spec-lint`, `bless`), `.gitattributes`, CI (Windows WARP + Linux lavapipe), CLAUDE.md, constitution, spec skeleton incl. `subsystems/testing.md`, Tracy; OpenSpec initialised; project agents and skills (see the starter doc); performance-harness protocol for Rasierklinge (power profile, adapter check, noise measurement) | Windows-native build and test green locally; CI green on both Windows and Linux; headless "hello triangle" PNG golden on both; `spec-lint` passes; `test fast` under 10 s | Language/API, app structure, dev environment, testing architecture |
| **M1 Coordinates, harness & walking skeleton** | f64 frames, `ReferenceSurface`, geo conversions, cube-sphere addressing, RNG/hash with pure-Rust math, test-render mode. **Walking skeleton:** one CPU-generated tile → `FramePlan` → texture → screen, with a debug view, end to end. CPU field-to-PNG dumps (height, face net). Repro bundle format | Round-trip ≤ 1 mm; seam property tests; tile hashes identical on Windows and Linux CI; golden harness live with exact-match debug views; skeleton runs in all three run modes | Coordinates, depth, topology, numeric determinism, reference surface |
| **M2 Planet LOD skeleton** | CDLOD on cube sphere with CPU node selection, camera-relative rendering, reversed-Z, render graph, GPU-driven instance path (stub), adaptive camera, **minimal async tile generation** (thread pool + request queue; hardened in M4), debug views (tile ID, LOD, face, normals, depth) | Orbit→1 m altitude without jitter; ≤ 8 ms terrain at 1440p on the mid-range tier; no cracks in 10 scripted views (checked on debug views); node selection snapshot tests. **Go/no-go gate:** review wgpu fit, precision and LOD before M3 | Render graph, LOD |
| **M3 Field pipeline & cache** | WorldDefinition DAG with schema version, fields, global-coarse bake (continents, climate), local noise + erosion filter, content-addressed disk cache with read-recording inputs, `generate` CLI. Global-hydrology **spike** runs in parallel, time-boxed | Bit-exact CPU tiles across runs/threads/OSes; global 1 km bake ≤ 10 min; definition edit invalidates only affected tiles; `cache-verify` passes; metamorphic tests (partition invariance, coarse/fine agreement) pass | Generator purity, erosion, tile storage, schema versioning |
| **M4 Streaming** | Priority scheduler, cancellation, velocity prefetch, upload budget, VRAM residency | Dive test: no blank tiles, p99 frame ≤ 20 ms, VRAM within tier budget, RAM ≤ 8 GB | Streaming/residency |
| **M5 Atmosphere & ocean** | Hillaire LUTs, aerial perspective, HDR/exposure, ocean surface + shoreline | Atmosphere LUTs match the CPU reference within tolerance (tier B); golden images for ground/limb/sunset views; atmosphere ≤ 1 ms; coastal flight without shoreline flicker. **Go/no-go gate:** the MVP vertical slice is complete here; review scope and priorities before M6 | Atmosphere, ocean |
| **M6 Authored layer + editor** | Layers, stamps, splines, geo-anchored objects, command/undo, egui editor, freeze/bake, version pinning | Edit→visible ≤ 1 s; undo/redo 1,000 ops; text files merge cleanly in git | Authored data model, editor UI |
| **M7 Cities** | Suitability → seeds → roads → parcels → buildings; city HLOD; terrain stamps from roads | 10^5-building city at 60 FPS from 2 km altitude; seamless approach | City model |
| **M8 Polish/perf** | Shadows, vegetation, quality tiers, clouds (if time), upscalers | 60 FPS at 1440p on mid-range tier across standard flight paths | — |
| **M9 Optional cloud** | Tile server over `generators + cache`; remote cache tier | Client streams identical tiles (hash-verified) from server | Cloud architecture |

### 16. Testing and iteration design (Revision 3)

**Goal:** most of the system can be tested in isolation, quickly and without a GPU, so integration into the visual parts only has to cover the gap that can't be tested any other way, and that gap stays small.

**Principle: push the gap down.** The renderer is an executor, not a decision-maker. Every decision about *what* to draw lives in GPU-free code and produces a `FramePlan` (Section 10). The untestable surface becomes "does the executor turn a correct plan into correct pixels". That surface is small and changes rarely.

**Four verification tiers.** Every requirement is verified at the cheapest tier that can falsify it, and declares that tier in the spec (Section 12).

| Tier | What it checks | Runs on | Typical subjects | Comparison |
|---|---|---|---|---|
| **A. Pure** | Logic and numbers, no GPU | Anywhere; seconds | Coordinates, tile addressing, cache keys, fields, global-coarse bakes, terrain node selection, `FramePlan` contents, streaming scheduler (fake clock/I/O), authored-layer evaluation, edit/undo, city graphs and parcels | Exact values, snapshots, properties |
| **B. Oracle** | GPU code against a CPU twin | Software adapter (WARP/lavapipe) or real GPU | Noise and erosion shaders, atmosphere LUTs, GPU culling, CDLOD morph, compute generation | Dispatch on test inputs, read back buffers, compare within tolerance |
| **C. Pixels** | Wiring: bindings, formats, pass order, integration | Software adapter in CI; real GPU locally | About 20 canonical views, mostly debug views; a few lit "beauty" views | Exact for debug views; perceptual (FLIP/SSIM) for lit views; per-adapter goldens |
| **D. Human** | Does it look right and good | You | Contact sheets per milestone, flight videos | Approve once with `xtask bless`; approved images become tier-C goldens |

Target: the share of requirements verifiable only at C or D shrinks over time, and `spec-lint` reports it.

**Mechanics that make visual problems checkable**
- **Debug views from M1:** flat-colour renders of tile ID, face, LOD level, morph factor, normals, depth, streaming state (resident, pending, missing). They don't depend on lighting, are deterministic across adapters and can be compared exactly. Cracks, seams, LOD popping and missing tiles show up as objective pixel patterns (for example, background-coloured pixels inside the planet disc in the tile-ID view mean a crack).
- **Image metrics** on top of goldens: no NaN or infinite pixels, horizon position within tolerance, sky luminance in range, count of "hole" pixels equals zero.
- **Data products as images, without a renderer:** height maps, field maps (climate, biome, flow) and the unfolded six-face cube net are written to PNG from CPU code. Claude can look at them, so generator work in M3 is reviewable before any GPU code exists.
- **Repro bundles:** camera pose and path, definition hash, generator version, settings and adapter info in one file. Any visual bug becomes a test case that `xtask repro <bundle>` replays in every run mode.
- **Scripted flight paths** (orbit, dive, coastal pass, mountain valley, city approach) shared by golden tests, the performance harness and manual review.

**Mechanics that keep components independently testable**
- **Contract suites per interface:** one test suite per trait that every implementation must pass. Examples: `Generator` (CPU and GPU implementations), `TileCache` (memory, disk, later remote), `Io` (real, fake), `ReferenceSurface` (sphere, later ellipsoid).
- **Metamorphic tests for generators** (there is no "correct" terrain to compare with):
  - Neighbouring tiles agree along shared borders, including across cube faces and at the 8 face corners.
  - A coarse tile agrees with its four children within a tolerance.
  - Generating a region as one large tile or many small tiles gives the same result (partition invariance).
  - Results are identical across thread counts and across Windows and Linux.
- **Differential tests:** reference versus optimised paths (naive versus accelerated culling, CPU versus GPU generation).
- **Count-based budgets:** draw calls, triangles, resident tiles, bytes uploaded per frame and VRAM allocated are deterministic and can be asserted on any adapter. Only time-based budgets need Rasierklinge.
- **Injected time and I/O:** a fake clock, fake I/O with injectable latency and failures, and a deterministic spawner make the streaming system testable at tier A, including the fast-descent case.
- **Cache verification:** `xtask cache-verify` regenerates a sample of tiles from scratch and compares them with the cached versions. That catches cache keys that miss an input.

**Mechanics that shrink the remaining gap**
- Generate Rust structs from the WGSL bindings (or the reverse) so CPU and shader layouts can't drift.
- Validate every shader with naga at build time, and create every pipeline once in a CI smoke test.
- wgpu validation always on in debug builds and in CI.
- Shader hot reload and live editing of the WorldDefinition for the human loop.

**Loop time budgets** (targets, to be confirmed in M0)

| Command | Contents | Target |
|---|---|---|
| `xtask test fast` | Tier A for the changed crates | < 10 s |
| `xtask test` | All tier A, tier B on the software adapter | < 2 min |
| `xtask test gpu` | Tier C goldens and debug views | < 5 min |
| `xtask accept Mx` | Milestone acceptance incl. perf harness (Rasierklinge only for timings) | < 15 min |
| CI (per push) | Windows + Linux, tiers A–C | < 15 min |

Build-speed measures: small GPU-free crates so most edits rebuild little; `cargo-nextest`; a fast linker; `sccache`; `opt-level` overrides for math and noise crates in the dev profile; small resolutions (256–512 px) for software-rendered goldens.

**Agent workflow rules (for `CLAUDE.md`)**
1. Pick the cheapest tier that can falsify the change, and write the failing test first.
2. Failure output must be machine-readable and actionable: numeric deltas, tile IDs, diff images written to `target/review/`.
3. No flaky tests: anything non-deterministic is fixed or quarantined with a tracking entry, never retried silently.
4. New visual features land with a debug view and at least one tier-C test; lit-view goldens only after human approval.
5. Crate boundaries double as task boundaries, which keeps parallel work (worktrees, subagents) from colliding.

### 17. De-risking review (Revision 3)

A review pass over the whole plan, looking for risks, internal tensions and gaps. Findings are folded into Sections 8, 10, 12, 13, 14 and 15. This review is design judgment, not research output.

**Scope and sequencing**
1. **The project is very large; reaching a satisfying vertical slice is the top risk.** M0–M5 alone is a long stretch. *Change:* go/no-go gates after M2 (does the precision, LOD and wgpu foundation hold?) and after M5 (the MVP slice is complete; re-prioritise). Spikes are time-boxed with a written fallback.
2. **The MVP depended on a research-grade component.** "Global-coarse fields" included stream-power hydrology, which is the least proven part of the plan. *Change:* the MVP's global-coarse stage is continents, elevation and climate only; hydrology becomes SHOULD, and its spike runs in parallel with M3. Fallback: the local erosion filter alone, plus rivers traced on a coarse (4–10 km) flow grid.
3. **M2 needed streaming before M4.** Flying from orbit to 1 m altitude needs deep tiles generated on the fly. *Change:* M2 includes a minimal async generation path (thread pool + request queue); M4 hardens it (priorities, cancellation, prefetch, residency).
4. **Integration was back-loaded.** *Change:* a walking skeleton in M1 (one tile end to end through all three run modes), so the integration path exists from the start.

**Numerics and determinism**
5. **"Bit-exact across machines" was not achievable as written.** Rust's `f64::sin`, `exp` and similar call the platform maths library, which differs between MSVC on Windows and glibc on Linux. *Change:* generator code uses the pure-Rust `libm` crate for transcendentals, avoids fast-math, and uses fixed-order reductions independent of thread count. CI compares tile hashes between Windows and Linux.
6. **Mixing CPU- and GPU-generated tiles can create seams.** Two neighbouring tiles from implementations that agree only within tolerance won't match exactly at the border. *Change:* within one cache, a given field layer is produced by one implementation; the implementation id is part of the cache key; a test checks seams between tiles from different sources.
7. **Planet-scale magnitudes can leak into shaders.** Camera-relative rendering only works if no shader computes values of planet-radius size (for example, projecting a cube point to the sphere and multiplying by the radius in the shader). *Change:* constitution rule and review checklist item: shaders receive tile-local or camera-relative quantities only; a precision test runs at 1 m altitude far from the origin.
8. **Sphere versus ellipsoid was implicit.** *Change:* new decide-now ADR: sphere for the MVP, with geo conversions behind a `ReferenceSurface` interface so stored geo-anchors stay valid if an ellipsoid is added.

**Cache, data and versioning**
9. **Content-addressed keys can silently miss an input** (the classic stale-cache bug). *Change:* generators read inputs only through a context that records what was read, and `cache-verify` regenerates samples from scratch.
10. **Schema changes were mixed up with generator changes.** *Change:* the WorldDefinition gets its own schema version and tested migration functions (fixture files from older versions), independent of `generator_version`.
11. **Tile ID capacity checked:** 3 bits for the face + 5 bits for the level + 2 bits per level allows 28 levels in 64 bits, more than the ~19 levels needed for 10 cm detail. No change needed.

**Rendering**
12. **GPU-driven rendering conflicted with testability.** *Change:* GPU-driven submission is for instances; terrain node selection stays on the CPU and is tested at tier A (Section 8).
13. **Coastlines can flicker as LOD changes near sea level.** *Change:* added to the risk register with morph-aware shoreline blending and a coastal flight test in M5.

**Process and environment**
14. **Performance budgets assume a "mid-range" GPU, but they are measured on one machine.** *Resolved in Revision 4:* Rasierklinge has an RTX 3080 Ti Laptop GPU (16 GB), roughly mid-range in performance, so it becomes the reference machine. Its extra VRAM is handled with an 8 GB cap for the mid-range tier, and laptop-specific noise and adapter selection get their own harness protocol (Section 8).
15. **Software-rendered goldens can be slow or flaky in CI.** *Change:* small resolutions, per-adapter goldens, exact comparison only on debug views.
16. **Specs can bloat and drift.** *Change:* `spec-lint`, small spec files, trace IDs in tests, and a rule that a typical task needs at most two or three spec files.
17. **Licences of adopted techniques and code were not tracked.** *Change:* each adoption ADR records the licence (for example the erosion filter's shader source and any Hillaire reference code).
18. **wgpu API churn.** *Change:* pin the wgpu version and upgrade deliberately at milestone boundaries, with the upgrade recorded in `learnings.md`.

**Still open after this review**
- Whether the global hydrology spike succeeds or the fallback becomes the plan (decided at the end of M3).
- How much of the editor's UI should be node-graph based (Section 1 shows users expect it from Houdini/Gaea) versus layer-list based. Decide at M6.

## Recommendations

1. Write the constitution and the "decide now" ADRs (including the development-environment ADR) before any rendering code. Include the precision budget and determinism policy as testable invariants.
2. Build M0–M1 (Windows-native toolchain, CI with WARP/lavapipe, coordinates, golden-image harness) first, so Claude can verify every later milestone on its own.
3. Develop natively on Windows with VS Code and Claude Code. Do not set up WSL. Cover Linux through CI, and keep all commands behind `cargo xtask`.
4. Adopt the staged field pipeline (global-coarse bake → halo regional → local per-point) and content-addressed tile keys. This is the single decision that keeps both "authored edits can survive" and "cloud later" cheap.
5. Implement authored content as geo-anchored intent + relative deltas, pinned to a generator version with freeze/bake. Record "expected result" snapshots to enable future re-fitting. Defer constraint solving except for rivers and city centers. Treat cross-version survival as a nice-to-have, and accept "old data is not supported after a generator change" where migration is not cheap.
6. Re-evaluate wgpu at the M2 go/no-go gate against the GPU-driven terrain spike. If indirect/bindless limits bite, swap in a raw-Vulkan/D3D12 backend behind the render-HAL seam rather than changing language.
7. Build the testing architecture first (Section 16): the `FramePlan` boundary, the four verification tiers, debug views and repro bundles all start in M0/M1, so later visual work only has to cover the gap that can't be tested any other way.
8. Apply the de-risking changes in Section 17 before writing the M0 specs, in particular the MVP scope cut (no global hydrology in the slice), the cross-platform determinism rules and the go/no-go gates.

## Caveats

- **Time-sensitive tooling state:** wgpu versions (v28 mesh shaders; experimental ray queries; v30 present by Oct 2026), Spec Kit command names and upscaler availability all change quickly. Re-verify at M0 and M2. WSL GPU status matters only if WSL is reintroduced.
- **FP64 ratios:** RTX 40 (1:64) is confirmed by NVIDIA's CUDA guide and TechPowerUp. RTX 50 is unconfirmed. RDNA 4 sources conflict (1:64 vs 1:32). This doesn't change the design: avoid GPU FP64 in hot paths.
- **Inferred internals:** Outerra, Space Engine, MSFS and Star Citizen internals come from developer blogs, newsletters and press, not formal papers. Treat them as directional.
- **Size, budget and performance numbers** in this report are back-of-envelope engineering estimates, not measurements. The M2–M4 spikes exist to replace them with data from Rasierklinge.
- **Software-adapter testing:** wgpu's own CI uses WARP/lavapipe/llvmpipe, but no official statement was found that these adapters are a supported target for downstream projects. Expect occasional tolerance tuning in golden images. Software renders can also differ slightly from real-GPU renders, so keep separate goldens per adapter.
- **Windows-native workflow assumptions** (MSVC toolchain, Dev Drive and Defender exclusion benefits, Claude Code running locally on Windows) come from general tooling knowledge, not from the research run. Confirm them during M0.
- **Sections 16 and 17 (Revision 3)** are design recommendations, not research findings. The loop time budgets are targets to confirm in M0. The point about platform maths libraries differing between Windows and Linux is general Rust/IEEE knowledge; confirm it with the first cross-OS hash test in M1.
- The report was produced from web sources by an automated research run and is reference material, not a verified specification.

## Sources

1. https://dev.to/nomurasan/why-wsl2-is-slow-on-mntc-and-how-to-find-the-exact-operation-costing-you-time-40o7
2. https://www.ceos3c.com/linux/wsl2-performance-optimization-speed-up-your-linux-experience/
3. https://portal.productboard.com/epicgames/1-unreal-engine-public-roadmap/c/1253-large-world-coordinates-on-gpu
4. https://dev.epicgames.com/documentation/en-us/unreal-engine/large-world-coordinates-rendering-in-unreal-engine-5
5. https://starcitizen.tools/Star_Engine
6. https://news.ycombinator.com/item?id=26938812
7. https://docs.rs/big_space
8. https://docs.nvidia.com/cuda/archive/12.9.1/cuda-c-programming-guide/index.html
9. https://graphicscardsdatabase.com/gpu/nvidia-geforce-rtx-4090
10. https://community.amd.com/t5/pc-graphics/what-are-the-fp64-fp32-ratios-what-is-the-double-precision-power/td-p/574735
11. https://graphicscardsdatabase.com/gpu/amd-radeon-rx-7900-xtx
12. https://en.wikipedia.org/wiki/RDNA_4
13. https://geeks3d.com/20250305/amd-radeon-rx-9070-xt-and-rx-9070-launched/
14. https://news.ycombinator.com/item?id=15920871
15. https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/12440767
16. https://outerra.blogspot.com/2012/11/maximizing-depth-buffer-range-and.html
17. https://outerra.blogspot.com/2013/07/logarithmic-depth-buffer-optimizations.html
18. https://www.semanticscholar.org/paper/Continuous-Distance-Dependent-Level-of-Detail-for-Strugar/6a75892f45b72f8765379134e8d2a4ed6a04f1b0
19. https://elitedangerous2016.wordpress.com/category/stellar-forge/
20. https://onlinelibrary.wiley.com/doi/abs/10.1111/cgf.14050
21. https://x.com/SebHillaire/status/1274799858642235395
22. https://github.com/taytay-industries/fork-godogen/pull/2
23. https://github.com/ggml-org/llama.cpp/discussions/26729
24. https://forums.developer.nvidia.com/t/wsl2-ubuntu-uses-llvmpipe-instead-of-nvidia-gpu-3090/319022
25. https://github.com/steelbrain/reims-vgpu/issues/32
26. https://github.com/SecondHalfGames/wgpu
27. https://en.wikipedia.org/wiki/Development_of_No_Man's_Sky
28. https://github.com/github/spec-kit
29. https://devops.com/githubs-spec-kit-puts-the-spec-back-in-software-development/
30. https://casopisi.junis.ni.ac.rs/index.php/FUMathInf/article/viewFile/871/pdf_75
31. https://procedural-generation.isaackarth.com/2015/11/23/elite-dangerous-2014-planet-generation.html
32. https://procedural-generation.tumblr.com/post/133819109664/elite-dangerous-2014-planet-generation
33. https://www.windowscentral.com/microsoft-flight-simulator-2020-next-generation
34. https://cesium.com/platform/cesium-ion/content/cesium-world-terrain/
35. https://github.com/CesiumGS/3d-tiles/blob/main/specification/README.adoc
36. https://github.com/CesiumGS/3d-tiles/tree/1.0/specification
37. https://shapes.inc/fandom/microsoft-flight-simulator/world-and-scenery
38. https://www.jcgt.org/published/0007/02/01/paper-lowres.pdf
39. https://github.com/shimakaze09/Engine/issues/1130
40. https://80.lv/articles/fast-terrain-erosion-filter-that-emulates-erosion-without-simulation
41. https://dl.acm.org/doi/10.1145/3592787
42. https://learn.microsoft.com/en-us/windows/wsl/filesystems
43. https://robertsspaceindustries.com/en/comm-link/transmission/14839-Letter-From-The-Chairman
44. https://www.shiplight.ai/blog/spec-driven-development-with-spec-kit
45. https://github.com/gfx-rs/wgpu/releases/tag/v28.0.0
46. https://github.com/gfx-rs/wgpu/blob/trunk/docs/api-specs/ray_tracing.md
47. https://github.com/stevepryde/sgl/issues/221
48. https://www.techpowerup.com/gpu-specs/asus-tuf-rtx-4090-gaming-og.b11127
49. https://www.notebookcheck.net/NVIDIA-GeForce-RTX-3080-Ti-Laptop-GPU-GPU-Benchmarks-and-Specs.588451.0.html
50. https://github.com/Fission-AI/OpenSpec
51. https://github.com/Fission-AI/OpenSpec/blob/main/docs/supported-tools.md
52. https://github.com/github/spec-kit/blob/main/docs/installation.md
