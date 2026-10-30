# Status

Forward state only (owner, 2026-10-16): open owner items, the current queue's open items, the current unanswered stop report, tracked items and quarantines. History goes to the log (`cargo xtask log`). When a change is archived the coder deletes its queue item's text (the log line is the record); when the architect answers a stop report it deletes it. A Tracked entry has at most 3 lines; a finished one is deleted (ADR 0075). Cap: 15 KB. Earlier states: `docs/archive/M4/status-at-m4-acceptance.md`, `docs/archive/M6/status-at-m6-close.md`. M7's queue as executed: `docs/milestones/M7-report.md`.

## Waiting for the owner
- **`/bless`** of 10 RTX golden candidates on driver 621.01 (`target/review/golden-review/report.md`); then explorer re-runs `accept M8`. Owner test 2 (2026-10-30): done, nothing blocking; criterion 1 PASS (relief plausible enough for M8); the default look stays, the driver look is revisited with M9's driver maps.
- **Owner live test 2026-10-27** (`docs/milestones/M8/owner-test-2026-10-27/README.md`): open for M9 planning: group E (edits on top as default, erosion keeping mass), F and G scope, the 1 m / 500 m exception. Decided 2026-10-28: the no-degradation gate first, then the D0 decision measurement with its stop rule (README "Decisions").
- **D3 confirmed (owner, 2026-10-26):** both orders are wanted eventually; M8-7 builds edits under the erosion first (option 2: the edit shows at once, its eroded result follows within 10 s); edits on top (finishing touches) stay a per-layer setting for later. Cost per stamp or brush stroke is M8-7's first measurement.
- **ADR 0083 (compute budgets):** the share table is a spec (`compute-budgets`, BUDG-001 to BUDG-007, all planned; initial values ratified provisionally by opus-planner, 2026-10-30; the owner may override). Not built: pools not on one job system (B2), no reserve test (B3).
- **Optional, when convenient:** the M6 retest list (`docs/archive/M6/briefings/m6-owner-test-plan.md`, "Retest"); a live check of the flickers (Tracked).
- **At the laptop (postponed):** excluding the repo's `target` folders from Windows Search indexing.

Dormant (no near trigger; not asked now):
- **ADR 0045** (bodies, bake resolution, storage budget): proposed. Trigger: multi-body work.
- **Region keep-inside** (`openspec/changes/region-mode1`): soft influence for Cities, not a hard clip (owner, 2026-10-21, Q8). Trigger: Cities.
- **Coarse-first refinement of an edit** (owner, 2026-10-20): parked. Trigger: a brush case of the edit-latency scene that misses its budget.

## M8 queue (plan: `docs/briefings/m8-plan.md`; next week's start: `docs/briefings/week-2026-10-25.md`)

