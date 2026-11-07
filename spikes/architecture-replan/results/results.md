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
