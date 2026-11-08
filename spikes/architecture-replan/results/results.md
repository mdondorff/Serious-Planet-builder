# SPIKE-105: architecture re-plan of terrain and hydrology (phase 1)

Opened: 2026-11-08 (planner inbox #96; owner go for phase 1 only). Run on Opus.
Status: closed 2026-11-08 (phases 2 and 3, inbox #97 to #99). Answer: option A, conditional, prototype first against a written target with switch-to-B criteria (§13.1); owner decisions D1-D10 in §13.5 supersede §9.1.
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

# Phase 2 (inbox #97, 2026-11-08)

## 5. The options in depth: effort, risks, what stays, the effect on M9

All efforts are **estimates in lane-days** (one agent lane, one working day), ranged; none is measured. Lanes: explorer (bake, stores), crusader (runtime, shaders, editor), spikes (measurement, prototypes). Basis for the ranges: comparable past changes (the M9 drivers took about 1-3 lane-days each, the keep-pits seams change 3-4, the chunk stage about 15 over M8) and the size of the step.

Two lessons from history bound every option:
- **Routing must be global or bounded by drainage divides** (SPIKE-68: per-tile routing left 134-172 m seams; SPIKE-70: a global face carve then a capped local carve has none). A regional solve is seamless only if its domain border is a drainage divide (no flow crosses it) or its border conditions come from a global solve.
- **Regional solves must never run on the frame's critical path** (ADR 0086: the chunk stage failed the no-degradation gate with held-back tiles and slow chunk frames). A re-solve is a background job whose result is stored and swapped in by key (ADR 0072), with a valid older version always drawn (CON-22).

### 5.1 Option A: evolve step by step

| Step | What | Lane | Effort | Depends on |
|---|---|---|---|---|
| A1 | Surface identity: pass-2 breach (SPIKE-104 D-A), pass 2 on the store's lattice (D-B), C1 default (#1137) | explorer | 3-5 | - |
| A2 | Turned-lattice noise for the bands and creases (D-C), re-tune of the band factors | crusader | 3-5 | - |
| A3 | A target-architecture document and its ADRs (hydrology representation, layer order, domains, reach, preview levels) | planner, spikes | 3-5 | - |
| A4 | Hydrology stage v1: a stored drainage graph from pass 1's final routing (receivers, area), basins from the spill graph (`bake/spill.rs`) with floor, spill level, outlet, type, area curve; river segments above a runoff-weighted area threshold; debug views | explorer | 10-16 | A1, A3 |
| A5 | Lakes: basins as producers of the water field (ADR 0041 point 7's retrofit path); the fill-depth add-back retired | crusader | 5-8 | A4 |
| A6 | Rivers at runtime: channel geometry from segments in the runtime field and a river water surface | crusader | 8-14 | A4 |
| A7 | Edits below erosion: a new layer order (height and driver items as inputs of the field bake and the erosion); ADRs 0007 and 0066 amended | explorer, crusader | 6-10 | A3 |
| A8 | Regional erosion on watershed domains: the erosion of a set of catchments with outlet levels from the graph, the same kernels; the first world build runs the same domains, so a re-solve equals a from-scratch run by construction | explorer | 12-20 | A4, A7 |
| A9 | Invalidation along the graph (downstream reach, basin merge and split upstream) on top of the stage keys | explorer | 6-10 | A4, A8 |
| A10 | Preview levels per edit kind (overlay field, coarse re-route in seconds, regional re-solve in minutes) | crusader | 6-10 | A8, A9 |
| A11 | Scale as a parameter: per-body cell sizes (ADR 0045 point 2), SPIKE-103's Earth values lifted | spikes, lanes | 6-10 | - |
| | **Total** | | **68-113** | |

- Calendar at three lanes in parallel: about 5-8 weeks after A3 (the critical path is A1 -> A4 -> A8 -> A9 -> A10, about 37-61 lane-days on one lane, shortened by splitting A4 and A8).
- **Keeps from main:** everything below `generators` (`core`, `world-def`, `cache`, `streaming`, `frame`, `render`, `editor`, `app`); the drivers (ADRs 0089, 0102-0104, 0108); the kernels in `bake/` (carve, flood, spill, route, thermal, deposition); keys and versions; the field bake. About 90 % of main's code stays useful unchanged; the rest is amended in place.
- **Retires:** the per-face pass 1 as the only erosion unit (A8 replaces it with catchment domains; a face run can stay as the initial build if it is bit-equal, which needs one domain definition for both); the fill-depth add-back (A5); ADR 0109's infill and the pass-1 deposition draft (#1223).

| Risk | Kind | How it is retired |
|---|---|---|
| A8: catchment domains change when an edit moves a divide, so the domain set is a function of the edited surface; two runs with different domain sets differ | technical | domains come from the coarse graph of the **edited** surface, a pure function of the definition; the vertical prototype (§7) checks re-solve = from-scratch on one edit |
| A8: large catchments (Amazon-size, millions of km^2) are domains far larger than the editor's 4 GB | technical | nested domains: a large catchment is cut at confluences with the trunk's level from the graph as the boundary (a "sub-catchment with a fixed outlet"); prototype measures the error at the cut |
| A4 + A6: river geometry at runtime costs frame time and VRAM | technical | a measured budget line (BUDG) before A6; rivers are tile data like the channels, not per-frame work |
| Patchwork: A7-A10 amend ADR 0007, ADR 0066, CON-08's reading one by one | patchwork | A3 first: the target and its ADRs are written before any of A7-A10 starts; each step is checked against the target by spec-guardian |
| Schedule: the M9 bake slips by the length of A1-A5 at least | schedule | the owner decides which steps must precede the bake (§9); A1-A2 fit the current M9.1 window |

### 5.2 Option B: a new terrain core beside the current one

| Step | What | Lane | Effort | Depends on |
|---|---|---|---|---|
| B1 | Target architecture and data-model ADRs (as A3, deeper: module boundaries, interfaces, numeric representations) | planner, spikes | 5-8 | - |
| B2 | Vertical prototype of the new core on one 500 x 500 km region (§7), behind its own implementation id | spikes, explorer | 15-25 | B1 |
| B3 | Planet state and terrain state per region (base, displacement, sediment with version links); the `TerrainSampler`-like interface the runtime reads | explorer | 10-15 | B2 |
| B4 | Hydrology subsystem planet-wide (graph, watersheds, basins, rivers, lakes) | explorer | 12-20 | B3 |
| B5 | Geomorphology on watershed domains (today's kernels re-housed) | explorer | 10-16 | B4 |
| B6 | Simulation layer: dependency graph, dirty regions, snapshots, preview and full quality | crusader | 10-16 | B3 |
| B7 | Runtime integration: the new sampler under streaming and render; rivers, lakes, materials from the new data | crusader | 10-16 | B3, B4 |
| B8 | Editor integration: items as commands on the new state, previews | crusader | 6-10 | B6, B7 |
| B9 | Scale as a parameter, built in (A11's content) | spikes | 4-8 | B3 |
| B10 | Switch-over: drivers, look and scorecard re-tuned on the new core; goldens; the old pipeline retired | all | 10-15 | B4-B8 |
| | **Total** | | **92-149** | |

- Calendar at three lanes: about 7-11 weeks (critical path B1 -> B2 -> B3 -> B4 -> B5 -> B10, about 62-99 lane-days on one lane, shortened by B6-B9 running beside it).
- **Keeps from main:** the same crates below `generators` as A, the drivers as library code, the kernels as building blocks; about 70-80 % of main's code stays useful (the bake orchestration, layer store formats and runtime layer read are replaced).
- **Retires:** the two-pass erosion bake as the architecture (ADRs 0088, 0099, 0100, 0101, 0105, 0109 superseded), kind 13 and the fill-depth channel, the M9 planet bake, the M9.1 items that serve only the two-pass bake.

| Risk | Kind | How it is retired |
|---|---|---|
| Nothing reaches the owner's view for 4-6 weeks (B2 is the first visible result) | schedule | B2's prototype is viewable in the viewer from its first week (one region, its own id), so the owner sees progress |
| Two pipelines in one repository for weeks (CI time, keys, goldens) | technical | the new core is a field implementation like any other (CON-07); the old one is frozen, not developed, while B runs |
| The tuning done on the old bake (drivers, class amplitudes, scorecard) moves | schedule | the drivers are analytic and reused unchanged; only the amplitudes are re-tuned (B10) |
| Patchwork risk | patchwork | lowest of the three: one design, written first (B1) |

### 5.3 Option C: requirement cuts

| Cut | Effect on A or B | Effort saved (estimate) | Risk it adds |
|---|---|---|---|
| C-a: erosion fixed at the seed; edits re-route water only | removes A8, most of A10 (or B5's edit path); keeps A4-A6 | 15-25 lane-days | the owner's stated purpose of R1a ("editing bugs go unnoticed") is not met for erosion; a later add needs A8 anyway |
| C-b: hydrology and erosion respond only to a face re-bake | removes A8-A10's regional path; keeps the per-face pass 1 | 20-30 | a 20-min per-face wait and 10 GB: not inside the editor (X7) |
| C-c: driver edits only before the first build | removes driver invalidation from A9 | 3-5 | regional climate and rock edits come later with a re-key of the drivers |
| C-d: rivers derived at runtime, no persistent river objects | removes river segments from A4, simplifies A6 | 4-8 | R4d (authored rivers) needs the objects later |
| C-e: Earth-only engine scale for the MVP | removes A11 / B9 from the MVP | 6-10 | the Earth assumptions harden (SPIKE-103 found 35 rows already) |
| C-f: pass 2 at about 200 m | store: kind 13 shrinks about 4x (about 24 -> 8 GB) | 0 (a parameter), store +16 GB of room | detail 100-200 m from creases only; R6 judged on the look |

### 5.4 The effect on M9

| | A | B | C (with A) |
|---|---|---|---|
| M9 planet bake (I6) | after A1 + A4 at least (a bake with one surface and a stored graph); about 2026-12-01 to 12-10 (estimate) | after B10; about 2027-01 (estimate) | after A1; a bake on today's design with C-a/C-b's cuts, about 2026-11-22 to 11-29 (estimate) |
| M9.1 gate | redefined: T1 + T2 shown on the preview, connectivity measures (X14) beside the Earth comparisons | replaced by B2's prototype pass/fail (§7) and then B10 | as planned, plus A1 |
| M9.1 items | D2 (pass-1 deposition) dropped; D3, D4 (coastal, glacial) after A4; C3 (constants) folds into A11 | D2-D4 re-housed in B5; C3 into B9 | as planned |
| Work done on M9 so far | kept: drivers, kernels, store, runtime read; the tuning stays valid | drivers and kernels kept; tuning redone | kept |

## 6. The target architecture in our terms

Marks: **E** existing on main as needed; **X** existing, extended; **N** new. The target is the same for A and B; they differ in how it is reached (§5).

### 6.1 Components and the data each owns

| # | Component | Owns (authoritative data) | Today | Mark |
|---|---|---|---|---|
| K1 | Planetary state | the WorldDefinition: seed, radius, sea level, the preset (ADR 0108) with geometric, physical and procedural parameters, per-body grid sizes; authored items | WorldDefinition (CON-05, CON-10), items (ADR 0007, ADR 0066), Earth preset in progress (SPIKE-103) | **X**: per-body cell sizes (ADR 0045 point 2), gravity and rotation as preset values |
| K2 | Geology and climate drivers | analytic fields: plate provinces, lithology K(x, z), uplift U, class weights, wind; stored: orographic precipitation and temperature | GEN-094 to GEN-098, GEN-116, GEN-117, GEN-123, GEN-128 (all CPU, analytic or field-baked) | **X**: driver edits by region (ADR 0076 point 7 made real), re-keying by footprint |
| K3 | Base field | the base elevation on one authoritative lattice (the store's), read by one interpolant everywhere (bake and runtime) | field bake `global16384`, C1 read (GEN-107) | **X**: one lattice for pass 2 and runtime (A1), turned noise (A2), height items enter here (A7) |
| K4 | Coarse hydrology H0 | a planet-wide drainage graph at a coarse grid (about 3 km at Earth): receivers, area, watersheds and divides, trunk rivers and their levels | none (the coupled run of ADR 0077 was a version of it, removed by ADR 0086) | **N** |
| K5 | Regional geomorphology | per watershed domain (a set of catchments of H0, cut at confluences when too large): the implicit stream-power carve with K, U and runoff at the regional grid (814 m at Earth), breach and keep-pits, outlet levels from H0 as boundary conditions | pass 1 per cube face (ADR 0088, ADR 0099-0104) | **X**: the domain replaces the face as the unit; the kernels stay |
| K6 | Fine hydrology H1 | per domain, from K5's last routing: the drainage graph at 814 m (receivers 3 bits, area code), basins (floor, spill level, outlet, type, area curve), river segments (centreline, width, depth, discharge, profile) above a runoff-weighted area threshold, lakes (water level, kind) | drainage area byte (GEN-110), spill graph (GEN-121), fill depth (ADR 0101) | **N** (graph, segments, lakes), **X** (spill graph -> basin objects) |
| K7 | Local refinement | per pass-2 tile at the local grid (102 m at Earth): breach, incise, thermal, deposition, with the domain's channel positions and levels as boundary conditions | pass 2 (GEN-101) | **X**: breach (A1), channel boundary conditions from K6 |
| K8 | Multiresolution surface | `base (K3) + regional change (K5) + local change (K7) + channel geometry (K6 segments) + creases`, each term keyed to the identity of the base lattice it was taken on; one sampler for CPU systems and tiles | runtime field (GEN-099, GEN-112), `HeightField` | **X**: base identity in every layer key, channel geometry term, fill add-back removed |
| K9 | Simulation and versions | the dependency graph (stage keys + drainage reach), dirty sets per kind, background jobs, snapshot by key, preview and full quality as separate fields | stage keys (ADR 0006, ADR 0066), stale pre-emption (ADR 0072) | **X**: reach along the graph (A9), preview overlay fields (A10) |
| K10 | Editor | items as commands (heights, drivers, later lakes and rivers), undo, previews | items, session undo, edit latency (ADR 0072) | **X**: driver items, preview display, "stale" indication |
| K11 | Runtime | streaming, LOD, render, materials, water | `streaming`, `frame`, `render` (CON-03, CON-22) | **X**: river and lake water from K6, materials from K6 and K7 channels |

### 6.2 What feeds what

```
K1 definition + items
  -> K2 drivers (analytic; precipitation and temperature field-baked)
  -> K3 base field (height items applied here)
  -> K4 H0: coarse routing, watersheds, trunk levels           [global, coarse]
  -> K5 regional carve per watershed domain, outlet levels from K4   [per domain]
  -> K6 H1: graph, basins, rivers, lakes per domain                 [per domain]
  -> K7 local refinement per tile, channels from K6                 [per tile]
  -> K8 surface = K3 + K5 + K7 + channel geometry (K6) + creases
  -> K11 runtime; K10 editor writes K1 items
```
- The first world build runs exactly this chain over every domain and tile, so an edited and re-solved world equals a from-scratch build of the edited definition, bit for bit (X5).
- K4 is the only stage that is global at runtime cost; at about 3 km cells it is about 5.7e7 cells at Earth (6 x 3072^2): seconds to minutes (to be measured, §8).

### 6.3 Versioning and dirty-region propagation along the drainage graph

| Step after an edit | What is dirty | Rule | Mark |
|---|---|---|---|
| 1 | base tiles and driver tiles under the item's footprint | stage keys by footprint (ADR 0066) | E |
| 2 | K4 (planet-wide coarse graph) | re-routed in full (it is cheap); its outputs are compared per watershed | N |
| 3 | watershed domains | a domain is dirty when its base, drivers or its H0 entries (divides, receivers, outlet level, inflow area) changed; a moved divide dirties both sides | N |
| 4 | downstream domains | the outflow (area, discharge, sediment if coupled) of a dirty domain enters its downstream neighbour's boundary; propagate downstream until the change quantises to zero | N |
| 5 | K6 per dirty domain, K7 tiles inside dirty domains | stage keys chain through the domain's key | X |
| 6 | runtime tiles | re-keyed tiles refresh, drawn first, stale results dropped (ADR 0072) | E |

- Each derived product carries the keys of its inputs (ADR 0006); a version is a set of keys, so a snapshot is the key set at a commit point and a background result for an older key set is dropped (ADR 0072 point 3).
- A transaction is per domain: K5, K6 and K7 of one domain become visible together (the renderer keeps the older domain until all three are ready).

### 6.4 Preview levels per edit kind (R2)

| Edit kind | Level 0 (frames) | Level 1 (seconds) | Level 2 (minutes, background) | Level 3 (on demand, longer) |
|---|---|---|---|---|
| Height sculpt, stamp, raise or lower a range | the height edit drawn over the current surface (ADR 0072) | K4 re-route: watersheds, trunk rivers move; shown as an overlay | K5-K7 of the dirty domains | domains downstream beyond a limit, or the whole chain for a continent-wide edit |
| Lithology or uplift region | the driver map in the region (existing preview maps, GEN-122, GEN-125) | - (no routing change) | K5-K7 of the domains under the region | as above |
| Climate region (rain, temperature) | the climate preview map (GEN-130) of the region | K4 re-route with new runoff weights (rivers' discharge, arid sinks) | K5-K7 of the domains whose runoff changed | the planet's climate sweep if the edit changes the wind field's input (orography) far away |
| Class paint | weights preview (exists) | - | as height edit | as height edit |
| Lake or river authoring (post-MVP) | the object drawn | K6 objects edited; K4 constraint | K5-K7 conditioned on the object | - |

- Level 0 and 1 are previews (their own fields, X6); level 2 and 3 are the authoritative chain.
- A "too global" edit (R1b, game mode) is one whose level-2 domain set exceeds a budget; the game can refuse it, the editor runs it at level 3.

### 6.5 Scale as a parameter (R5)

| Element | Today | Target |
|---|---|---|
| Grid sizes | counts per face (`BakeShape::production`: 12,288 / 98,304) | target cell sizes in metres per body (K1), counts derived (ADR 0045 point 2) |
| Domains | the cube face | watershed domains of H0: their count grows with land area, their size is bounded by the memory budget, not by the planet |
| Constants | Earth literals in kernels (SPIKE-103) | preset parameters (ADR 0108), gravity and rotation included |
| Moons and dry bodies | - | a body without a sea or runoff skips K4-K7 (no hydrology); craters and the base only (R5b) |
| Larger planets | 4x store at 2x radius | coarser target cells for the local level (C-f style) on large bodies; the store plan per body (R7) |

### 6.6 Store budget (R7), Earth, estimates

| Item | Today | Target (estimate) | Note |
|---|---|---|---|
| Field bake (4 x f32, 16384) | 11.7 GB (measured, ADR 0045) | 5-8 GB | narrower channels for temperature and precipitation (ADR 0045 point 4, open) |
| Regional change (814 m, i16) | folded into kind 13 | about 0.5 GB | 6 x 12,288^2 x 0.29 land x 2 bytes |
| Local change and channels (102 m) | about 24 GB (owner, 2026-11-04) | 16-24 GB, or about 6-8 GB at 200 m (C-f) | kind 13 plus area, deposit and fill bytes |
| H0 graph (3 km) | - | under 0.1 GB | |
| H1 graph (814 m): receivers 3 bits, area 1 byte | area byte only | about 0.4 GB | |
| Rivers (segments, centrelines) | - | 0.5-2 GB | depends on the threshold; feasibility check §8 |
| Basins and lakes | fill depth u16 in kind 13 | under 0.1 GB for objects; the fill channel can go | |
| **Total** | **about 36 GB** | **about 23-35 GB** (about 13-17 GB with C-f) | inside 40 GB if the field bake is narrowed or C-f is taken |

### 6.7 Realtime on Rasierklinge (R3)

- The frame never waits for simulation: every re-solve is a background job on terrain's core share (BUDG-003: 60 % of B, at most 10 threads), stored and swapped by key; a valid older version is always drawn (CON-22, ADR 0072).
- Memory: a level-2 job must fit beside streaming in terrain's 4.0 GB (BUDG-005). At pass 1's measured 64 bytes a cell (10.0 GB / 1.56e8 cells), a 1.5 GB job is about 2.3e7 cells at 814 m, about 1.5e7 km^2: larger than every catchment but the largest few, which are cut at confluences (§5.1 risk). Local tiles (3,072^2 + halo) already fit (under 2 GB).
- Rivers and lakes are tile data (segments rasterised into the tile's channel block, water levels per body as today's sea), not per-frame work; their GPU cost is a budget line measured before A6 (BUDG-002).
- The chunk stage's failure mode (solves on the critical path, ADR 0086) is excluded by construction: no solve is requested by the plan; the plan requests tiles whose keys exist.

## 7. The vertical prototype

### 7.1 Scope

- **Region:** 500 x 500 km of seed 14 holding a mountain range, a plateau, a river system with at least three tributaries above the river threshold, a coastal outlet and one intentional closed basin (arid or tectonic). Chosen by a search over the class map, the coarse routing and the aridity (a spikes task of about half a day; candidate area: around tile 2/12/10, SPIKE-104's mountain tile, which has mountains and a coast on face 2).
- **Second, small case:** a 100 x 100 km window across a cube edge with one river crossing it.
- **Chain:** K3 (one base lattice), K4 (H0 at about 3 km over the whole planet, since watersheds can extend beyond the region), K5 on the watershed domains that intersect the region, K6 (graph, basins, rivers, the closed basin as a lake), K7 on the pass-2 tiles of the region, K8 (the surface with channel geometry), K11 (viewer and stills through the product path, rivers and the lake drawn), K9 (one edit's propagation), K10 only as one scripted edit item.
- **Edit:** raise a 50 x 50 km part of the range by 1,000 m (a height item below erosion), then undo.
- **Out of scope:** glacial and coastal processes, materials beyond today's, meanders, seasons, other planet sizes (one scaled run of K4 only, §7.2 Q7).

### 7.2 Questions it must answer (A against B)

| # | Question | Decides |
|---|---|---|
| Q1 | Do today's kernels (carve, flood, spill, route, breach) run on a watershed domain with outlet levels from H0, unchanged in their core? | A if yes with small changes; B's re-housing if not |
| Q2 | Is the domain decomposition stable enough: does a re-solve after the edit equal a from-scratch build of the edited definition, bit for bit, on 1 and 8 threads? | the feasibility of X5 under either option |
| Q3 | Do river segments and basins from H1 keep connectivity through K7 and through every runtime sampling (the C1 read at the store lattice, the render LODs)? | whether the hydrology must be authoritative (graph first) or can stay derived (C-d) |
| Q4 | Can the runtime field add channel geometry and stay within the plan's bounds (the pyramid, GEN-102) and the frame budget? | A6 / B7 shape |
| Q5 | Can the derived data (graph, domains, segments) live in today's pack store and key scheme (ADR 0019, ADR 0006), or does it need a new store model? | A (fits) or B (new store) |
| Q6 | Edit latency and memory per preview level (L1, L2) on terrain's share | R2 and R3 feasibility |
| Q7 | Store per 500 km region extrapolated to the planet, and H0's cost at 2x radius | R7, R5 |

Decision rule: if Q1, Q2 and Q5 pass with changes confined to new stages plus the amended layer order, **A**. If Q1 or Q5 fails (the kernels or the store cannot carry domains, versions and graph data without a new data model), **B**. Q3, Q4, Q6 and Q7 shape either option; a failure there is a requirement question (C).

### 7.3 Pass and fail criteria (proposed numbers)

| Criterion | Measure | Pass |
|---|---|---|
| Surface identity | runtime height minus the bake's surface at the bake's lattice nodes | 0 at nodes (bit-equal before quantisation); within the 0.25 m quantisation step after |
| Connectivity | river segments disconnected at any supported sampling (814 m, 102 m, runtime read at levels covering 611 m to 1 m) | 0 segments; channel cells with no downhill path to their segment's outlet: 0 |
| Unintended sinks | land cells more than 0.5 m under the flood that belong to no classified basin | at most 1 % of the region's land (SPIKE-104: 1.0 % with a 102 m breach) |
| Intentional basin | the chosen closed basin | classified as closed, kept, drawn as a lake (or playa if arid) |
| Relief | 3 km relief p50 in mountains | at least 0.9 of SPIKE-104's breached surface (514 m on 2/12/10) |
| Cube edge | the crossing river | one segment chain across the edge, endpoints within one cell, area conserved to 1e-9 relative |
| Edit result | the raised range | rivers move where the divide moved; downstream discharge changes; re-solve = from-scratch build bit for bit |
| Edit latency | L1 (H0 re-route) and L2 (domains of the region) on 10 threads | L1 at most 10 s; L2 at most 10 min; peak memory at most 1.5 GB per job |
| Undo | undo of the edit | the world's key set equals the pre-edit key set |
| Determinism | 1 and 8 threads; Windows and Linux CI | bit-equal |
| Runtime | the region's flight in the viewer, rivers and lake drawn | no held-back tiles; frame within BUDG-001/002 (measured in a perf window, owner-free under ADR 0087's rules) |
| Store | the region's derived data, extrapolated to the planet's land | at most 35 GB at Earth (5 GB margin under R7) |

### 7.4 Effort and what carries over

| | Effort (estimate) | Lanes | Carries into production under A | Under B |
|---|---|---|---|---|
| Region search and fixtures | 1-2 | spikes | the region and the cube-edge window become the validation suite (review P11) | same |
| K4 H0 planet-wide + K5 on domains | 5-8 | explorer | A4 and A8's first half (H0, domains); the kernels unchanged | B4/B5 v0 |
| K6 graph, basins, segments, lake | 4-7 | explorer | A4, A5 | B4 v0 |
| K7 with channel boundaries + K8 surface | 3-5 | explorer, crusader | A1 (already planned) and A6's field term | B3, B7 v0 |
| Runtime: rivers and the lake drawn | 3-5 | crusader | A6 v0 | B7 v0 |
| The edit and its propagation | 3-5 | explorer, crusader | A7, A9 v0 | B6 v0 |
| Measurement against §7.3 | 1-2 | spikes | the gates of A's steps | B's switch-over gate |
| **Total** | **20-34** | three lanes, about 2.5-4 weeks | under A almost all of it is production code of A4-A9 for one region | under B it is the core's v0 |

- Under A the prototype is not throwaway: it is A4-A9 run on one region. That makes the decision cheap to reverse: if Q1 or Q5 fails half way, the region's code becomes B's v0.

## 8. Feasibility checks

Three checks by Sonnet sub-agents (owner go, inbox #98), each one question with a time box. All ran on seed 14, face 2, release builds, 4 threads, on a shared machine: **timings are indicative**. Peak memory is the process's PeakWorkingSet64, polled every 200 ms. Probe code: `spike/terrain-pipeline-review` (`crates/cache/examples/f1_coarse_routing.rs`, `f2_domain_carve.rs`, `f3_river_store.rs`; never merged).

### F1: planet-wide coarse routing (H0)
Question: what does one routing at about 3 km cost, and how large are the catchments?
Method: base sampled per cell with `BakeInputs::base`, then `route_weighted` (flood, jittered D8, MFD area) on the face with a 4-cell halo; outlets are the sea and the grid edge.

| | 3,072 cells (3.26 km) | 6,144 cells (1.63 km) |
|---|---|---|
| Cells | 9.49e6 | 3.78e7 |
| Base sampling (analytic) | 62.9 s | 205.3 s |
| Priority flood alone | 0.70 s | 1.88 s |
| Routing (flood, receivers, MFD area) | 3.40 s | 7.89 s |
| Peak memory (whole process) | 348 MB | 1,242 MB |
| Outlets draining land | 50,369 (48,928 sea) | 103,989 |
| Catchments over 1e5 km^2 | 42, holding 32.1 % of land | 31, holding 23.0 % |
| Largest catchment | 1.12e6 km^2 | 1.02e6 km^2 |

- Extrapolated to six faces: routing about 20 s, plus base sampling about 6.3 min with the analytic base. The production H0 would read the stored field bake instead, so sampling becomes a lattice read.
- Extrapolated to twice Earth's radius at the same cell size: routing about 1.4 min, about 1.4 GB per face.
- Caveat: catchments that end at the face edge are truncated (3 of the 10 largest). H0 must join faces, for example with the spill graph's cross-face edges (`bake/spill.rs`).

### F2: carving a watershed domain alone
Question: does the implicit carve on one catchment equal the same carve inside a larger window, and what does a domain solve cost?
Method:
- 1,228 x 1,228 cells at 814 m (82 % land), default `CarveParams`, carved whole.
- Its largest interior catchment D (117,486 cells, 77,925 km^2), found on the original routing, carved alone. Cells outside D were treated either as outlets at base height or as walls (outlets raised by 1e5 m).

| Measure | Result |
|---|---|
| Whole-window carve (1,228^2, 814 m) | 4.0 s (base sampling excluded), 188 MB peak |
| Divide migration in the whole-window carve | 276 cells of D drain out of D, 247 outside cells drain into D |
| D alone, outside as outlets | max abs difference 458.6 m, rms 5.7 m; 115,691 of 117,486 cells differ; 500 cells leak out |
| D alone, outside as walls | max 428.1 m, rms 4.8 m; 114,647 cells differ; no leaks |
| Domain carve 614^2 at 814 m (500 km) | 0.9 s, 169 MB peak |
| Carve 2,048^2 at 101.8 m (208 km) | 16.9 s, 200 MB peak |

- **Answer: no.** A domain carve does not reproduce the window carve inside the domain. The carve moves divides and trades drainage area across them, so a domain with a frozen border sees a different problem.
- Consequences for the target (§6):
  - The domain decomposition must itself be the authoritative definition, also for the first world build (equality by construction, X5). It cannot reproduce today's face carve.
  - Domain borders freeze divides for one solve. The prototype must measure the step across a domain border, and test an outer iteration: H0 on the carved surface, re-cut the domains, carve again, 2-3 rounds, a fixed count.
  - Cost is not the issue: a 500 km domain takes about 1 s at 814 m.

### F3: size of a persistent river network
Question: how large is a vector river network for an Earth-size planet, by channel threshold?
Method:
- Face 2's pass-1 drainage-area channel (GEN-110, the explorer's n0 store, read only), masked to land (28.6 % of the face).
- River length = channel cells x 814.4 m x 1.1.
- 24 bytes a vertex (position, elevation, width, depth, discharge) plus 32 bytes a segment.
- Extrapolated by planet land over face-2 land (x 6.09).

| Threshold | Channel length, face 2 | Planet length (extrapolated) | Planet store, vertex every 814 m | Every 102 m |
|---|---|---|---|---|
| 1 km^2 | 26.9e6 km | 163e6 km | 5.5 GB | 39 GB |
| 10 km^2 | 11.0e6 km | 66.8e6 km | 2.4 GB | 16 GB |
| 50 km^2 | 5.0e6 km | 30.5e6 km | 1.1 GB | 7.4 GB |
| 100 km^2 | 3.6e6 km | 21.8e6 km | 0.78 GB | 5.3 GB |
| 1,000 km^2 | 1.2e6 km | 7.6e6 km | 0.26 GB | 1.8 GB |

- Heads and confluences are noisy (area codes are 1/8-octave steps and tie often). Segments are under 3 % of the bytes, so the totals are solid.
- **Answer:** a river network at a 50-100 km^2 threshold with vertices on the 814 m graph is 0.8-1.1 GB. Finer channels are derived at runtime from the area channel and refined between the vertices.

### What the checks change in §6
| Item | Before | After the checks |
|---|---|---|
| Memory of a level-2 job (§6.7) | estimated from pass 1's 64 bytes a cell | a 78,000 km^2 domain solves in under 200 MB, a 500 km domain in 169 MB. The largest catchment (about 1e6 km^2, 1.7e6 cells at 814 m) is about 110 MB at 64 bytes a cell. X7's memory block dissolves once the unit is a domain, not a face |
| L1 preview (§6.4) | at most 10 s | routing alone is 3.4 s per face at 3 km. L1 for an edit's continent fits 10 s if the base is read from the store, not evaluated analytically |
| Rivers in the store (§6.6) | 0.5-2 GB | 0.8-1.1 GB at a 50-100 km^2 threshold, vertices at 814 m |
| Domain equality (§5.1 A8 risk, §7.2 Q2) | open | a domain carve is not a window carve. Domains must be the definition; divide steps at domain borders and an outer iteration are new prototype criteria |
| H0 across cube edges (§6.2) | implied | required: about 30 % of large catchments reach a face edge |

## 9. Recommendation

**Option A, prototype first: evolve the application towards the target T1-T5, with the vertical prototype built as A's own steps on one region before they go planet-wide; no requirement cut is needed.**

Reasons:
1. The checks remove B's main argument. The kernels already run on a watershed domain at about 1 s for 500 km (F2). A coarse planet routing costs seconds (F1). A river network fits the store at about 1 GB (F3). Today's code can carry the new structure; what is missing is the structure, not the machinery.
2. A keeps about 90 % of main unchanged and ships visible results within weeks (T1 and the noise fix in week 1). B shows nothing new for 4-6 weeks.
3. A's patchwork risk is the one that matters (§5.1). A3 retires it: the target document and its ADRs are written and accepted **before** any step that amends the stage model (A7-A10). Each later step is checked against the target, not against the previous patch.
4. The decision stays reversible. If the prototype fails Q1 or Q5 (§7.2), its code becomes B's v0.
5. No requirement cut is needed. C-f (pass 2 at 200 m) stays an option if the store goes over budget; C-e is not recommended (§9.1, D7).

### 9.1 Owner decisions

**D1. Route: A, B or cuts**
- Context: §4-§5; the checks in §8.
- Options:
  - a. A, prototype first: about 68-113 lane-days, visible results from week 1.
  - b. B, a new core beside the old: about 92-149 lane-days, first result after 4-6 weeks, lowest patchwork risk.
  - c. Today's design with cuts C-a + C-b: about 35-55 lane-days saved, but R1a's purpose is not met.
- Trade-off: A is fastest to visible results and keeps the tuning; B is the cleanest design; c is the cheapest and misses a firm requirement.
- **Recommendation: a.**

**D2. The M9 planet bake**
- Context: the bake would freeze a seed-only design with implicit hydrology (X16); old stores need not stay compatible.
- Options:
  - a. Delay the bake until the prototype passes and A4 has gone planet-wide (about early December, estimate).
  - b. Bake now on today's design with SPIKE-104's fixes as a stopgap, and re-bake later.
  - c. Bake now unchanged.
- Trade-off: b gives a look at the whole planet sooner, at the cost of about one day of compute plus tuning rounds spent on a representation that A replaces; c also carries the 13 % pit share.
- **Recommendation: a.** The M9.1 gate is redefined as the prototype's §7.3 criteria plus T1 planet-wide.

**D3. Watershed domains become the erosion unit and the definition**
- Context: F2. A domain carve differs from a window carve (rms about 5 m, max about 430 m) because divides move. Equality between an edit's re-solve and a from-scratch build therefore needs the domains to be the authoritative unit for every build.
- Options:
  - a. Domains from H0 as the unit, with a fixed outer iteration (H0 -> domains -> carve, 2-3 rounds).
  - b. Keep the cube face as the unit for the first build and use domains only for edits; then a re-solve differs from a fresh build (X5 broken).
  - c. Overlapping domains with blended results.
- Trade-off: a gives equality and edit locality, but its results differ from today's face carve and divides are frozen per round. b breaks determinism of edited worlds. c gives approximate equality and seams.
- **Recommendation: a.** The prototype measures the divide steps and the round count.

**D4. Which edits sit below erosion**
- Context: X1. Today every item sits above the base and the erosion.
- Options:
  - a. Height edits (sculpt, raise, lower, stamps meant as landforms) and driver edits (lithology, uplift, climate, class paint) go below erosion. Constructed features (roads, plots, flattened sites, objects) stay above, as ADR 0089 point 11's final override stage.
  - b. All height items go below erosion.
  - c. Only driver edits go below.
- Trade-off: a matches R1a and keeps built features exact; b erodes roads and plots; c leaves sculpting without erosion.
- **Recommendation: a.**

**D5. Rivers as persistent objects**
- Context: R4a, R4d; F3.
- Options:
  - a. Persistent river segments above a runoff-weighted threshold of 50-100 km^2, vertices on the 814 m graph (0.8-1.1 GB), finer channels derived at runtime.
  - b. Rivers derived at runtime only (C-d).
  - c. Persistent at 10 km^2 (about 2.4 GB).
- Trade-off: a gives stable ids for gameplay and later authoring within budget; b is cheaper but leaves no stable rivers; c costs store room.
- **Recommendation: a.**

**D6. Store room (R7)**
- Context: §6.6; about 23-35 GB in the target.
- Options:
  - a. Narrow the field bake's channels (ADR 0045 point 4) and keep pass 2 at 102 m.
  - b. Pass 2 at about 200 m (C-f).
  - c. Both.
- Trade-off: a costs a spike and a re-bake of the field; b loses 100-200 m detail from the bake.
- **Recommendation: a.** b only if the prototype's extrapolation exceeds 35 GB.

**D7. Scale in the MVP**
- Context: R5a; X11, X12.
- Options:
  - a. Per-body cell sizes and the lifted Earth preset as part of A (A11), with one 2x-radius run of H0 in the prototype.
  - b. Earth-only for the MVP (C-e).
- Trade-off: a costs 6-10 lane-days and keeps scale honest while the design is being set; b saves them and lets Earth assumptions harden.
- **Recommendation: a.**

**D8. Owner attention (R8)**
- Context: X15.
- Options:
  - a. The planner judges the prototype and each A step against numeric criteria (§7.3 and the step gates). The owner decides D1-D7 once, then sees the prototype's stills and flight at the end (one review) and blesses only first-of-class goldens (ADR 0017).
  - b. Owner reviews at every step.
- Trade-off: a is quicker and keeps the owner's time for real decisions; b gives tighter control.
- **Recommendation: a.**

### 9.2 The first two weeks (if D1-D8 go as recommended)

| Week | Explorer | Crusader | Spikes |
|---|---|---|---|
| 1 | A1: pass-2 breach and pass 2 on the store lattice (SPIKE-104 D-A, D-B), gated by SPIKE-104's probe numbers on 3 tiles; then H0 v0: planet-wide routing at about 3 km on the stored field bake, faces joined by the spill graph's edges | A2: turned-lattice noise for the bands and creases, re-tuned on the scorecard's band numbers; #1137 (C1 default) merged | region search for the prototype (§7.1) and the cube-edge window; the validation suite as fixtures (review P11) with connectivity measures (unintended sinks, disconnected segments, area conservation); then A3 with the planner: target document and ADR drafts (hydrology representation, layer order, domains, reach, preview levels) |
| 2 | prototype K4-K5: watershed domains from H0, domain carve with outer iteration, divide-step measure; K6 v0: basins from the spill graph, river segments at 50-100 km^2 | prototype K8 and K11: channel geometry term in the runtime field from segments (synthetic first), the lake on the water field (ADR 0041 point 7), a budget line for river drawing | measure §7.3 as pieces land (surface identity, connectivity, relief, store); D3's divide experiment (rounds 1-3); A11 groundwork: SPIKE-103's lifts that the prototype touches |

- Owner attention in these two weeks: the D1-D8 decisions once, ADR acceptance for A3 (architecture ADRs stay with the owner), and nothing else unless a gate fails. Everything else runs on numeric gates judged by the planner.
- After week 2: weeks 3-4 finish the prototype (the edit and its propagation, undo, latency, determinism), then the owner's one review, then A4-A10 planet-wide (§5.1).

# Phase 3 (inbox #99, 2026-11-08): the external review of this spike

Input: an external review of phases 1 and 2 (owner refs, 2026-11-08, outside git; called "the second review" below). The owner confirmed D1 = evolve (A) as the direction, the same verdict as this review. The reviewer knows neither our ADRs, specs nor code in detail, so each point below is checked against them.

## 11. Verdict on each point

Verdict: **adopt**, **adopt with change**, **already covered**, **reject**.

| # | Point | Verdict | Reason, references |
|---|---|---|---|
| 1 | F2 does not justify A; the erosion operator does not decompose into watershed domains; the outer iteration is unvalidated | **adopt** | §8 F2 measured exactly this and §9 still recommended A on the strength of cost, which F2 does not address. D3 becomes a hypothesis (§13); A8 is not approved for production before the prototype passes. The five questions (what is iterated, stopping rule, non-convergence, moving borders, what a border guarantees) are answered in the A3 contract (§13.4) and measured by F4 (§12) |
| 2 | F1 is not planet-wide; cross-face routing and stored-field sampling are not shown | **adopt** | F1 used one face with the edge as an outlet and the analytic base (§8). §8's wording "routing is cheap" is too strong. F6 (§12) measures both |
| 3 | Physical cell areas and distances belong in H0 and H1 from the start | **adopt** | `route.rs:125` uses `dx^2` for every cell and `Grid::distance` one `dx` (`bake/mod.rs`); §2.1 called this "under 1.5x, uniform per face", which understated it: area feeds river thresholds and erosion rates. Today's bake has the bias too. F5 (§12) sizes it. `route_weighted` already takes per-cell weights, so areas are a small change; distances need a per-cell metric in `Grid` |
| 4a | Separate the channel graph from diffuse drainage | **adopt** | the two already exist in code and are conflated only in §6's K6 row: jittered D8 receivers for the carve and MFD area for accumulation (ADR 0099 point 2). A3 assigns the roles: the channel graph (one receiver per channel cell, segments, confluences) and the diffuse field (MFD area, a raster) (§13.4) |
| 4b | Stable segment identity; basin identity under merge, split, breach, outlet migration | **adopt** | not defined in §6. A3 defines ids by a canonical rule (§13.4) and the prototype tests each case |
| 5 | Propagation cannot stop on "quantised to zero"; small changes have structural consequences | **adopt with change** | the mechanism in §6.3 is wrong as worded. It should not be a tolerance test but **exact equality of a domain's input key set** (the quantised interface values at its border and its drivers). Equal inputs give equal outputs bit for bit by construction (ADR 0006), so stopping is safe; a threshold crossing or spill inside a domain is handled because that domain's inputs changed and it is recomputed. Structural cascades (a moved divide) come from H0, which is re-routed in full on each edit. The review's concern remains for the interface precision: too coarse a quantisation of the outflow accumulates error downstream, so it is a parameter measured in the prototype. More edit cases: adopted (§13.3) |
| 5b | Reproducibility is not correctness: both can share the same domain-border error | **adopt** | the correctness measures (border flux, divide migration, connectivity) are separate gates from equality (§13.3) |
| 6 | 40 GB as a hard measured gate, including pack layout, graph metadata, indexes, versions, edit overrides; peak temporary storage separately; 200 m a fallback only | **adopt** | §6.6 and F3 are estimates; ADR 0045 point 6 is the only measured item. Adopted as a gate (§13.3) |
| 7 | 90 % of code is not 90 % of architecture; A is conditional; define switch criteria | **adopt** | §5.1 and §9 used the code share as an argument. The layer order, simulation unit, hydrology, invalidation and regional simulation all change. Switch criteria in §13.1 |
| 8 | Stronger prototype criteria | **adopt** | in §13.3 |
| 9A | The sampler's composition (order, interpolation, quantisation); channel geometry must not be a second elevation source | **adopt with change** | §6.1 K8 listed channel geometry as a separate term, which invites exactly that. Channel geometry is **burned into the local change at the bake** (K7 carves along the segment), so the surface has one elevation source. The runtime adds nothing above the 102 m grid. Below it, a bank profile may lower the ground only, inside a declared bound, as creases do (GEN-109) (§13.4) |
| 9B | Simulation time independent of editor time | **adopt with change** | partly there: `K dt`, steps and uplift are bake parameters in the keys (GEN-100, ADR 0088 point 7), so an edit never advances erosion by "some time". Missing is the named concept. A `SimulationProfile` in the WorldDefinition (duration as steps x `K dt`, the quality level) is added; every solve restarts from the definition, not from a previous result (§13.4) |
| 9C | Determinism hard: platforms, thread counts, edit replay, graph topology; f64 in flood and spill; no GPU feedback | **already covered**, with one addition | CON-07, CON-14, ADR 0010 and ADR 0035 hold all of it for Windows and Linux x86_64; f64 comparisons with no fast-math and ties by cell index (GEN-100) are deterministic there. ARM and macOS are untested and not supported targets (CON-26). Addition: results that later stages read must be read in their **canonical stored form**, not an unquantised f64 copy: `bake/tile.rs:322` stores f32 heights for the post-pass while the carve works in f64. A re-solve reading stored outputs and a from-scratch run reading in-memory f64 can then differ at a divide. The contract names the canonical form of every interface value (§13.4); edit-replay tests are added (§13.3) |
| 9D | Larger planets tested beyond H0; no cell-count-defines-resolution | **adopt** | design-level scaling table for terrain, hydrology, storage and climate in A3 (§13.4); the growth tests of the first review (S9.4) join the gates |
| 10 | Positions on D1-D8; next steps | **adopt** | D1 conditional, D3 a hypothesis, D5 configurable threshold and network validation, D6 subject to measurement, D8 with failures of architectural gates returning to the owner (§13.5). Next steps match §13.2 |

## 12. Feasibility checks of phase 3

Sonnet sub-agents, seed 14, release builds, 4 threads, shared machine: **timings are indicative**. Probe code on `spike/terrain-pipeline-review` (`f4_outer_iteration.rs`, `f5_cell_areas.rs`, `f6_cross_face_h0.rs`; never merged).

### F7: exact decomposition within a round (added for A3, inbox #100)
Question: if a round routes the whole window once and runs the breach and sweeps per catchment on that shared routing, does it equal the whole-window carve?
Method: `f7_exact_domains.rs`, F2's window; catchments are D8 trees of one window routing per round; forward, reverse and 4-thread catchment orders.

| Case | Cells differing from the whole-window carve |
|---|---|
| Copy of the loops on the whole window (control) | 0 |
| Per catchment, forward | 0 |
| Per catchment, reverse | 0 |
| Per catchment, 4 threads | 0 |

Routing locality (a catchment routed alone against the window routing, 5 largest non-border catchments): the first routing has 0 receiver and 0 filled differences, but 24k-52k cells of area differences (MFD shares cross D8 catchment borders, ratio up to 20); after round 1, 3-182 receiver differences and up to 3,191 cells of filled differences (up to 0.245 m, the flood's FIFO tie order). A round's cost is about 85-90 % routing, 10-15 % catchment work.

- **Answer:** with a shared routing the per-round decomposition is bit-exact. The error measured in F4 comes from routing each catchment alone. This changes the domain contract: see `docs/design/terrain-target.md` §6 (MFD clipped to the catchment; a canonical flood tie rule).

### F4: the outer iteration (route, cut domains, carve, repeat)
Question: on F2's window (1,228 x 1,228 cells at 814 m, 20 steps), how close does the outer iteration get to the whole-window carve R, is it stable, and what happens at the borders?
Method:
- Domains are D8 catchments of a routing of the current surface; every domain is carved alone (outside cells are outlets raised 1e5 m) and the results are assembled.
- Variants: **C** (continued surface, R rounds of 20/R steps) and **F** (fresh restart from the base each round, domains from the previous round).

| Variant | rms against R | max | Cells over 1 m | 3.3 km relief p50 (R 396.0 m) |
|---|---|---|---|---|
| C, 1 round | 18.3 m | 1,074 m | 19.4 % | 397.3 |
| C, 2 rounds | 15.2 | 1,158 | 18.0 % | 396.7 |
| C, 4 rounds | **12.8** | 966 | 17.5 % | 396.6 |
| C, 10 rounds | 57.5 | 1,485 | 78.9 % | 420.0 |
| F, round 1 / 2 / 3 / 4 | 18.3 / 17.7 / 18.1 / 18.4 | up to 1,413 | 19-22 % | |
| C, 4 rounds, domains from a 3.26 km grid (4x coarser) | 105 | 2,382 | 53 % | 393.1 |

Controls (the noise floor of R itself):

| Control against R | rms | Cells over 1 m | Relief p50 |
|---|---|---|---|
| R with another jitter seed (two seeds) | **72.4, 73.1 m** | 82 % | 396.4, 397.3 |
| Base + 1 cm per-cell noise | 1.11 m (max 327 m) | 0.5 % | 396.0 |
| Base + 1 m per-cell noise | 13.7 m | 29.8 % | 396.0 |
| R with re-route every 2 steps (not 5) | 56.5 m | 77 % | 420.5 |
| C 4 rounds against R, seed for seed (3 seeds) | 12.4-13.3 m | 17-18 % | within 1 m of each R |

| Border and sink measures (final surface) | R | C, 4 rounds | F, round 4 |
|---|---|---|---|
| Height step, pairs across a domain border, p50 / p99 | 20.0 / 476 m (same pairs) | 18.9 / 454 m | 26.2 / 479 m |
| Land in pits; depth p50 / p95 | 4.86 %; 141 / 603 m | 4.36 %; 170 / 615 m | 4.33 %; 171 / 615 m |
| Land cells draining across the frozen border, last round | - | 3.3 % | 27-34 % (F's label measure) |

Round-to-round change for F: 21.3, 11.7, 7.3 m rms (rounds 2, 3, 4); the change falls, but the distance to R does not.

- **Answer:**
  - **R is chaotic, so pointwise equality with R is the wrong test.** A different jitter seed moves it by 72 m rms, a 1 m base noise by 14 m. An outer iteration at 4 rounds (12.8 m) is inside that floor and well below the seed noise. It leaves no visible step at domain borders and adds no sinks.
  - F does not get better than one round, so **C is the form to use**: continued surface, the rounds being the carve's re-routes.
  - C with 10 rounds is **not** a domain failure: the whole-window carve with the same re-route rhythm gives the same 57 m and the same relief gain. Re-route frequency is itself an erosion parameter.
  - **A coarse domain cut is not acceptable as it stands:** 105 m rms and a slight seam (border p99 609 m against 524). The domains must come from a routing of the erosion grid, with H0 only selecting regions.

### F5: physical cell areas
Question: how much bias does a uniform `dx^2` put into catchments and river selection?
Method: exact spherical quad area per cell (sum over the face equals the face area to 3e-14); `route_weighted` with weights = area/dx^2 against none, face 2, 3,072 cells.

| Quantity | Value |
|---|---|
| Cell area / dx^2 (dx = R pi/2 / N) | min 0.707, max 1.000, mean 0.849 (a max/min ratio of 1.41) |
| Uniform-area total against the true total | +17.9 % (the mean cell is smaller than dx^2 by 8/(3 pi)) |
| Channel cells (true area >= 50 km^2): unweighted/true - 1 | p50 +17.8 %, p95 +35 %, max +41 % |
| Ten largest catchments, uniform against true | +0.9 % to +29.7 % |
| Channel cells added by the uniform area, share of true channel cells | T = 10 km^2: 14.0 %; 50: 7.3 %; 100: 7.7 %; concentrated in the outer half of the face |
| Weighted outlets total against the sum of true areas | relative error 1.5e-11 |
| Distances, neighbour-to-neighbour against dx (analytic) | centre 1.00; corner axis 0.94, diagonal 0.67; edge middle along the edge 0.71 (slope and `f` biased up to 1.5x) |

- **Answer:** yes, the bias changes which rivers exist (7-14 % of channel cells) and the largest catchments' areas by up to 30 %. It is in **today's bake** too. Receivers did not move; only accumulation did.

### F6: cross-face H0 from the stored field
Question: what does a planet-wide route across cube edges cost when heights come from the stored field bake, and do catchments span faces as one drainage?
Method: elevation read by C1 from `global16384-m9c14` (read only, blocks decoded with the key from the store's index; the production open path refused the store because its key has changed since the store was baked), 3,072 cells a face, one index over the six faces with neighbours across cube edges, one priority flood from the sea, D8 receivers, area.

| Quantity | Value |
|---|---|
| Store read of all faces (decoding 1,536 blocks, 4 threads) | 130.8 s (20-27 s a face) |
| Flood / receivers / accumulation (planet-wide) | 5.3 s / 1.2 s / 0.6 s |
| Peak memory | 2.2 GB (about 1.0 GB read cache) |
| Land cells / cells unreached | 16.7e6 / 0 |
| Catchments / spanning two or more faces | 264,345 / 896, holding 26.6 % of land |
| Largest catchment | 5.88e6 km^2 (faces 0 and 3; F1's single-face truncated largest was 1.12e6) |
| Connectivity of the largest cross-edge catchment | 20,533 sampled cells, all reach its one outlet |

- Correction (2026-11-08): the store is older than the code (its base differs by a mean of 1,688 m), so F6's routing cost stands but its catchment sizes are not the current base's. On the current base the largest cross-edge river is about 9.1e5 km^2 (`spikes/prototype-region/results/`); 5.9e6 km^2 stays as a conservative upper bound.
- **Answer:** a planet-wide cross-face route takes about 7 s at 3 km once heights are in memory. The largest catchment is 5.9e6 km^2. At 814 m that is about 8.9e6 cells, about 0.6 GB at pass 1's 64 bytes a cell: still a domain that fits a background job.
- L1 preview: heights stay resident, so re-routing is about 7 s (the read is paid once, and the 131 s is decode-bound, not an edit cost).

## 13. The revised plan

### 13.1 Option A, conditional

**A is the preferred route for the prototype, not yet the approved route for production.** A means: evolve the application and replace the terrain architecture where the target needs it, not preserve the existing stage model. The prototype is built against the target (A3), reusing kernels and infrastructure where they fit, and it does not bend the target to the old stage model.

Criteria to switch to B (any one, measured at the A/B gate; B keeps the prototype as its v0):
| # | Criterion | Measured by |
|---|---|---|
| S1 | The domain solver fails a correctness gate of §13.3 (border seam, conservation, connectivity, divide flux) in every outer-iteration setting tried | prototype |
| S2 | The target cannot be expressed through stage keys (ADR 0006): a second, separate dependency or version scheme is needed beside the keys | prototype; a review of A3 |
| S3 | Graph, domain or edit data cannot live in the pack store (ADR 0019), or the store gate fails with field narrowing done | prototype |
| S4 | The authoritative sampler cannot be `HeightField` with its bound and batch rules (GEN-020, GEN-102): the runtime needs another field interface | prototype |
| S5 | The re-estimate at the gate puts A's remaining work at or above B's remaining work | gate review |

Evidence on 90 % of the code: the claim is withdrawn as an argument. Kept is the infrastructure (crates below `generators`, keys, store, render, editor, kernels). Replaced in any case: the layer order, the simulation unit, the hydrology representation, invalidation.

### 13.2 Revised steps and effort (estimates, lane-days)

| Step | What | Lane | Effort | Note |
|---|---|---|---|---|
| Freeze | The M9 planet bake frozen; no tuning on the old bake; old code and stores stay as the reference | planner | - | D2 |
| A3 | Target architecture and data contracts (§13.4) | planner, spikes | 5-8 | before any stage-model change; external review round (§14) |
| A2 | Turned-lattice noise (bands and creases) | crusader | 3-5 | unchanged; independent of the stage model |
| A12 | Physical cell metric: per-cell area and neighbour distance in `Grid`, route, carve, H0, H1 | explorer | 3-5 | new (F5) |
| P | Prototype v2 (§13.3), including surface identity (was A1), H0 cross-face, domains, hydrology, the edit cases | all | 30-45 | was 20-34 |
| Gate | The A/B gate against §13.1 and §13.3 | planner, owner | 1-2 | failures of architecture gates go to the owner |
| A4-A10 planet-wide | Under A, after the gate (the prototype covers part of each) | all | 40-70 | re-estimated at the gate |
| A11 | Scale as a parameter | spikes, lanes | 6-10 | unchanged |
| | **Total A** | | **88-145** | was 68-113; the ranges of A and B overlap and neither is measured |

- After the gate, B's remaining work (B3-B10 with the prototype as v0) is about 60-100 lane-days (estimate): the difference between A and B at the gate is small, so the gate decides on correctness and fit, not on cost.
- Dates (estimates): A3 and the prototype end about mid-December; the planet-wide work and a new M9 bake about late January 2027.

### 13.3 Prototype v2

**Scope:** §7.1 unchanged (500 x 500 km region, cube-edge window) and:
- H0 planet-wide across cube edges, from the stored field (F6's route), with true cell areas (A12);
- domains cut by a routing of the erosion grid within H0's regions (F4: a 3 km cut alone is not good enough);
- the outer iteration as continued rounds (C), 4 rounds of 5 steps, the round count a parameter of the `SimulationProfile`;
- the channel graph and the diffuse field kept apart; basin and segment identity by rule (§13.4);
- sediment carried across domain borders.

**Edit cases** (all as items below erosion, then undo):
| # | Edit | Tests |
|---|---|---|
| E1 | Raise a 50 x 50 km part of the range by 1,000 m | the base case |
| E2 | Raise a ridge across a major divide so that two catchments trade area | divide movement, domain re-cut |
| E3 | Lower a dam or sill so two basins merge or a basin gains an outlet; and the reverse (a split) | basin identity, spill |
| E4 | An upstream edit that changes a major river's discharge far downstream | downstream propagation |
| E5 | A rainfall edit with no height change that moves river initiation | driver path, thresholds |
| E6 | A lithology or uplift region | driver path into the carve |

**Pass and fail** (numbers proposed; the Earth comparisons of §7.3 stay as reported values):
| Area | Measure | Pass |
|---|---|---|
| Surface | runtime minus bake surface at lattice nodes | 0 before quantisation, at most 0.25 m after |
| Domains | border step: p99 of height differences across domain-border pairs, against the whole-window carve at the same pairs | at most 1.1x |
| Domains | flow crossing frozen borders in the last round (share of land in an unconstrained routing of the final surface) | at most 5 % |
| Domains | distance to the whole-window carve | within its noise floor: rms at most the 1 m-noise control (about 14 m on F4's window), relief p50 within 2 % |
| Iteration | rounds 1, 2, 3, 4, 6 | the rms against R and the round-to-round change fall or stabilise; oscillation over 10 % fails. A plateau inside the noise floor passes |
| Connectivity | segments and channel cells | 0 disconnected segments at every sampling; every channel cell has a downhill path to its segment's outlet |
| Conservation | drainage area at the outlets against the sum of true cell areas, also across cube edges | relative error at most 1e-9 |
| Sinks | unclassified sinks | at most 1 % of land (reported beside the connectivity, no longer the main measure) |
| Basins | E3 | merge, split, spill and outlet migration give the ids and parents the rule says |
| Sediment | per domain and across borders | eroded = deposited + exported to 1e-9 relative |
| Incremental | E1 to E6 | incremental equals a from-scratch build of the edited definition, bit for bit; undo returns the original key set |
| Correctness beside equality | E1 to E6 | the connectivity, conservation and border gates hold after each edit (equal runs can share an error) |
| Latency | L1 with resident heights | about 10 s (F6: 7 s planet-wide at 3 km); L2 of a domain set: report, target 10 min on terrain's share |
| Store | actual serialized data of the region (packs, indexes, graph, ids, versions, edit overrides), extrapolated | at most 35 GB at Earth, hard limit 40 GB; peak temporary storage during an L2 and a planet-wide re-bake reported separately |
| Runtime | flight in the region | no held-back tiles; frame within BUDG-001/002 |
| Determinism | 1 and 8 threads, Windows and Linux, edit replay | bit-equal |
| Scale | the design table of A3; one 2x-radius H0 | H0 cost and memory as F6 x 4 (within 1.5x); area conservation |

Effort: 30-45 lane-days, three lanes, about 3.5-5 weeks. Carry-over as §7.4.

### 13.4 The target-architecture document (A3): table of contents and data contracts

| § | Content |
|---|---|
| 1 | Requirements R1-R8 and non-goals (from §1) |
| 2 | Components K1-K11 (§6.1), with the corrections below |
| 3 | **The authoritative surface:** the sampler's composition in order (base read, regional change, local change, coast fade, creases), interpolation at each step, quantisation at each boundary, the base-identity key each layer carries |
| 4 | **Grids and metrics:** per-cell area and neighbour distance from the mapping; scale as target cell sizes; the design-level scaling table (terrain, hydrology, storage, climate) for moon, Earth, 2x Earth |
| 5 | **Hydrology model:** the channel graph (one receiver per channel cell, segments, confluences) against the diffuse field (MFD area raster); river initiation; lakes; segment and basin identity |
| 6 | **The domain solver:** H0 regions, domain cut, outer iteration, boundary conditions, what a border guarantees |
| 7 | **Edits and dependencies:** the layer order (below or above erosion), keys and reach along the graph, what ends propagation, transactions per domain |
| 8 | **Simulation profile:** duration, rounds, parameters, quality level; every solve restarts from the definition |
| 9 | **Determinism and canonical forms:** supported platforms, canonical stored forms of interface values, f64 versus integer decisions, thread independence, edit replay, no GPU feedback |
| 10 | **Store and budget:** record kinds, keys, the 40 GB gate, temporary storage |
| 11 | **Runtime and previews:** preview levels per edit kind, frame safety |
| 12 | **Validation suite:** the fixtures and gates of §13.3 |
| 13 | ADR list: new ADRs and amendments (0007, 0066, 0088, 0099-0101, 0105, 0109; CON-08 reading) |

Draft contracts, the questions the reviewers asked:
- **What is iterated:** a continued surface. Each round routes the current surface, cuts domains, carves `n` steps per domain, assembles; the round is the carve's re-route (F4).
- **Stopping rule:** a fixed round count in the `SimulationProfile`, part of the keys. No convergence test inside the build, because a test would make the result depend on a threshold and the carve is chaotic (F4). Convergence is checked by the prototype and the scorecard.
- **Without stabilisation:** the count is chosen from the measured plateau. A failure of the correctness gates at every count is S1.
- **What a border guarantees:** no flow crosses it inside a round; the walls move between rounds. It is not a physical divide but a numerical partition. The gates measure its error.
- **Channel geometry:** an input of the local refinement (K7 carves along the segment), so the surface has one elevation source. Below 102 m, a declared bound lowers the ground only (like creases, GEN-109).
- **Basin identity:** a basin's id is derived from a canonical cell of the basin (its lowest cell, ties by cell id). A merged basin keeps the id of the deeper part and records the other as a parent; a split produces new ids with the old id as parent; an outlet migration keeps the id and updates the outlet.
- **Segment identity:** a segment's id is derived from its downstream confluence cell and its upstream head cell; a changed confluence yields new segments with parent links. Stable ids at river scale need the prototype's evidence (E2, E3).
- **Propagation end:** a domain is recomputed when its input key changes; the interface values (outlet level, inflow area, inflow sediment) are quantised to a stated precision that the prototype measures. A threshold crossing or a spill inside a domain is a change of its own output; a structural change at H0 re-cuts the domains.
- **Canonical stored forms:** every value a later stage reads has a canonical stored form (f32 heights, i16 changes, ...); from-scratch runs read it, not an in-memory f64 (determinism of re-solves).

### 13.5 Owner decisions, revised

**D1. Route (changed).**
- Context: §13.1; the second review; the owner's direction: evolve.
- Options:
  - a. A, conditional: prototype against the target, with the switch criteria S1-S5 at the A/B gate.
  - b. A unconditional.
  - c. B now.
- Trade-off: a keeps the option to change course at the cost of the gate; b commits before the domain solver is shown; c spends the parallel core before any evidence that it is needed.
- **Recommendation: a.**

**D2. The M9 planet bake (changed: firmer).**
- Options:
  - a. Freeze the bake and all tuning on the old representation; old code and stores stay as a reference; D2/D3/D4 of M9.1 wait for the gate.
  - b. Delay only.
  - c. Bake now.
- Trade-off: a saves wasted rounds and gives up a whole-planet look for now.
- **Recommendation: a.** Surface identity moves into the prototype (no polish of the old bake).

**D3. Watershed domains as the erosion unit (changed: a hypothesis to test).**
- Context: F2, F4, the second review. The carve is chaotic (a jitter seed moves it by 72 m rms), so the test is structural: border step, border flux, conservation, connectivity, the noise floor.
- Options:
  - a. Test continued rounds (4 x 5 steps) with domains from the erosion-grid routing in the prototype; a production decision only at the gate.
  - b. Accept domains now.
  - c. Drop domains and stay with a per-face carve.
- Trade-off: a costs nothing extra; b is premature; c has no regional re-solve.
- **Recommendation: a.**

**D4. Edits below erosion (unchanged).** The second review agrees.

**D5. Persistent rivers (changed: configurable).**
- Options:
  - a. Persistent segments with a configurable threshold (default 50-100 km^2 of true area), vertices on the regional graph, validated by connectivity gates.
  - b. Derived only.
- **Recommendation: a.** The threshold and its area are in true area (F5).

**D6. Store (changed: a hard measured gate).**
- Options:
  - a. Narrow the field bake first; the 40 GB limit is a measured gate on real serialized data, including indexes and versions; temporary storage measured separately; 200 m pass 2 a fallback only.
  - b. Take 200 m now.
- **Recommendation: a.**

**D7. Scale now (changed: broader).** Options: a. per-body cell sizes, the preset, and a design-level scaling table (terrain, hydrology, storage, climate) in A3, one 2x-radius H0 in the prototype; b. Earth only. **Recommendation: a.**

**D8. Owner attention (changed).**
- Options:
  - a. The planner judges numeric gates; failures of **architecture or requirement** gates, and the A/B gate, come to the owner, who reviews the prototype once.
  - b. Owner at every step.
- **Recommendation: a.**

**D9. Physical cell metric (new).**
- Context: F5. A uniform cell area overstates drainage area by 18 % on average and up to 30 % for the largest catchments, and changes 7-14 % of channel cells. The bias sits in today's bake.
- Options:
  - a. True area and distance in the routing, the carve, H0 and H1 from the prototype on, in the kernels (A12).
  - b. In the new hydrology only, leaving the carve uniform.
  - c. Defer.
- Trade-off: a re-tunes erosion constants (the areas shrink by about 15 %, so `K dt` moves); b keeps tuning but carves and routes on different metrics; c builds on the bias.
- **Recommendation: a.**

**D10. What "correct" means for a chaotic carve (new).**
- Context: F4. Pointwise comparison with a reference carve is meaningless beyond its noise floor.
- Options:
  - a. Correctness is structural (seam, flux, conservation, connectivity, relief within the noise floor) plus bit equality for reproducibility.
  - b. Require closeness to a single reference run.
- Trade-off: b would fail every design, including the whole-window carve against itself with another seed.
- **Recommendation: a.**

### 13.6 The next two weeks (if D1-D10 go as recommended)

| Week | Explorer | Crusader | Spikes |
|---|---|---|---|
| 1 | A12: true cell metric in `Grid`, routing and carve (with a tuning note); H0 across cube edges on the stored field | A2: turned-lattice noise; #1137 (C1 default) | A3 with the planner: target and contracts (§13.4), ready for the external round; the region search and the validation fixtures |
| 2 | prototype: domains from an erosion-grid routing within H0 regions, continued rounds, border gates | prototype: sampler composition (K8), lakes on the water field | measuring the gates as they land; edit-case fixtures E1-E6 |

Owner attention: D1-D10 once; the A3 ADRs; nothing else unless an architecture gate fails.

## 14. Another external review round?

**Recommendation: yes, one more round, on the A3 target document, before the prototype is built against it. A further round only if the A/B gate is close.**

| Round | Value | Cost | Advice |
|---|---|---|---|
| On A3 (the target and data contracts) | high: both reviews found design gaps that the code mapping did not (the F2 reading, basin identity, channel geometry as a second elevation source). A3 is where such gaps are cheapest to fix, before lanes build on it | the owner forwards one document and pastes one answer; the planner folds it in; about half a day, no lane stops | **do it** |
| On the prototype results at the A/B gate | moderate: the gates are numeric and the reviewer lacks the code; useful if the result sits on a switch criterion | as above, plus the measurement tables | **only if close** |
| Now, on this phase 3 | low: the same points would return; nothing new is built yet | | **no** |

What to give the reviewer for A3: the A3 document itself; §8 and §12 of this file (the measurements, with the noise-floor controls); the current code of `bake/carve.rs` and `bake/route.rs` as excerpts, so the review of the solver can be checked against the real kernels (the two reviews so far did not see code). Questions to ask:
1. Is the outer iteration's contract (continued rounds, fixed count, a wall between domains) sound for a chaotic carve, or is another domain strategy better?
2. Are the basin and segment identity rules stable enough for gameplay references and for undo?
3. Are the gates of §13.3 sufficient to distinguish a sound domain solver from a reproducible but wrong one?
4. Is anything in the sampler's composition or the canonical forms inconsistent with deterministic re-solves?

# Phase 4 (inbox #101, 2026-11-08): the external review of A3

Input: a third external review (owner refs, 2026-11-08, outside git; "the third review"), of `docs/design/terrain-target.md`. Verdict of the review: the feedback is incorporated; A3 is ready for a constrained prototype after three amendments; it is not an approved production architecture. The reviewer did not receive the kernels (the brief named them; they were not attached), so the points are checked against the code here.

## 15. Verdict on each point

| Point | Verdict | Reason, references |
|---|---|---|
| The incremental routing contract is circular: the new catchments are needed to know what to re-route (§6.5) | **adopt** | Correct. §6.5's "runs only over the union of dirty catchments" named the union without saying how it is found. Amended in A3 §6.5 (four approaches compared, one chosen) and the catchment-round key defined exactly |
| Routing is 85-90 % of a round, so the main cost of an edit stays global | **adopt** | F7's measure. The design must either accept region-wide routing or give a method that is not an assumption; A3 §6.5 and F9 |
| H1 memory: 0.6 GB is a catchment; the round routes a landmass (tens of GB at 64 B a cell) | **adopt** | Correct; §6.2 and §6.5 mixed the two. F8 measures the landmass route. A3 §6.6 states the memory model |
| The H0-split fallback reintroduces the boundary problem | **adopt** | H0 divides are not H1 divides. Replaced by a partitioned route of the same landmass (no artificial border); the split stays as a tested fallback in G-SCALE |
| MFD area as channel discharge; a running maximum is not conservation; a downstream receiver can have a lower MFD value | **adopt** | Correct. A3 §5.2 is rewritten: diffuse area, initiation and channel discharge are separate; the running maximum is dropped; discharge accumulates along the explicit graph |
| F7 did not test the clipped MFD and the canonical flood | **already covered** | They are gates (A3 §6.4, G-LOCAL, G-ROUND); wording tightened: the reference uses the same rules |
| Kernels not reviewed | **note** | The brief pointed at files the reviewer was not given. A later round must attach them; this round's code claims are checked here |
| Endorheic water level: "water balance" hides a model | **adopt** | A3 §5.5 states the steady water-balance model |
| Topographic spill point against an active outlet | **adopt** | A3 §5.3 separates them |
| Channel continues downstream after initiation | **adopt** | A3 §5.2 |
| Which H0 outputs constrain H1 | **adopt** | A3 §5.1 lists them |
| Supported platforms | **already covered**, stated plainly | A3 §9: Windows and Linux x86-64 |
| Store: key table, edit overrides, temporary re-bake space | **adopt** | A3 §10 |
| Preview usefulness for lithology and uplift | **adopt** | A3 §11.1 adds a coarse carve as their level 1 |
| G-EQ must look beyond relief (flood topology, initiation) | **adopt** | A3 §12 |
| G-CON (downstream continuation, confluence discharge) | **adopt** | A3 §12 |
| G-LOCAL, G-ROUND on the new kernels | **adopt** | A3 §12 |
| G-INC: routing work and dirty-set discovery | **adopt** | A3 §12, on E2 |
| G-SCALE: landmass memory and the split test | **adopt** | A3 §12 |
| All structural checks after the edits | **adopt** | A3 §12: stated as a hard rule |

## 16. Probes of phase 4

Sonnet sub-agents, seed 14 on the current base, 4 threads, shared machine (timings indicative). Probe code on `spike/terrain-pipeline-review` (never merged).

### F8: a landmass route at H1
Question: memory and time of one resident routing of the largest landmass at the regional grid. Method: `f8_landmass_route.rs`; one index over the landmass cells across cube edges, real `BakeInputs::base`, the arrays of `route.rs` at 32 bytes a cell.

| Quantity | Value |
|---|---|
| Largest landmass | 9.3e6 cells at 3.26 km, 8.42e7 km^2 true area, 1.49e8 cells at 814 m (four faces) |
| Peak memory, route | 5.1 GiB (32 bytes a cell + index 1.1 bytes) |
| Flood / receivers / MFD | 55 s / 6.5 s / 77 s, single-threaded |
| Base evaluation (not needed in production) | 783 s |

Reading: a resident routing of a landmass fits the 16 GB machine but not the editor's 4 GB terrain share. A round adds the carve's arrays; pass 1's measured 64 bytes a cell already includes the routing, so a round is about 10 GB (the probe's 14 GB estimate double-counts), about today's pass 1.

### F9: dirty tiles after an E2-type edit
Question: what share of 256-cell tiles changes in level, receiver or area after a divide-moving edit? Method: `f9_dirty_tiles.rs`, face 1, 3,072 x 3,072 cells at 814 m, a ridge line raised by 200 m along E2's divide (`spikes/prototype-region/results/`); whole-window routing before and after, diffed per tile; `flood_labelled` and `SpillGraph` run per tile.

| Change against before | First routing, tiles (of 144) | After one carve round, tiles | Cells |
|---|---|---|---|
| Level | 2 | 4 | 0.02 %, 0.23 % |
| Receiver | 2 | 4 | 0.01 %, 0.02 % |
| Area (relative over 1e-9) | 4 | 4 | 0.09 %, 0.20 % |
| Catchments with changed membership | 2 of 20,045 | 7 of 19,486 | |
| Tile flood output | 2 | 4 | |
| Border graph | 146,881 nodes, 17 ms | same | |

Tiles plus border graph reproduce the whole-window flood exactly (0 cells differ, four surfaces). The 144 tile floods take 0.22 s (4 threads), one whole-window flood 1.0 s.

- **Answer:** the changed set of a divide-moving edit is about 3 % of tiles and is found by flooding only the changed tiles and solving the graph. A larger edit (E4) can touch more tiles downstream along the river; G-INC reports it.
