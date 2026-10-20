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
| 0016 | [Height-aware terrain selection and morph](0016-height-aware-terrain-selection.md) | accepted |  |
| 0017 | [Golden review policy and agent blessing](0017-golden-review-policy.md) | accepted | cross-adapter agreement amended by 0061 |
| 0018 | [Persistent cache keys: 128 bits from BLAKE3 over a canonical encoding](0018-persistent-cache-keys.md) | accepted |  |
| 0019 | [Tile storage: an append-only pack with a rebuildable index, Hilbert-ordered](0019-tile-storage-pack-format.md) | accepted |  |
| 0020 | [Autonomous performance runs behind a measured readiness gate](0020-autonomous-perf-runs.md) | accepted |  |
| 0021 | [Authored-item index: extent, layers, widths, staging](0021-authored-item-index.md) | accepted | points 2, 5, 6, 8 decided by ADR 0066 |
| 0022 | [Stage reads, scopes and halos](0022-stage-reads-and-halos.md) | accepted | option 2 needs the owner's CON-08 reading (trigger: a chain with same-stage reads) |
| 0023 | [Clarifications to the persistent key encoding (ADR 0018)](0023-persistent-key-clarifications.md) | accepted |  |
| 0024 | [Clarifications to the tile pack format (ADR 0019)](0024-tile-pack-clarifications.md) | accepted |  |
| 0025 | [Tile payload codec, node set id and what cache-verify finds](0025-tile-codec-and-cache-verify.md) | accepted |  |
| 0026 | [Global-coarse field representation and bake layout](0026-global-coarse-representation.md) | accepted |  |
| 0027 | [Reading the global-coarse bake, and the canonical block layout](0027-reading-the-global-bake.md) | accepted |  |
| 0028 | [The terrain field: a pure function of the direction over the global bake](0028-terrain-field-from-the-global-bake.md) | accepted |  |
| 0029 | [Compaction and the size cap: write then delete, an injectable eviction rank](0029-compaction-and-the-size-cap.md) | accepted |  |
| 0030 | [Erosion strategy: local filter in M3, a coarse global stream-power stage after it](0030-erosion-strategy-hybrid.md) | accepted |  |
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
| 0044 | [M4 implementation readings of ADR 0031 (cancellation) and ADR 0032 (dive pitch)](0044-m4-implementation-readings-of-0031-and-0032.md) | accepted |  |
| 0045 | [Bodies, bake resolution and the storage budget](0045-bodies-bake-resolution-and-storage-budget.md) | proposed | all but point 3: multi-body work (point 7 before the editor ships) |
| 0046 | [Generation workers run below normal priority](0046-generation-workers-run-below-normal-priority.md) | accepted |  |
| 0047 | [Adapter gate: a discrete GPU, chosen before the device exists](0047-adapter-gate.md) | accepted |  |
| 0048 | [Frame-target share (amends ADR 0033's remainder) and the lit path's readings of ADR 0038 and ADR 0042](0048-frame-target-share-and-lit-path-readings.md) | accepted |  |
| 0049 | [The climate field's bound and storage (amends ADR 0039 point 2), and the look file's version rule](0049-climate-field-bound-and-storage.md) | accepted | climate storage amended by 0062 |
| 0050 | [Atmosphere: implementation readings of ADR 0040](0050-atmosphere-implementation-readings.md) | accepted |  |
| 0051 | [Ocean channel, liquid table and the water-level field: implementation readings of ADR 0041](0051-ocean-channel-liquid-table-and-water-field-readings.md) | accepted |  |
| 0052 | [First M5 goldens: an edge tolerance for the cross-adapter debug-view check](0052-m5-debug-view-edge-tolerance.md) | superseded by 0061 |  |
| 0053 | [The water pass: implementation readings of ADR 0041 points 4 to 9](0053-water-pass-readings-of-adr-0041.md) | accepted | owner judges `shore_filter` at the M5 gate |
| 0054 | [Compact tiles in storage buffers instead of texture atlases (amends ADR 0033 point 3)](0054-compact-tiles-in-storage-buffers.md) | accepted | slot layout amended by 0062 |
| 0055 | [Documentation model: specs hold current rules, ADRs record why](0055-documentation-model.md) | accepted | point 6: consolidation in the M5 housekeeping (queue item 10b) |
| 0056 | [Water selection: the coastal band, tile water in the tile job, a pixel-footprint wave fade](0056-coastal-band-water-selection.md) | accepted | amends 0053 points 4 and 5 |
| 0057 | [GPU figures below the clock floor: a cool start, and a pass is a pass](0057-gpu-clock-below-the-floor.md) | accepted | amends 0036 point 2 |
| 0058 | [A coarse level of the water field: a summary the bake writes once](0058-coarse-level-of-the-water-field.md) | accepted | amends 0053 points 4, 5 and 0056 points 1, 4 for coarse tiles; for the owner at the M5 gate |
| 0059 | [Noise floors: 0.1 ms per pass, 0.5 ms per frame total](0059-noise-floors.md) | accepted | amends 0036 point 3; for the owner at the M5 gate |
| 0060 | [A power-capped P0 clock is valid; coast-3m water budget 2.2 ms](0060-power-capped-clock-and-coast-3m-water.md) | accepted (owner) | amends 0036 point 2 and 0057 point 2 |
| 0061 | [Goldens after M5: the M5 window closes, goldens are per-adapter regression checks](0061-goldens-after-m5.md) | accepted (owner) | supersedes 0037 point 1 and 0052; amends 0017 |
| 0062 | [Retire the vertex tile format from rendering](0062-retire-the-vertex-tile-format.md) | accepted (owner) | first work of M6; point 4 amended by 0063 |
| 0063 | [Slot v2 without the 5x5 reserve: 9,168 B](0063-slot-v2-size-correction.md) | accepted | corrects 0062 point 4; for the owner at the M5 gate |
| 0064 | [The render floor's roots load outside the per-frame upload cap](0064-floor-roots-exempt-from-the-upload-cap.md) | accepted | amends 0033 point 9 |
| 0065 | [CPU calibration decides whether a perf scene counts](0065-cpu-calibration-validity.md) | accepted | extends 0020 and 0036 |
| 0066 | [Authored items: reach, region summaries, stages, selection bounds, item files, command log](0066-authored-items-reach-summaries-stages-and-files.md) | accepted | decides 0021 points 2, 5, 6, 8; amends 0007, 0018, 0023 |
| 0067 | [Stamp key hash and height bounds by delta range](0067-stamp-key-hash-and-delta-range-bounds.md) | accepted | amends 0066 points 2, 5 and the key role of 6 |
| 0068 | [An authored stage with an empty footprint passes its source key through](0068-empty-authored-stage-passes-the-key-through.md) | accepted | amends 0066 point 4 |
| 0069 | [Lines and regions are point-list items; a long line splits into linked items](0069-lines-and-regions-are-point-list-items.md) | accepted | extends 0066 point 6 and 0067; mode 1 pending (region-mode1) |
| 0070 | [Brush strokes stay in the item file; the blob store is specified and deferred](0070-brush-strokes-stay-in-the-item-file-no-blob-store-in-the-mvp.md) | proposed | extends 0066 point 6 and 0069; decides the blob ADR of 0007 |