| # | Step | Lane |
|---|---|---|
| M8-1 (c) rest | open: the key read of about 25M bake samples even when the run is stored; uplift and erodibility constant until the driver maps (ADR 0076); the K view is coarse (climate shading). Done: the run at 2048 from any bake (#638, GEN-056); criterion 4 (window 21: a fresh make from the 16384 bake took 19.7 s) | crusader |
| R4b | `GeoPoint::of_dir` and `dir` in `editor/polyline.rs` onto the core helpers | crusader, with M8-7 |
| CS | After M8's acceptance: remove the chunk stage (`--chunks`, chunk erosion and routing, chunk jobs) from the code; git keeps the history (owner, 2026-10-29); horizon maps stay until SPIKE-56's ADR replaces them | crusader |
| RS | Parked research (owner, 2026-10-30): all publications at https://h-schott.github.io/publications/ checked for M9 "Realistic terrain" fit: GPU and CPU cost, build time within the roadmap; first look at terrain descriptors (https://h-schott.github.io/p/desc/). A desk-study spike when the spikes lane is free; not started | spikes |

Open owner items: the two flickers (Tracked) need a live check (editor off, debug path L, or a recording or F9 at the moment); P1-5 editing frame rate on a quiet machine; P2-8 missing tiles during an edit's refresh.

## Tracked
- **Detail blocks** (BUILD-027): 10 in `xtask/src/detail_blocks.txt`: split or shorten each, delete its line; no new blocks.
- **Frame tail** (SPIKE-44): `draw_plan_transient` 8-14 ms on a plan delivery (24-48 frames/min) + ~9 ms frame work drops a frame at 60 fps; ADR 0083 frame share.
- **Hillslope flat patches** (look review 2b): ground beyond a rim is 0.7 m under the base, half as rough, flat in 60-160 m patches.
- **Driver fade seam** (#716): a tile rebuilt for a neighbour chunk's landing fades its drivers up from neutral again. Trigger: the first look with driver curves. Fix: a driver increment plane on the upload path (crusader). Resolution: 32 m texels (GEN-074); M9: per-vertex.
- **Flight lead** (criterion 7 margin 1.6x): the track asks half an edge further, at least one edge (STRM-042, tier A). Measure late and first-frame chunks in the acceptance window (`perf stream envelope --only-steady`); a first-frame chunk still unasked is a queue cause.
- **Steepest river and the 3e7 m2 drop**: at (34.158, 140.835) 519 of 1,344 links climb and 3,491 cells sit at the 300 m bound: the coarse network climbs beyond a 1 km node slide (M9, coarse run). A 3e7 m2 tributary threshold gave a 27.8 m drop; the stage uses 5e7.
- **Deep cut where the ground along the routed path rises** (river-r1, look review 2): a 40-50 m canyon on a plain, the routed ground +15 m over 3 km, the bed at the least fall (GEN-059); a cut-cap change trades it for a dam (GEN-069). Fix with the M9 coarse/fine network work.
- **Fine tributary tier is M9** (ADR 0081 point 3): the chunk's drainage gives 0.01-0.43 km per km2 over 1e5 m2, none over 1e6 (SPIKE-40: 0.5-3); flow restarts at a region's edge, a seam at a chunk border.
- **Static memos** (CON-06): `global::landform_lattice` is a static Mutex memo of a pure function of the seed; later the bake's owner builds it and passes it to `ErosionGenerator::field`. Reset `RouteMemo` if `ANet.run`, `.base` or `.radius_m` change.
- **M8-7 remaining; `test fast` over budget** (crusader): edits under the erosion are in (#734 to #749); next the re-solve reuse of base heights (criterion 6, 10 s) and the first stamp and brush measurement on the 16384 store. `test fast` takes 36 to 59 s (BUILD-014): `an_edit_in_the_read_set_...` 30 s; share a store or tier B.
- **Chunk-fade limits** (crusader): chunks landing within 1.5 s apart and fresh LOD tiles crack against neighbours; edits and new chunk versions pop at the swap (M8-7); plane grants forgotten off-plan; mesh-pack tiles carry no plane; normal-choice look test; 2 km octave aliasing. Epochs closed (#714). Look review 2 judges. Editor stall: 0 frames over 1 s, edits not driven.
- **Water cut keeps more nodes after the hill band** (opus-planner, 2026-10-25, #673): `oracle_water_cut`'s declared re-pin raised the kept share from 40-50 % to 53-73 % (upper bounds 65 to 80, 55 to 65, 60 to 70); sea pixels unchanged. More water nodes drawn: check the water pass's GPU time in the next perf window.
- **Shadows redone** (owner, 2026-10-29): the horizon-map shadows (GEN-044/045, REND-038, STRM-048/060/064) are replaced after SPIKE-56 (shadow architecture: GPU heightfield ray-march as the lead candidate, objects as occluders later); their latency, disk cache and edge limits are not worked on. Window 28: 131 of 269 maps over 10 s, all on large edits; a map about 4 s at up to 8 at once.
- **M8 criterion 5 met in window 26** (2026-10-28; max 1.373 ms of 1.5 at river-r1 300 m midday): anything added to the terrain pass must keep it.
- **Fast tier over budget under load** (BUILD-003, BUILD-014): `test fast` took 44-71 s under other sessions' tests. Local `test gpu` with the heavy group (#885): 177 of 177 pass on a quiet machine, slowest 249 s (TEST-070), 230 s, 181 s (2026-10-29).
- **Blocky sun glint while turning** (owner, 2026-10-23, RTX; `docs/dev_screenshots/2026-10-23-bug-sun-glint-*.png`): low over a coast, sun ahead; pitch or yaw only turns the sea glint into a grid of rectangles; a still camera restores it. Path: 584 samples, 59.9 s, 1.0 to 577.5 m, slowest frame 21 ms.
  Suspects: the turn cover (STRM-033) drawing coarser water tiles while the plan lags; a per-tile water normal. Next: replay with F9 while turning, compare tile levels in the F3 overlay.
- **Orbit flicker and dark-pixel flicker not reproduced** (owner, 2026-10-22; `target/review/frame-61216`): the sync-plan replay (relief, lit) is stable at the centre, a pipeline simulation with everything resident draws the ideal plan, and a sun-ahead specular test at 12 to 240 m shows no flicker (a Tokuyoshi kernel changed nothing). A still camera draws identical pixels in the level, relief and normals views (TEST-068, 2026-10-29); the lit coast view with the sun ahead and the overlay on also (TEST-069); not covered: streaming with the lit path, the editor hover, wall-clock water. Needs the owner's answer: editor off, debug view, F3 overlay; or a screen recording; suspects: streaming stand-ins, hover overlay, exposure, water glint.
- **Editor-mode CPU frame cost** (owner, 2026-10-21, RTX, `target/owner-items`): at about 18 km near "Plateau 7", looking past "Plateau 4" in the direction of travel, CPU over 90 ms, GPU under 4 ms; editor off: CPU under 2 ms. It follows the cube's edges and corners, not the edits (marker "TheEdgeHill", 35.2538 N 135.0217 E). Hypothesis: the hover pick's height lookup (`frame::pick::pick_surface`) is slow across face seams. Next: time it at a face centre, an edge and that corner, then an F9 profile.
- **Authored items at scale** (SPIKE-29, 2026-10-22; Ridge 3): at 5k items the warm plan is 4 ms, the editor frame 17 ms, a real edit's plan 0.9 to 1.5 s near a cluster and 0.15 to 0.25 s elsewhere (20k: warm 4 to 44 ms). Open: the cold plan over empty ground that sees a 5k/20k cluster, 3.75/9.4 s; the candidate is a per-cell bound aggregate (looser bounds, a decision).
  Results and the curve: `spikes/edit-scale/results/results.md`.
- **Tiles flash to a coarser level while moving** (owner, 2026-10-19 and 20; P1-4, P2-8): seen near the sea, in debug views and on a slow approach from orbit. Not reproduced; a scripted approach through the managed loop never coarsens (TEST-067, 2026-10-29); the lit view over streamed tiles equals itself after a 2 degree turn and back (TEST-070, settled frames only; the frames during the turn are not compared); next: the owner's F9 capture.
  Lead answered by `turn-cover` (STRM-033: the cover is the plan of the frame's camera over what is resident; only the label was wrong). Still open: a margin round the plan's frustum so small turns keep the plan, and the flashing on movement.
- **Editor smoke race on the RTX** (VIEW-022): fixed by waiting for a still camera. Delete once main's `test gpu --real` passes it.
- **Base LOD refused below about 31 km over a field with only a global bound** (2026-10-20; VIEW-032): 6.8 to 9.4 % of sweep poses; possibly a harness artefact. Next: sweep the real eroded world.
- **RTX known-answer hashes** in `gpu_compact_blocks.rs` and `gpu_water.rs` removed under MESH_VERSION 8: re-record with `test gpu --real` (now MESH_VERSION 9).
- **Item store revisit trigger** (ADR 0066 point 6): more than 20,000 items in a body or a load over 2 s decides the backend. Nothing counts or times it yet.
- **M3 gate condition 2**: the declared-versus-recorded reads contract test (ADR 0021 point 7) and a tile-level GEN-017 test are approved, not written; the per-tile node set id waits for the first stored regional field.
- **Device loss on the RTX**: gone with driver 621.01 (2026-10-30: the item sweeps 16 of 16, full `test gpu --real` without a loss; on 591.86 9 of 16 failed). Delete after a week without a recurrence.
- **Drag residuals** (owner test 2, 2026-10-30): after #900 some tiles are clipped or not drawn while dragging an edit and settle at rest; collect F9 captures over time; no fix scheduled.
- **`--site` in the editor** (owner, 2026-10-30): no descent, the window opens at the default view; a debug aid; `--marker` with G works.
- **Window resize (P2)** (owner, 2026-10-30): the 3D target keeps its size after a resize, the world pixelates while the UI stays sharp; `--size WxH` at start works around it.
- **K view stall at orbit** (owner, 2026-10-30, report `20261030-110527Z`, build 79653500): 1.3 s frame, 1,063 ms in camera, ground heights and overview with 4 main-thread block decodes (within VIEW-061's ceiling); no panic any more.
- **Flaky `gpu_window` flight test**: one failure after 7.5 s, 0 of 5 since. On a second failure keep the full output before any rerun.
- **WARP device removal with the compact format and the atmosphere**: test-only, accepted; the software smoke stays on the vertex format. Re-test when WARP, wgpu or Windows changes.
- **Beach shading flashes on WARP** (36.8 % of shore-band pixels on the coastal descent): unchecked since REND-030; check with the next look review.
- **ADR 0056 limits**: steep coastal ground and inlets under 4 km stay dry (fix: per-vertex sea mask); `WaterBook` is never evicted (watch its growth).
- **ADR 0051 (superseded by 0055)**: point 1 (liquid parameters in the definition) awaits the planner's acceptance; ADR 0051 point 6 (per-tile W storage) is open.
- **Cold start without the pre-fill**: 384 roots in 17.76 s (indicative, software adapter); no budget set.
- **Flight clock collapse follows run order** (windows 10, 11): the first run of a flight pair drops to P5 or P8; cause open.
- **Editor verifier gaps** (minor): `Editor::set_line_params` skips the inspector ranges; `WidthTooLarge` at the part budget untested; the brush chain unhardened for radius 0 or NaN (unreachable); poles and antimeridian untested for the brush chain.
