# SPIKE-105: architecture re-plan of terrain and hydrology (phase 1)

Opened: 2026-11-08 (planner inbox #96; owner go for phase 1 only). Run on Opus.
Question: how far are today's ADRs, constitution and M9 bake from the owner's firm requirements of 2026-11-07, and which of three routes closes the gap with the least patchwork: (A) evolve step by step, (B) fork or re-implement the core, (C) cut requirements?
Why it matters: the erosion and basin chain was amended five times without a whole-chain design (SPIKE-104), and an external review recommends a hydrology-first, editable, hierarchical architecture. The planet bake (M9.2) would freeze the current design.
Time box: phase 1 is one night, docs only (no sub-agents, no probes, no builds). Phase 2 (costs, risks, the vertical prototype, recommendation) starts on the planner's next entry.
Inputs: the external review (owner refs, 2026-11-07, outside git; called "the review" below); SPIKE-104 (`spikes/terrain-pipeline-review/results/results.md`); the owner's requirements of 2026-11-07 (inbox #96); the constitution v1.6; the ADRs named per row.
Branch: `spike/architecture-replan` (local commits per step).

Terms of the review and ours (used in every section):
| Review | Ours |
|---|---|
| base terrain, base surface | the base field: class source `e0` plus the hill and mountain bands (GEN-096, GEN-098); stored as the field bake (`global16384`, ADR 0026) |
| coarse erosion (814 m) | pass 1 of the erosion bake (ADR 0088 point 4.1, ADR 0099) |
| fine erosion (102 m) | pass 2 (ADR 0088 point 4.2, GEN-101) |
| erosion displacement | the erosion layer's change (kind 13, STRM-065) |
| fill depth | the fill-depth channel (ADR 0101, GEN-120) |
| runtime reconstruction | the runtime field: lattice read C1 + layer read + creases (GEN-099, GEN-112) |
| planetary state, PlanetDefinition | the WorldDefinition (CON-05) with ADR 0108's Earth preset |
| TerrainEdit, editor commands | authored items (ADR 0007, ADR 0066): stamps, splines, regions, brush strokes, class paint |
| dirty region | an item's footprint and reach (ADR 0066 point 1-2), stage keys (ADR 0006) |
| rendering hierarchy | the terrain LOD quadtree in `frame` (ADR 0014, ADR 0016) |
| world generation identity | stage and cache keys (ADR 0006, ADR 0018), field versions, implementation ids |

## 1. Requirements

Priority: P0 = MVP must; C = consideration (design for it, not built in the MVP); post = post-MVP, the architecture must not block it.

| # | Requirement (owner, 2026-11-07) | Priority | What the review assumed | What today's ADRs and constitution assume |
|---|---|---|---|---|
| R1a | Edits update erosion and hydrology; the author edits in the editor before shipping | P0 | the same, stronger: "sculpt and see erosion evolve interactively"; dependency graph, dirty regions, async regional re-simulation, versioned transactions | the opposite by default: authored items are stages **after** the base and the erosion layer (ADR 0007, ADR 0066 point 4); edits under the erosion were deferred to M9's erosion design (ADR 0086 point 3) and M9 did not take them up; global authoring and hydrology are out of scope of the storage ADR (ADR 0045 point 8). An edit moves heights; rivers, basins and erosion do not respond |
| R1b | A sandbox game with limited player edits (may forbid "too global" edits) | C | a game with editable terrain and gameplay-driven erosion, dams, floods | no game layer yet (roadmap: after M9); same item model would serve |
| R2 | Every edit kind has a quick preview; global edits may fall back to a low-res preview, then an on-demand re-bake of a larger region with a longer delay; regional edits include climate, lithology and other properties | P0 | three quality levels: fast preview (frames), medium regional recomputation (async), high simulation (deliberate) | heights: edits are visible within the edit-latency budget for drawn tiles (ADR 0072), a stage over the base. World build: every stage has a preview mode, the full build waits for the owner's approval (ADR 0097 point 5). No preview or re-bake path for an edit of drivers (climate, lithology, uplift) after a store exists: lithology and uplift are analytic fields of the seed (ADR 0102, ADR 0103), climate is a field-bake output (ADR 0104); class paint is the only driver edit (ADR 0089 point 8) and it re-keys the erosion layer where weights change (ADR 0045 point 9) with no partial re-bake path |
| R3 | The shipped product is realtime without stutter on Rasierklinge | P0 | runtime performance "high", GPU-oriented runtime; no machine named | CON-21 (60 FPS median, p99 <= 20 ms at 1440p, RAM <= 8 GB, uploads <= 2 ms), CON-22 (streaming never blocks a frame); budgets BUDG-001 to 003 |
| R4a | Hydrology is a first-class output: rivers and basins | P0 | the central investment: an explicit hierarchical drainage graph, river segments as vector objects, basins with spill level and area curve | rivers are implicit: the drainage area of pass 1's last route stored as a byte (GEN-110); basins as the fill-depth channel (ADR 0101); the sea is the only water body (ADR 0041; lake ids reserved); the global river network of ADR 0077 left with the chunk stage (ADR 0086) |
| R4b | Rivers change after an edit | P0 | yes, via dependency propagation (raising a mountain can redirect a river far downstream) | no: rivers are a by-product of the offline erosion bake; an edit does not re-route |
| R4c | Seasons and floods | post | a later capability of the same system (dynamic discharge, seasonal lakes, flooding) | two-season precipitation exists as a climate input (ADR 0104); no time-varying water |
| R4d | Authoring rivers and lakes directly | post (architecture must allow it) | `CreateLakeBasin`, "change a river course" as edit commands | no river or lake item type; the water field accepts a test-only lake producer (ADR 0041 point 7) |
| R5a | Planet sizes from moon to larger than Earth, in principle | P0 for the engine (scales vary); MVP ships one planet | planets larger than Earth and moons with different geology; no fixed metre resolutions; planet-aware areas and distances; gravity and physics as preset parameters | bodies are addressable (ADR 0001, ADR 0003) and `res` is a per-body parameter from a target cell size (ADR 0045 point 2, proposed); but the M9 bake is fixed at 12,288 / 98,304 cells a face edge (BakeShape::production), several constants assume Earth (SPIKE-103: four copies of the radius, climate literals), and ADR 0108 makes Earth a default preset, not a fixed model |
| R5b | A simpler generator may serve moons | C | "substantially different geological characteristics" handled by presets | crater stamps exist (`stamp.rs`); no moon generator |
| R6 | Realism: "plausible and consistent" wins over "close to Earth measurements"; measured comparison stays welcome | P0 (as a rule for decisions) | the same: "geomorphological plausibility, not strict physical simulation"; mutual consistency of geometry, drainage, geology and edits | the scorecard (TEST-081 to TEST-094) judges against GLO-30 numbers; ADR 0091 limits heightfield targets to 45 degrees; the owner already preferred percentile rules over hard limits (memory, 2026-11-02) |
| R7 | Store about 40 GB at most for an Earth-like planet, lower is better; larger planets may use simpler features | P0 | no storage figure; hierarchical, sparse storage; "persistent but regenerable caches" | field bake 11.7 GB at 16384 (ADR 0045 point 6) + erosion layer with channels about 24 GB (owner, 2026-11-04; ADR 0088 status) = about 36 GB before any hydrology graph, lakes or edits; bakes are caches under a disk budget (ADR 0045 point 1, proposed) |
| R8 | The M9 bake may be delayed; the work must then run with little owner attention | P0 (process) | "resist rewriting immediately"; a vertical prototype of one region first | lanes and gates (M9.1 gate, owner approvals of previews and goldens: ADR 0017, ADR 0097 point 5) need the owner at each preview round |
| - | Determinism of authoritative state across platforms and threads | (not restated by the owner; implied by CON-14) | NFR-01: CPU-only authoritative generation; D1 bitwise / D2 quantized / D3 semantic levels; edit-replay reproducibility | CON-14, ADR 0010: bit-exact CPU output across runs, thread counts, Windows and Linux; `libm`; integer lattices; CPU canonical, GPU only within bounds or as presentation (CON-07, ADR 0035) |

Readings:
- R1a and R4b together make hydrology and erosion **functions of the edited world**, not of the seed alone. Today they are functions of the seed (the bake), and edits sit on top.
- R2 asks for a latency ladder per edit kind (preview now, fuller result later), which ADR 0072 has for heights only.
- R5a asks the engine to take the planet's scale as a parameter everywhere; today the bake shape and several kernels take it as a constant.
- R7 is met today only because hydrology and edits store nothing: about 36 GB of 40 GB is used by two caches.
- The review's three ticked answers (hydrological network as an output, larger planets and moons, interactive erosion while sculpting) are stronger than R1-R5 in one place: the owner asks for a quick preview, then a re-bake with a delay, not erosion evolving live under the brush (R2). That makes the review's "fast" level a preview of heights and approximate drainage, and its "medium" and "high" levels the owner's on-demand re-bake.

## 2. The review's recommendations against our code and ADRs

Status: **there** (built or decided as recommended), **partly** (a piece exists, or exists at one scale only), **missing**, **n/a** (does not apply to our requirements or contradicts a firm one). Section numbers: "P" = the first part (pipeline review), "R" = the re-review with the architecture, "S" = the non-binding reference specification.

### 2.1 Surface identity and reconstruction

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| One authoritative base surface; runtime reconstructs exactly what erosion used | P2.3, P6, P11 M1, S3.4 | **missing** | pass 2 takes the change against the base every 4 cells (`bake/tile.rs:203`), the runtime adds it to the store's 611 m lattice read C1 (GEN-099, GEN-112): rms 12 m, max 209 m apart (SPIKE-104 §3) | SPIKE-104 D-B is the minimal fix |
| Base-surface identity in every baked layer; reject incompatible combinations | P6, S3.3 | **partly** | layer keys fold the bake key, channel versions and kernel parameters (ADR 0088 point 7, GEN-114); a record under another key is refused (`load_tile`, STRM-065). The base the change was taken against (stride 4) is not named as such | the mismatch above passed every key check because both bases come from one bake key |
| Store displacement, not absolute elevation; split regional, local, hydrological | P6.1 | **partly** | the change is stored per pass-2 tile (kind 13); pass 1's change is folded in, not kept as its own layer | regional/local split exists only inside the bake |
| `TerrainSampler`: one interface returning elevation, slope, normal, material, geology, erosion, hydrology; elevation independent of camera LOD | S3.4, R8 | **partly** | `HeightField` is the one CPU interface for heights (`height.rs`); batch reads equal single reads (GEN-020); LOD changes only sampling, not identity (CON-08 coarse/fine agreement). Material, geology and hydrology are separate reads (driver block GEN-119, channel block GEN-126) | |
| Simulation resolution independent of rendering resolution | R3.1, R8 | **there** | bake grids (814 m, 102 m) and the render quadtree are independent (ADR 0088, ADR 0014) | the problem is three bake samplings, not coupling to render LOD |
| Do not use rendered tiles as simulation domains | R11.3 | **there** | pass-2 tiles are a fixed grid (3,072 cells + halo, GEN-101), not render tiles | |
| Cross-face continuity in the data, not only a render cross-fade | P5.2, S3.2 | **partly** | pass 1 carves each face with a halo, no routing across cube edges; the change is cross-faded within the halo (GEN-100); the spill level is planet-wide across tile and cube edges (`bake/spill.rs`, GEN-121); one canonical coordinate path (`planet_core::cube`, ADR 0003) | a river crossing a cube edge is two routes joined by a fade, not one river |
| Physical areas and distances, not grid indices | R3.2 | **partly** | routing uses one `dx` per grid (`bake/mod.rs:39`), so the MFD area is cells x dx^2 (`route.rs:125`); the tangent warp varies cell area 1.41x across a face (ADR 0003) | under 1.5x error, uniform per face; not planet-aware per cell |

### 2.2 Hydrology

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| Hydrology as an explicit, persistent, hierarchical drainage graph | P2.2, P9, R4, S4 | **missing** | drainage area of pass 1's last route as a byte per cell (GEN-110); receivers are not stored; ADR 0077's global river network left with the chunk stage (ADR 0086) | the review's central investment |
| River network as vector segments (centreline, width, depth, discharge, profile) | R4.3, S4.4 | **missing** | none; rivers are valleys in the change, drawn as terrain | the coupled run of ADR 0077 had a coarse network; removed |
| Basins with floor, spill elevation, outlet, area curve, type, water level; separate from fill | P7, R4.1, S4.2 | **partly** | the fill depth is a channel (ADR 0101), the spill level is planet-wide (GEN-121); no basin objects, ids, types or area curves; the runtime adds the fill back (GEN-112) | the review: do not conflate routing fill with water |
| Priority-Flood for depressions | P10, R4.2A | **there** | Barnes improved priority flood (`bake/flood.rs`); parallel minimax version for the spill level (`bake/spill.rs`) | |
| Classify depressions (artefact, endorheic, lake, intentional) before breaching or keeping | P3.2, P4.2, R4.2A | **partly** | breach every depression up to a fixed limit (500 m, GEN-100), keep the rest (ADR 0101); arid basins as sinks (ADR 0105 point 2, accepted, not built) | SPIKE-104 adds a breach at 102 m; still not class-aware |
| D-infinity or MFD routing; different models for channels and hillslopes | P10, R4.2B | **partly** | MFD (Freeman 1.1) for area, jittered D8 receivers for the carve (ADR 0099 point 2) | a two-model split already, but not channel against hillslope by location |
| Channel initiation by a climate- and geology-dependent threshold | R4.2C | **missing** | none (no channel extraction) | runoff weights exist (GEN-129) |
| Hierarchical watersheds (coarse regions, refined locally, parent connectivity) | R4.1, S4.3 | **missing** | two passes are a two-level hierarchy of grids, not of watersheds | |
| Lakes as water bodies | P7, R4.1 | **missing** | the water field has one producer, the sea (ADR 0041); lake ids reserved; F8 pit tolerance proposed (SPIKE-102) | |
| Area-weighted accumulation with conservation tests | S4.2, S10.2 | **partly** | MFD area conserves the grid's area at outlets (GEN-100 scenario "The area is shared") | not per physical cell area |

### 2.3 Geology and base relief

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| Plates -> orogenic structure -> mountain geometry (orientation, envelope, chain type) | P3.3 | **partly** | plate mask with convergence and belts (GEN-094, GEN-095), belt core and flank as relief terms (GEN-096), uplift by province (ADR 0103); the ridged band is isotropic noise (GEN-098) | belts have extent and position, not ridge orientation |
| Different noise per phenomenon; structure fields, not "more ridged noise" | P3.1 | **missing** | one trilinear value noise for bands, creases and detail (`height.rs:51`; SPIKE-104 §1) | |
| No axis-aligned lattice artefacts | P3.1, P8.1, P11 M4 | **partly** | the shader's ground detail turns its lattices (REND-057); the generator does not | SPIKE-104 D-C |
| Base terrain that drains by construction or explains its depressions | P3.2 | **missing** | the base holds 30.6 % of mountain land in closed basins (SPIKE-104 §3) | |
| Creases conditioned by lithology and structure | P8.1 | **missing** | creases are noise octaves 7-11 (GEN-109) | lithology is analytic and could drive them (ADR 0102) |
| Ground detail aware of material, slope, distance | P8.2 | **there** | detail weights by material, slope and footprint fade (`render/src/terrain.rs:470`, REND-055, REND-057) | |

### 2.4 Geomorphology

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| Stream-power regional model (FastScape-style implicit) | P10, R5.1 | **there** | implicit stream power with uplift after Braun and Willett (ADR 0088 point 4, `bake/carve.rs`); K from lithology (ADR 0102), U field (ADR 0103), runoff-weighted area (ADR 0104) | 20 steps, transient (ADR 0103 amendment) |
| Sediment transport and deposition, SPACE-like coupling, sediment conservation tests | P10, R5.1, S5.2 | **partly** | pass-2 deposition sweep with mass balance `eroded = deposited + exported` (GEN-101); pass-1 deposition (Yuan et al.) drafted, not merged (#1223, ADR 0105 point 1) | |
| Hillslope diffusion separate from thermal | P4.4, R7 | **partly** | thermal at a critical angle (GEN-101); no diffusion | |
| Glacial erosion as its own operator with characteristic forms | P4.4, R5.3 | **partly** | ice-history field (GEN-134); minimal glacial erosion behind a gate (ADR 0106, open) | |
| Local refinement driven by regional boundary conditions (upstream discharge, downstream level, sediment input) | R5.2, S5.3 | **partly** | pass 2 reads pass 1's change (C1) as its start and nothing else; no discharge or outlet level passed down | SPIKE-104: the hand-over loses connectivity |
| Shared boundary conditions, overlap, or a regional network for adjacent tiles | P5.3 | **partly** | pass-2 tiles overlap by a 128-cell halo; the spill level is solved across tiles (GEN-121); no shared network | |
| Do not raise erosion resolution globally | R11.1 | **there** | pass 1 at 12,288 (ADR 0099), pass 2 at 102 m, nothing finer | |
| Simulation time distinct from editor time (`SimulationRequest` with duration and quality) | S5.4 | **missing** | carve steps and `K dt` are bake parameters (GEN-100) | |

### 2.5 Editing, versioning, persistence

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| Command-based edits with stable ids, deterministic semantics, undo/redo | S6.1, S6.4 | **there** | authored items with ULIDs, canonical TOML, application order `(priority, id)` (ADR 0007, ADR 0066); session undo; the command log is not persisted (ADR 0066) | |
| Edits as commands on drivers (lithology, uplift, rainfall, lake basin) | R6, S6.1 | **partly** | class paint (ADR 0089 point 8) and driver edits by region (ADR 0076 point 7, proposed); no lithology, uplift, rainfall or lake items | |
| Dependency graph with reverse dependencies; dirty regions per kind (geometric, hydrological, sediment, render) | R6.1, R6.2, S6.2 | **partly** | stage keys chain the read sets (ADR 0006, ADR 0066 point 4); footprint queries and Merkle region summaries (ADR 0066 points 2-3) find re-keyed tiles. No hydrological reach: an item's reach is a radius (ADR 0066 point 1), so an edit cannot invalidate downstream rivers | the key system is the right base; reach is geometric only |
| Three quality levels: fast preview, regional recomputation, high simulation | R6.3 | **partly** | heights: drawn tiles exact within the edit-latency budget (ADR 0072); world build: preview first (ADR 0097 point 5); nothing between | |
| Versioned, transactional updates; stale background results never overwrite newer state | R6.4, S6.3 | **there** for tiles | stale refreshes pre-empted, cancelled or dropped by key (ADR 0072 point 3); consumers key by artefact key (ADR 0006) | no transaction across several derived products (heights, rivers, materials) yet |
| World definition separate from generated state; derived data regenerable | S8.1, S8.2 | **there** | CON-05: the definition is the truth, every runtime representation is a disposable cache; bakes are caches under a disk budget (ADR 0045 point 1, proposed) | an edited, re-simulated region makes "state" more than a cache (§3) |
| Content-addressed cache keys over canonical inputs | S8.3 | **there** | 128-bit BLAKE3 keys over canonical encodings (ADR 0006, ADR 0018, ADR 0023) | |
| Generation identity with algorithm and numerical-profile versions | P6, S1.5 | **there** | generator, field, kernel and channel versions plus implementation ids in every key (ADR 0006); a `libm` upgrade is an output change (ADR 0010) | |

### 2.6 Determinism and platform

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| CPU-only authoritative generation; GPU presentation only | S1.2, R end | **there** | CON-07, CON-14, ADR 0035: CPU canonical; a GPU field must match within bounds; presentation data has no CPU twin | the review asks to keep this |
| No fast-math, deterministic reductions, fixed order | S1.3 | **there** | CON-14, ADR 0010 (clippy-enforced `libm`, fixed-order chunking) | |
| Counter-based PRNG from stable ids | S1.3D | **there** | integer hashing of seed and ids (`core::hash::hash_u64s`, SplitMix64 chain; ADR 0006) | not Philox, same property |
| Integer or fixed-point for topology decisions (spill, ordering, thresholds) | S1.3C | **partly** | wet floor in integer codes (`wet_floor.rs`); ties by cell index (GEN-100); flood and spill in f64 | |
| Cross-platform CI (Windows, Linux x86-64, Linux ARM64, macOS ARM64) | S10.1 | **partly** | Windows and Linux x86_64 compared in CI (determinism manifest, ADR 0010); ARM and macOS untested | |
| Edit-replay determinism | S1.6 | **partly** | items apply in canonical order (ADR 0007); no replay test across platforms | |

### 2.7 Scale and storage

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| No fixed metre resolutions; hierarchical spatial domains per phenomenon | P5.1, R3 | **partly** | per-body `res` from a target cell size (ADR 0045 point 2, proposed); TileId addresses any body (ADR 0003); the M9 bake shape is fixed (`BakeShape::production`) | |
| Geometric, physical and procedural parameters in a planetary preset | R3.3 | **partly** | ADR 0108 (Earth is the default preset); SPIKE-103 lists about 35 rows of climate literals and four radius copies still to lift; no gravity parameter | |
| Hierarchical, sparse storage; explicit lifetime of intermediate fields | S9.2 | **partly** | packs with compression and a compaction cap (ADR 0019, ADR 0029); dense per-face arrays in pass 1 (10 GB peak, SPIKE-104 §5) | |
| Scaling tests by growth (double radius, more watersheds) | S9.4 | **missing** | budgets fixed at Earth (BUDG-014, ADR 0097) | |

### 2.8 Process

| Recommendation | Where | Status | Ours | Note |
|---|---|---|---|---|
| Synthetic validation fixtures (mountain and river, coastal range, plain with meander, endorheic basin, glacial valley, cube-face crossing) | P11, S10.2 | **partly** | synthetic grids in unit tests (cone, bowl, closed basin, diagonal plane; GEN-100 and GEN-101 scenarios); no named landscape suite; the scorecard samples real-like tiles (TEST-081 ff) | |
| Measure connectivity, not a single pit share | S10.2 | **missing** | the measures are pit share and depth (SPIKE-102, SPIKE-104) | |
| Turn the architecture into ADRs (ADR-0200 to 0212) | S11 | **n/a** as numbered | our series continues at 0110; decisions map to existing ADRs where this section says "there" | |
| Build a vertical prototype of a 500 x 500 km region before rewriting | R12 | **missing** | the M9 previews are of the existing pipeline (ADR 0097) | phase 2 of this spike |
| "Stop optimising the erosion passes; fix surface identity and topology first" | P final | **partly** | SPIKE-104's D-A and D-B are these two fixes; the pass-1 memory diet and timing (M9.1 P2, P4) are optimisation | |

Count: of 55 rows, 14 there, 27 partly, 13 missing, 1 n/a. The foundations the review asks to keep (CPU-canonical determinism, keys and versions, item commands, stale-result pre-emption, priority flood, implicit stream power) are built. The missing core is one cluster: **an explicit, persistent hydrology (drainage graph, rivers, basins, lakes) and the edit path that updates it**.

## 3. Conflicts with the requirements

Weight: **blocks** (the requirement cannot be met while the rule stands); **strains** (met only at a cost the rule does not foresee); **reading** (a recorded reading or a small amendment resolves it). "Change by": who must decide (the constitution only by the owner; ADRs by their owner under the delegation rules).

| # | Requirement | Conflicting rule or plan | How they conflict | Weight | Change by |
|---|---|---|---|---|---|
| X1 | R1a, R4b (edits update erosion and rivers) | ADR 0007 (layer stack: authored layers are stages **after** the base); ADR 0066 point 4 (authored layers keyed as a chain over the base tile; precondition: "a stage reads the base only as a height field at points") | an edit sits on top of the eroded surface; erosion and drainage never read it. Moving items below erosion breaks point 4's precondition (the erosion stage would read items over a whole face), which the ADR itself says needs a new ADR | **blocks** | ADR (owner) |
| X2 | R1a, R4b | ADR 0086 point 3 (edits under the erosion deferred to M9's erosion design); ADR 0045 point 8 (global authoring and hydrology out of scope) | the deferral was never picked up: ADRs 0088-0109 define a seed-only bake with no edit input | **blocks** (an open decision, not a rule) | ADR (owner) |
| X3 | R4b (an edit can redirect a river far downstream) | ADR 0066 point 1 (an item's reach is a radius, at most one cell edge, about 40 km); ADR 0066 points 2-3 (invalidation by footprint) | hydrological reach is unbounded and follows the drainage graph, not a radius; a radius-bounded key cannot invalidate a river 1,000 km downstream | **blocks** | ADR (owner) |
| X4 | R1a, R4b | CON-08 (global-coarse -> regional with a bounded halo -> local; a stage reads only earlier outputs) | drainage is global by nature: a regional stage with a bounded halo cannot route it (SPIKE-68, ADR 0077 context), so hydrology must be a global stage; then every edit re-keys a global stage unless the stage updates incrementally. CON-08 does not forbid a global stage that reads items, but its metamorphic tests (border agreement, partition invariance) assume no later stage reaches back | **strains**; a reading suffices if hydrology is one global stage with incremental update and from-scratch equality | constitution reading, then ADR (owner) |
| X5 | R1a with CON-05 and CON-14 (regenerable caches, bit-exact) | CON-05 (every representation is a disposable cache regenerable from the definition), CON-14 (bit-exact across runs and threads) | an incrementally updated region must equal a from-scratch regeneration bit for bit, or a cache eviction changes the world. Incremental hydrology and erosion that land on the same bits as a full run are a hard algorithmic constraint (the review's "evolution reproducibility") | **strains**: it shapes every incremental algorithm | ADR (owner); no amendment if kept |
| X6 | R2 (preview now, larger re-bake later), R3 | CON-07, CON-14 (one implementation per field layer; neighbouring tiles share one `implementation_id`) | a low-res preview erosion in the edited region next to full erosion elsewhere puts two implementations of one layer side by side; seams follow | **reading**: the preview is its own field (an overlay with its own id), never a substitute tile of the final layer | ADR (planner) |
| X7 | R2, R3 (background re-bake while the editor runs without stutter) | CON-21 and BUDG-005 (RAM <= 8 GB; terrain 4.0 GB); BUDG-003 (terrain 60 % of B, at most 10 threads) | pass 1 needs 10.0 GB for one face (SPIKE-104 §5) and 20 min a face; inside the editor it must fit terrain's 4 GB beside the streaming caches and run on its share of B | **blocks** a per-face pass 1 as the edit path; a regional solver over bounded domains is needed | ADR (owner: the budgets are CON-21) |
| X8 | R2 (regional re-bake of an edited region) | ADR 0088 point 4.1, ADR 0099 (pass 1 per whole cube face; faces do not route across edges) | the smallest re-bake unit is a face (one sixth of the planet); there is no regional pass 1 with boundary conditions from outside | **blocks** a regional re-bake | ADR (owner) |
| X9 | R4a (hydrology a first-class output) | GEN-110 (drainage area as a byte), ADR 0041 (the sea is the only water producer), ADR 0086 (the global river network removed), ADR 0101 point 3 (the runtime adds the fill depth back) | rivers and basins exist only implicitly in heights and two byte channels; no river or basin object, id or lake | **blocks** | ADR (owner) |
| X10 | R4a | SPIKE-104 §3; ADR 0099 point 3, GEN-112 (C1 reads at the hand-over and at runtime) | the reads destroy connectivity between grids (13.3 % of land in pits from the read alone) | **strains**; SPIKE-104 D-A and D-B repair the bake; an explicit network removes the dependence | spec and ADR (owner) |
| X11 | R5a (moon to larger than Earth) | `BakeShape::production` (12,288 / 98,304 cells, tile 3,072), class lattice 256 a face, orography 512 cells, coupled run 2048, coarse horizon `min(n, 512)` | grids are fixed counts per face, so cell sizes follow the radius (2x radius gives 2x cell edge at the same counts) | **strains**; ADR 0045 point 2 (proposed: `res` from a target cell size) is the decision that resolves it | ADR (owner) |
| X12 | R5a | ADR 0108 (accepted; lifting pending); SPIKE-103 (about 35 climate rows inline, four radius copies, Earth rotation and tilt as literals) | other bodies get Earth's climate and constants until the values are lifted | **strains** (decided, not built) | lanes, per ADR 0108 point 5 |
| X13 | R7 (about 40 GB at most) | ADR 0088 status (layer and channels about 24 GB) + ADR 0045 point 6 (field bake 11.7 GB) | about 36 GB used before any drainage graph, river geometry, basin objects or edit overrides. Twice Earth's radius at the same cell sizes is about 4x (about 144 GB) | **strains** at Earth, **blocks** larger planets at today's cell sizes | ADR (owner) |
| X14 | R6 (plausible and consistent wins) | the scorecard and gates measured against GLO-30 numbers (TEST-081 ff, M9.1 gate item 2) | no conflict in principle; the gates measure shares and depths, not connectivity or consistency (§2.8) | **reading**: add consistency measures (connectivity, from-scratch equality, cube-edge continuity) as gates beside the Earth comparisons | spec (planner) |
| X15 | R8 (little owner attention) | ADR 0017 (owner blesses lit, sky and water goldens), ADR 0097 point 5 (owner approves each preview), the M9.1 gate (owner accepts scorecard and preview) | every bake-affecting step loops through the owner; a re-plan with many new stages multiplies the rounds | **strains** | owner (delegation) |
| X16 | R1a, R2, R4b (the work after M9) | the M9 plan (M9.1 drivers into pass 1, then the planet bake I6; M9.2 runtime on the bake) | the planet bake freezes a seed-only, dense-change representation with implicit hydrology. Old stores need not stay compatible (owner, 2026-11-05), so the cost is the bake time plus the tuning and scorecard rounds done on a representation a hydrology-first design replaces | **strains** (wasted work, not a block) | owner (plan) |

Structural conflicts (the rows that block):
1. **Edits sit above erosion and hydrology** (X1, X2): the layer stack and the stage-key chain put authored items after the base. R1a needs height and driver edits below erosion and drainage.
2. **Hydrology has no representation of its own** (X9): rivers and basins are implicit in a dense change and two byte channels. R4a and R4b need a drainage structure that exists, persists and can be updated.
3. **Invalidation is geometric** (X3): reach is a radius. R4b needs a reach along the drainage graph (downstream, and upstream for basin merges).
4. **The erosion unit is a whole cube face, offline, at 10 GB** (X7, X8): no regional solver fits inside the editor's budgets.
5. **Scale is a count per face, and the store is close to its limit at Earth** (X11, X13).

What does not conflict: CPU-canonical determinism (CON-07, CON-14), content-addressed keys and versions (ADR 0006), command-style items with stable ids (ADR 0007), stale-result pre-emption (ADR 0072), simulation grids separate from render LOD (ADR 0014, ADR 0088), presentation-only ground detail (ADR 0035). The review asks to keep all of these.

## 4. Options, first sketch (no costs yet; phase 2)

Common target (all three options aim at it; C narrows it):
- **T1** One authoritative surface: what erosion carved is what the runtime draws (SPIKE-104 D-B; review P6, S3.4).
- **T2** Hydrology as its own persistent structure: drainage graph, basins with spill level and type, river segments, lakes; the heights are an input to it, not its only carrier (review R4; resolves X9, X10).
- **T3** Edits as inputs below erosion and drainage: height and driver edits (lithology, uplift, climate) feed the field and the simulation; overlay edits (stamps for roads, plots) stay above (resolves X1, X2).
- **T4** Recomputation by dependency, not by face: invalidation along the drainage graph, regional solves on watershed domains with boundary conditions from the graph, a preview overlay first and the full result later, each bit-equal to a from-scratch run (resolves X3, X5-X8).
- **T5** Scale as a parameter: per-body cell sizes, Earth as a preset (ADR 0045 point 2, ADR 0108; resolves X11, X12), a storage plan within 40 GB at Earth (X13).

### Option A: evolve the application step by step

| Keeps | Changes | Drops |
|---|---|---|
| everything below `generators` unchanged: `core` (coordinates, tile ids, hashing), `world-def` (items, canonical TOML), `cache` (packs, keys), `streaming`, `frame`, `render`, `editor`, `app`; the determinism contract (CON-07, CON-14, ADR 0010); the field bake, class source and drivers (ADRs 0089, 0102-0104, 0108); the implicit stream-power kernel, priority flood, spill graph and MFD (`bake/`) | in order, each a normal OpenSpec change behind a field version: (1) T1, SPIKE-104 D-A and D-B in the existing bake; (2) a hydrology stage after the erosion bake that turns pass 1's routing and the spill graph into a stored graph, basins and river segments (T2), lakes on the water field (ADR 0041 point 7's retrofit path); (3) T3 as a new layer order (height and driver items read by the field bake and the erosion; ADR 0007 and 0066 amended); (4) regional erosion domains cut by watershed with graph boundary conditions, used for re-bakes, while the first world build may keep the per-face pass 1 (T4); (5) reach along the graph (ADR 0066 amended); (6) the preview overlay field (X6 reading); (7) T5 | the fill-depth add-back as a lake stand-in (ADR 0101 point 3); ADR 0109's infill; the M9 planet bake in its current form (delayed until (1)-(2) at least); M9.1 items that only add work to the per-face pass 1 (pass-1 deposition) |

- The existing tests, goldens, streaming and editor keep running at every step; each step can ship alone.
- Patchwork risk: steps (3)-(5) amend the stage model that ADRs 0007, 0066 and CON-08 rest on. Without a written target first, they repeat the M9 pattern (five amendments to one chain). The mitigation is a target-architecture document and the vertical prototype as its reference before step (3).

### Option B: a more fundamental re-implementation of the terrain core

| Keeps | Changes | Drops |
|---|---|---|
| the engine around the terrain core: `core`, `world-def` (items extended), `cache` (packs, keys), `streaming`, `frame`, `render`, `editor` shell, `app`, CI and the golden process; the determinism contract; the analytic drivers (lithology, uplift, climate, class source) as library code; today's kernels (stream power, flood, spill, MFD, thermal, deposition) as tested building blocks | new subsystems with their own crates or modules, per the review's data model: planet state (definition + presets), terrain state per region (base, displacement, sediment with version links), hydrology (graph, watersheds, basins, rivers), geomorphology (regional solver on watershed domains, local refinement with boundary conditions), simulation (dependency graph, dirty regions, versioned snapshots, preview and full quality); one `TerrainSampler`-like interface that the runtime field reads. Built first as the 500 x 500 km vertical prototype **in this repository** beside the current pipeline (a new field implementation id, CON-07), switched over when it passes | the two-pass erosion bake as the architecture (ADRs 0088, 0099, 0100, 0101, 0105, 0109 superseded by new ADRs); the kind-13 layer format and the fill-depth channel semantics; the M9 planet bake; open M9.1 items that only serve the two-pass bake |

- "Fork" here means a parallel implementation in the same repository and CI, not a separate repository: the streaming, render and test infrastructure is the larger part of the code and is not in question.
- Patchwork risk is lowest: the new core is designed as one piece against T1-T5. The risk moves to delivery: nothing ships to the owner's view until the prototype passes, and the drivers, look and scorecard tuned on the old bake are tuned again.

### Option C: cut requirements and keep more of today's design

Each cut is independent; A or B can absorb any of them. What each cut lets the design keep:

| Cut | Affects | Lets the design keep | What the owner gives up |
|---|---|---|---|
| C-a: erosion fixed at the seed; edits re-route water and update rivers and lakes, but do not re-erode | R1a (half) | the offline erosion bake as is (with T1); hydrology (T2) recomputed on the edited surface only, which is a routing, not a simulation | valleys do not respond to raised mountains or changed rock; "editing bugs" in erosion stay unseen |
| C-b: hydrology and erosion respond to edits only by an explicit re-bake of whole faces (minutes to an hour), no regional path | R2 (the regional re-bake) | per-face pass 1 as the only unit (no regional solver); the edit is shown on top, flagged stale, until re-bake | no quick regional result; one-sixth-planet waits |
| C-c: driver edits (climate, lithology, uplift) only before the first world build, not as later edits | R2 (regional property edits) | drivers stay analytic functions of seed and preset; no driver invalidation | regional climate and rock edits after the build |
| C-d: rivers implicit at runtime (derived per view from stored area and a threshold), no persistent river objects | R4a (partly), R4d | today's channels plus a derived river renderer | stable river ids for gameplay and authoring, rivers as game objects |
| C-e: engine scale fixed at Earth for the MVP; other radii only by a later per-body `res` | R5a ("in principle") | the fixed bake shape and Earth constants until after the MVP | scale tests now; risk of Earth assumptions hardening further |
| C-f: a coarser pass-2 grid (about 200 m) to make room in the store | R7, R6 | the store under 40 GB with hydrology added | detail between 100 and 200 m comes from runtime creases only |

Readings:
- No single cut removes all five structural conflicts. C-a + C-b together keep the bake nearly as is, but they contradict R1a's stated purpose ("else editing is too limited"), which the owner marked firm.
- C-e and C-f are the cuts that touch the fewest firm requirements; C-d touches a post-MVP item only (R4d) and part of R4a.
- A and B share the target T1-T5; they differ in order and in whether the old chain is amended in place (A) or replaced beside itself (B). The vertical prototype of the review (R12) is the experiment that decides between them: if the prototype's hydrology and regional solver can be fed by today's drivers and kernels with small changes, A's steps (3)-(5) are cheap; if not, B is the honest route.

Phase 2 (planner's next entry): costs and risks per option, the vertical-prototype plan, feasibility checks by coder agents, the recommendation and the owner decisions.
