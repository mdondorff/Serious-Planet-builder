# Architecture decision records

One file per decision: `NNNN-kebab-title.md`, created with `/adr`. Claude writes new ADRs as `proposed`; only the owner sets `accepted`. Accepted ADRs are never edited except for their status line.

Decide-now ADRs to draft in M0 (from background report §12): coordinate frames & precision; depth; planet topology & tile addressing; language & graphics layer; development environment & workflow; generator purity, versioning & cache keys; authored-layer data model; app structure; testing architecture & render boundary; cross-platform numeric determinism; reference surface; spec tooling.

## Status index

Refreshed by the `adr` skill whenever an ADR is added or its status changes, and by the `housekeeping` skill. The status line in each file is authoritative.

| ADR | Title | Status | Open points (trigger) |
|---|---|---|---|
| 0001 | [Coordinate frames and precision](0001-coordinate-frames-and-precision.md) | accepted |  |
| 0002 | [Depth buffer](0002-depth.md) | accepted |  |
| 0003 | [Planet topology and tile addressing](0003-planet-topology-and-tile-addressing.md) | accepted |  |
| 0004 | [Language and graphics layer](0004-language-and-graphics-layer.md) | accepted |  |
| 0005 | [Development environment and workflow](0005-development-environment-and-workflow.md) | accepted |  |
| 0006 | [Generator purity, versioning and cache keys](0006-generator-purity-versioning-and-cache-keys.md) | accepted |  |
| 0007 | [Authored-layer data model](0007-authored-layer-data-model.md) | accepted | amended by ADR 0066 (invalidation unit, command log) |
| 0008 | [App structure](0008-app-structure.md) | accepted |  |
| 0009 | [Testing architecture and render boundary](0009-testing-architecture-and-render-boundary.md) | accepted |  |
| 0010 | [Cross-platform numeric determinism](0010-cross-platform-numeric-determinism.md) | accepted |  |
| 0011 | [Reference surface](0011-reference-surface.md) | accepted |  |
| 0012 | [Spec tooling](0012-spec-tooling.md) | accepted |  |
| 0013 | [Render graph and GPU-driven baseline](0013-render-graph-and-gpu-driven-baseline.md) | accepted |  |
| 0014 | [Terrain LOD algorithm](0014-terrain-lod-algorithm.md) | accepted |  |
| 0015 | [Window surface and UI painting live in render](0015-window-surface-and-ui-painting-in-render.md) | accepted |  |
| 0016 | [Height-aware terrain selection and morph](0016-height-aware-terrain-selection.md) | accepted; amended or extended by 0088/0089 (2026-11-01) |  |
| 0017 | [Golden review policy and agent blessing](0017-golden-review-policy.md) | accepted | cross-adapter agreement amended by 0061; Auto-bless R extended by 0096 |
| 0018 | [Persistent cache keys: 128 bits from BLAKE3 over a canonical encoding](0018-persistent-cache-keys.md) | accepted |  |
| 0019 | [Tile storage: an append-only pack with a rebuildable index, Hilbert-ordered](0019-tile-storage-pack-format.md) | accepted |  |
| 0020 | [Autonomous performance runs behind a measured readiness gate](0020-autonomous-perf-runs.md) | accepted |  |
| 0021 | [Authored-item index: extent, layers, widths, staging](0021-authored-item-index.md) | accepted | points 2, 5, 6, 8 decided by ADR 0066 |
| 0022 | [Stage reads, scopes and halos](0022-stage-reads-and-halos.md) | accepted | option 2 needs the owner's CON-08 reading (trigger: a chain with same-stage reads) |
| 0023 | [Clarifications to the persistent key encoding (ADR 0018)](0023-persistent-key-clarifications.md) | accepted |  |
| 0024 | [Clarifications to the tile pack format (ADR 0019)](0024-tile-pack-clarifications.md) | accepted |  |
| 0025 | [Tile payload codec, node set id and what cache-verify finds](0025-tile-codec-and-cache-verify.md) | accepted |  |
| 0026 | [Global-coarse field representation and bake layout](0026-global-coarse-representation.md) | accepted; amended or extended by 0088/0089 (2026-11-01) |  |
| 0027 | [Reading the global-coarse bake, and the canonical block layout](0027-reading-the-global-bake.md) | superseded by 0055 (content in specs) |  |
| 0028 | [The terrain field: a pure function of the direction over the global bake](0028-terrain-field-from-the-global-bake.md) | accepted; amended or extended by 0088/0089 (2026-11-01) |  |
| 0029 | [Compaction and the size cap: write then delete, an injectable eviction rank](0029-compaction-and-the-size-cap.md) | accepted |  |
| 0030 | [Erosion strategy: local filter in M3, a coarse global stream-power stage after it](0030-erosion-strategy-hybrid.md) | accepted; amended or extended by 0088/0089 (2026-11-01) |  |
| 0031 | [Residency set and request priorities](0031-residency-set-and-request-priorities.md) | accepted |  |
| 0032 | [Streaming test definitions: dive, teleport, mouse look](0032-streaming-test-definitions.md) | accepted |  |
| 0033 | [Memory budgets and terrain's VRAM share](0033-memory-budgets-and-terrain-vram-share.md) | accepted | point 3 amended by 0054; points 3-5 by 0062 and 0063; point 9 by 0064 |
| 0034 | [GPU clock state in performance measurements](0034-gpu-clock-state-in-perf-measurements.md) | accepted with a change |  |
| 0035 | [CPU/GPU equality applies to fields, not to presentation data](0035-cpu-gpu-equality-applies-to-fields.md) | accepted |  |
| 0036 | [Perf validity at full GPU load: power cap and clock band](0036-perf-validity-at-full-gpu-load.md) | accepted in part | point 3 (GPU noise floor): first M5 perf window with GPU verdicts |
| 0037 | [M5: new goldens under policy, look reviewed at the gate](0037-m5-new-goldens-and-look-review.md) | accepted | point 1 superseded by 0061 |
| 0038 | [HDR target, exposure and tone mapping](0038-hdr-exposure-and-tone-mapping.md) | accepted | accepted when the M5 queue starts |
| 0039 | [Sun lighting and terrain base shading](0039-sun-lighting-and-terrain-base-shading.md) | accepted | climate storage amended by 0062 |
| 0040 | [Atmosphere: Hillaire 2020 LUTs, CPU reference, and CON-11](0040-atmosphere-model-and-cpu-reference.md) | accepted |  |
| 0041 | [Water: level field, ocean connectivity, Gerstner waves, shoreline](0041-water-level-field-ocean-and-shoreline.md) | accepted |  |
| 0042 | [M5 frame structure, look data and the viewer window](0042-m5-frame-structure-and-viewer-window.md) | accepted | accepted when the M5 queue starts |
| 0043 | [CON-11: geometry stays strict, presentation shading is judged by a tested bound](0043-con-11-geometry-strict-shading-by-bound.md) | accepted |  |
| 0044 | [M4 implementation readings of ADR 0031 (cancellation) and ADR 0032 (dive pitch)](0044-m4-implementation-readings-of-0031-and-0032.md) | superseded by 0055 (content in specs) |  |
| 0045 | [Bodies, bake resolution and the storage budget](0045-bodies-bake-resolution-and-storage-budget.md) | proposed | all but point 3: multi-body work (point 7 before the editor ships) |
| 0046 | [Generation workers run below normal priority](0046-generation-workers-run-below-normal-priority.md) | accepted |  |
| 0047 | [Adapter gate: a discrete GPU, chosen before the device exists](0047-adapter-gate.md) | accepted |  |
| 0048 | [Frame-target share (amends ADR 0033's remainder) and the lit path's readings of ADR 0038 and ADR 0042](0048-frame-target-share-and-lit-path-readings.md) | superseded by 0055 (content in specs) |  |
| 0049 | [The climate field's bound and storage (amends ADR 0039 point 2), and the look file's version rule](0049-climate-field-bound-and-storage.md) | accepted | climate storage amended by 0062 |
| 0050 | [Atmosphere: implementation readings of ADR 0040](0050-atmosphere-implementation-readings.md) | superseded by 0055 (content in specs) |  |
| 0051 | [Ocean channel, liquid table and the water-level field: implementation readings of ADR 0041](0051-ocean-channel-liquid-table-and-water-field-readings.md) | superseded by 0055 (content in specs) |  |
| 0052 | [First M5 goldens: an edge tolerance for the cross-adapter debug-view check](0052-m5-debug-view-edge-tolerance.md) | superseded by 0061 |  |
| 0053 | [The water pass: implementation readings of ADR 0041 points 4 to 9](0053-water-pass-readings-of-adr-0041.md) | superseded by 0055 (content in specs) | |
| 0054 | [Compact tiles in storage buffers instead of texture atlases (amends ADR 0033 point 3)](0054-compact-tiles-in-storage-buffers.md) | accepted | slot layout amended by 0062 |
| 0055 | [Documentation model: specs hold current rules, ADRs record why](0055-documentation-model.md) | accepted | point 5 amended by 0075; point 6 consolidation open except its point 4 (M7 queue H2) |
| 0056 | [Water selection: the coastal band, tile water in the tile job, a pixel-footprint wave fade](0056-coastal-band-water-selection.md) | accepted | amends 0053 points 4 and 5 |
| 0057 | [GPU figures below the clock floor: a cool start, and a pass is a pass](0057-gpu-clock-below-the-floor.md) | accepted | amends 0036 point 2; point 1 amended by 0094 |
| 0058 | [A coarse level of the water field: a summary the bake writes once](0058-coarse-level-of-the-water-field.md) | accepted | amends 0053 points 4, 5 and 0056 points 1, 4 for coarse tiles; for the owner at the M5 gate |
| 0059 | [Noise floors: 0.1 ms per pass, 0.5 ms per frame total](0059-noise-floors.md) | accepted | amends 0036 point 3; for the owner at the M5 gate |
| 0060 | [A power-capped P0 clock is valid; coast-3m water budget 2.2 ms](0060-power-capped-clock-and-coast-3m-water.md) | accepted (owner) | amends 0036 point 2 and 0057 point 2 |
| 0061 | [Goldens after M5: the M5 window closes, goldens are per-adapter regression checks](0061-goldens-after-m5.md) | accepted (owner) | supersedes 0037 point 1 and 0052; amends 0017 |
| 0062 | [Retire the vertex tile format from rendering](0062-retire-the-vertex-tile-format.md) | accepted (owner) | first work of M6; point 4 amended by 0063 |
| 0063 | [Slot v2 without the 5x5 reserve: 9,168 B](0063-slot-v2-size-correction.md) | accepted | corrects 0062 point 4; for the owner at the M5 gate |
| 0064 | [The render floor's roots load outside the per-frame upload cap](0064-floor-roots-exempt-from-the-upload-cap.md) | accepted | amends 0033 point 9 |
| 0065 | [CPU calibration decides whether a perf scene counts](0065-cpu-calibration-validity.md) | accepted | extends 0020 and 0036 |
| 0066 | [Authored items: reach, region summaries, stages, selection bounds, item files, command log](0066-authored-items-reach-summaries-stages-and-files.md) | accepted; amended or extended by 0088/0089 (2026-11-01) | decides 0021 points 2, 5, 6, 8; amends 0007, 0018, 0023 |
| 0067 | [Stamp key hash and height bounds by delta range](0067-stamp-key-hash-and-delta-range-bounds.md) | accepted | amends 0066 points 2, 5 and the key role of 6 |
| 0068 | [An authored stage with an empty footprint passes its source key through](0068-empty-authored-stage-passes-the-key-through.md) | accepted | amends 0066 point 4 |
| 0069 | [Lines and regions are point-list items; a long line splits into linked items](0069-lines-and-regions-are-point-list-items.md) | accepted | extends 0066 point 6 and 0067; mode 1 dormant until Cities (soft influence, owner Q8) |
| 0070 | [Brush strokes stay in the item file; the blob store is specified and deferred](0070-brush-strokes-stay-in-the-item-file-no-blob-store-in-the-mvp.md) | accepted | extends 0066 point 6 and 0069; decides the blob ADR of 0007 |
| 0071 | [Coastal water by connectivity through low ground, not by a reach](0071-coastal-water-by-connectivity-through-band-ground.md) | accepted | amends 0056 points 1 to 3 and the coarse rule of 0058 |
| 0072 | [Edit visible: the drawn tiles are exact, drawn tiles refresh first, stale refreshes are pre-empted](0072-edit-visible-drawn-tiles-exact-and-stale-refreshes-are-pre-empted.md) | accepted | reads the M6 criterion; extends STRM-026 |
| 0073 | [Normals from the tile's height lattice](0073-normals-from-the-tile-height-lattice.md) | accepted | amends GEN-019 and GEN-010; extends 0033 point 4 |
| 0074 | [A tile too steep to morph is not refined; the plan degrades locally](0074-a-tile-too-steep-to-morph-is-not-refined.md) | accepted | amends LOD-008; extends 0016 and 0014; built (#505, #523, #524) |
| 0075 | [Documentation budget: the hot files stay small and current](0075-documentation-budget.md) | accepted (opus-planner; owner approved) | amends 0055 point 5 |
| 0076 | [Driver maps: physical fields decide materials, plants and shapes through suitability rules](0076-driver-maps-suitability-and-consumers.md) | proposed, open | chunk driver channels leave with the chunk stage; driver maps decided with M9's bake |
| 0077 | [Regional stages run per chunk of a fixed grid; the global coupled run fixes the river network](0077-regional-stages-per-chunk-and-a-global-river-network.md) | accepted (owner) | CON-08 amended to name chunks (constitution v1.6); extends 0022 and 0030; built in M8 |
| 0078 | [Maintenance of accepted ADRs: links, spec-owned values, domain terms, errata](0078-maintenance-of-accepted-adrs.md) | accepted (owner) | amends CON-23 (constitution v1.5); spec-lint check queued |
| 0079 | [The constitution's size cap follows the owner's amendments](0079-constitution-cap-follows-owner-amendments.md) | accepted (owner) | amends 0075 point 1 for the constitution |
| 0080 | [Hill-scale relief: a band in the bake, landform-keyed creases, and a stationary uplift for the coupled run](0080-hill-relief-in-the-bake-landform-creases-and-a-stationary-uplift.md) | accepted; amended or extended by 0088/0089 (2026-11-01) | amends 0030 point 1; reorders 0076 point 6; SPIKE-45 (owner, 2026-10-27) |
| 0081 | [Local main-stem routing inside a chunk](0081-local-main-stem-routing-inside-a-chunk.md) | accepted | departs from 0077 point 3 (refines it: topology and border crossings stay global) |
| 0082 | [The coarse node slides, not the border crossing](0082-the-coarse-node-slides-not-the-border-crossing.md) | accepted | amends 0081 point 1 |
| 0083 | [Runtime compute budgets](0083-runtime-compute-budgets.md) | accepted | new; numbers in a spec |
| 0084 | [A coarse valley carve in the base](0084-coarse-valley-carve-in-the-base.md) | not pursued | SPIKE-49 negative; M9's bake decides valley carving |
| 0085 | [Shadows from a height clipmap march, objects by cascades](0085-shadows-from-a-height-clipmap-march-objects-by-cascades.md) | accepted; points 3, 9 amended by 0090 | replaces the horizon-map shadows (GEN-044/045, REND-038); SPIKE-56 |
| 0086 | [Chunk stage removed](0086-chunk-stage-removed.md) | accepted (owner) | supersedes 0077 (except the global coupled run), 0081, 0082 and 0084 points 3 and 4 |
| 0087 | [Perf verdicts under load, windows only for close calls](0087-perf-verdicts-under-load-windows-only-for-close-calls.md) | accepted (opus-planner, delegated) | amends 0020 in part; built (perf-loaded-mode, BUILD-030 to 033; point 7: perf-window-runner, thresholds in BUILD-034); open: the 0065 limits for quiet sessions; SPIKE-58 |
| 0088 | [The M9 terrain bake](0088-the-m9-terrain-bake.md) | accepted; points 2 and 5 amended by ADR 0098, point 4 by ADRs 0099, 0101, 0102 and 0103, point 5 by ADR 0100 | baked relief driver (from the classes, ADR 0089) and ring mean; the erosion layer in two passes (a carve per face on a coarse grid, then per tile an incise-only local carve, thermal and deposition) with a min/max pyramid; the bake waits for the owner's approval of the engine preview; SPIKE-61 to 83 |
| 0089 | [Relief classes drive the terrain's amplitude](0089-relief-classes-drive-the-terrains-amplitude.md) | accepted | class weights per lattice point as the interface; rule-based default (plate belts, plateaus) now, physical derivation later (option C); amplitude and hill band per class; class paint as an item; drivers, not dictated types; hard constraints after generation; SPIKE-65 to 83 |
| 0090 | [Shadow march: order-independent atomics and a drop coefficient](0090-shadow-march-order-independent-atomics-and-a-drop-coefficient.md) | accepted | amends ADR 0085 points 3 and 9: the gather's integer atomicMax owner pass; the drop as an f32 coefficient 1/(2R) beyond the clipmap |
| 0091 | [The heightfield is judged up to 45 degrees; steeper faces are a separate technology](0091-the-heightfield-is-judged-up-to-45-degrees.md) | accepted | criterion 1's slope counts cells at or below 45°; cliffs and overhangs by a separate technology (own ADR); SPIKE-85 |
| 0092 | [Compute shares are fractions of the P-core capacity; background work is budgeted, not pooled on E-cores](0092-compute-shares-are-fractions-of-the-p-core-capacity.md) | accepted | shares as fractions of the P-core capacity, values in BUDG-003 only; flight is the goal, teleport not; amends ADR 0083 point 3; SPIKE-89, SPIKE-90 |
| 0093 | [The CPU calibration: a robust estimator, read before the frames, voiding frame verdicts only](0093-calibration-estimator-and-scope.md) | accepted (opus-planner, delegated) | amends ADR 0065: minimum of 5 timings, read after the cool-down, GPU verdicts not voided, declared loads recorded; SPIKE-95 |
| 0094 | [Cool-down by scene kind, and a duty idle between runs](0094-cool-down-by-scene-kind-and-a-duty-idle.md) | accepted (opus-planner, delegated) | amends ADR 0057 point 1: no cool start for streaming scenes, a 30 s cap for views, a duty idle between runs; SPIKE-95 |
| 0096 | [Rounding auto-bless covers lit, sky and water views, against the owner's last bless](0096-rounding-auto-bless-covers-lit-sky-and-water.md) | accepted (owner) | amends ADR 0017: Auto-bless R for every class; lit, sky and water also checked against the anchor (last owner bless); tool support pending |
| 0097 | [World build: one day in shares, and a preview before every full run](0097-world-build-a-day-in-shares-preview-first.md) | accepted (owner, 2026-11-03); share values proposed | retires GEN-014's ten-minute limit; BUDG-014; terms world build, field bake, erosion bake |
| 0098 | [Creases only below the erosion layer's grid; the layer owns erosion at its scales](0098-creases-below-the-erosion-layer.md) | accepted (owner, 2026-11-03) | amends ADR 0088 points 2 and 5; fade band about 200-400 m a start value; whether fine creases keep `k` decided on the preview; GEN-109 |
| 0099 | [Erosion layer: pass 1 at 12,288 cells, MFD area, C1 read of the pass-1 change](0099-erosion-layer-pass-1-at-12288-mfd-area-c1-read.md) | accepted (owner, 2026-11-03) | amends ADR 0088 point 4; GEN-100, GEN-101 |
| 0100 | [Runtime read of the erosion layer: C1, clamped, with a coast fade](0100-runtime-read-of-the-erosion-layer.md) | accepted (owner, 2026-11-04) | amends ADR 0088 point 5; GEN-112 to GEN-114 |
| 0101 | [Keep depressions in the erosion layer; the fill depth is a stored channel](0101-keep-depressions-in-the-erosion-layer.md) | accepted (owner, 2026-11-05) | extends ADR 0088 point 4; the channel's code is decided before the planet bake; GEN-120, STRM-076 |
| 0102 | [Lithology: an analytic erodibility K(x, z) drives the erosion carve](0102-lithology-erodibility-in-the-carve.md) | accepted (owner, 2026-11-05) | extends ADR 0088 point 4; values from the preview round; GEN-116 |
| 0103 | [An analytic uplift field replaces the uniform U/K](0103-an-analytic-uplift-field-replaces-the-uniform-u-over-k.md) | accepted (owner, 2026-11-05) | extends ADR 0088 point 4; amends ADR 0102 point 3's wording (the uplift is not scaled by K or the class factor); GEN-123, GEN-125 |
| 0104 | [A relief-aware climate: wind, two-season orographic rain, runoff in the carve, arid class rules](0104-relief-aware-climate-runoff-and-arid-classes.md) | accepted (owner, 2026-11-05) | values from the preview round; older stores refused; GEN-117, GEN-128, GEN-129 |
| 0105 | [Deposition in pass 1 and endorheic sinks in arid basins](0105-deposition-and-endorheic-sinks-in-pass-1.md) | accepted (owner, 2026-11-05) | extends ADR 0088 point 4 and ADR 0101 |
| 0106 | [Minimal glacial erosion in pass 1, behind a switch and a gate](0106-minimal-glacial-erosion-behind-a-gate.md) | accepted (owner, 2026-11-05) | extends ADR 0088 point 4; gate against Sognefjord, Highlands, Alps; cirques and moraines deferred |
| 0107 | [Coasts from wave erosion and sediment supply in the bake, behind a switch and a gate](0107-coastal-erosion-in-the-bake-behind-a-gate.md) | accepted (owner, 2026-11-05) | extends ADR 0088 point 4; gate against a cliffed and a depositional tile, fallback a rule-based coast type; cliff technology by its own ADR |
| 0108 | [Earth is the default preset, not the model: laws in kernels, planet constants as named parameters](0108-earth-is-the-default-preset.md) | accepted (owner, 2026-11-05) | audit of values on main (SPIKE-103); off-Earth scorecard presets (TEST-094) |
| 0109 | [Basin infill of the base surface before the carve; what remains is a lake basin](0109-basin-infill-before-the-carve.md) | accepted (owner, 2026-11-07) | amends ADR 0101 and ADR 0105 point 1; thresholds tuned at I4b |
| 0110 | [The authoritative surface: one composition, one base read, one elevation source](0110-the-authoritative-surface.md) | accepted for the prototype, with conditions (owner, 2026-11-11) | A3 target; amends ADR 0088, 0099, 0100, 0101 |
| 0111 | [Hydrology model: H0 and H1, channel graph against diffuse field, basin and segment identity](0111-hydrology-model-and-identity.md) | accepted for the prototype, with conditions (owner, 2026-11-11) | A3 target; extends ADR 0041, amends ADR 0101, 0105 |
| 0112 | [The erosion solver: regions, rounds and catchments](0112-erosion-solver-regions-rounds-catchments.md) | accepted for the prototype, with conditions (owner, 2026-11-11) | A3 target; amends ADR 0088, 0099; replaces ADR 0109 and 0105 point 1 on the gate |
| 0113 | [Edit ordering and dependencies: landform and driver edits below erosion](0113-edit-ordering-and-dependencies.md) | accepted for the prototype, with conditions (owner, 2026-11-11) | A3 target; amends ADR 0007, 0066, 0089; settles ADR 0086 point 3 |
| 0114 | [Simulation profile, cell metric and canonical forms](0114-simulation-profile-cell-metric-and-canonical-forms.md) | accepted for the prototype, with conditions (owner, 2026-11-11) | A3 target; extends ADR 0010 |
| 0115 | [Scale as a parameter and the store budget](0115-scale-as-a-parameter-and-the-store-budget.md) | accepted for the prototype, with conditions (owner, 2026-11-11) | A3 target; supersedes ADR 0045 point 2 |
| 0116 | [Constitution v1.7: rules for the simulation architecture](0116-constitution-v1-7-simulation-architecture.md) | accepted (owner, 2026-11-11) | CON-05 to CON-08, CON-14 refined; CON-28 to CON-31 added; constitution cap 12,817 B |
| 0117 | [Driver regions are a world-level table, not tile items](0117-driver-regions-are-a-world-level-table-not-tile-items.md) | proposed | the architect decides, then the owner for the schema version (CON-10) |
| 0118 | [Decisions through one file; new generator stages land switched off; test guards and tolerances](0118-decision-channel-default-off-stages-and-test-guards.md) | accepted (owner, 2026-11-12) | process; `docs/routing.md` rules 6-9 |
| 0119 | [K7's tile grid and halo are part of its definition; execution partitioning stays result-free](0119-k7-canonical-tiling.md) | accepted (owner, 2026-11-12), with conditions | amends A3 §6.7 and GEN-101 (K7 per tile); reading of CON-08, CON-14, CON-30 for K7 only; gate G-SEAM |
