# Status

Forward state only (owner, 2026-10-16): open owner items, the current queue's open items, the current unanswered stop report, tracked items and quarantines. History goes to the log (`cargo xtask log`). When a change is archived the coder deletes its queue item's text (the log line is the record); when the architect answers a stop report it deletes it. A Tracked entry has at most 3 lines; a finished one is deleted (ADR 0075). Cap: 15 KB. Earlier states: `docs/archive/M4/status-at-m4-acceptance.md`, `docs/archive/M6/status-at-m6-close.md`. M8 closed 2026-10-30: `docs/milestones/M8-report.md`. New requirement IDs, ADR and SPIKE numbers are reserved through the planner (2026-10-30).

## Waiting for the owner
- **For M9 planning** (after the away week): owner test 2026-10-27 groups E (edits on top as default, erosion keeping mass), F and G scope, the 1 m / 500 m exception (`docs/milestones/M8/owner-test-2026-10-27/README.md`); where edits sit relative to M9's erosion (D3, owner 2026-10-26: under the erosion by default).
- **ADR 0083 shares** (`compute-budgets`, BUDG-001 to BUDG-007): initial values ratified provisionally by opus-planner (2026-10-30); the owner may override.
- **Optional, when convenient:** the M6 retest list (`docs/archive/M6/briefings/m6-owner-test-plan.md`, "Retest"); a live check of the flickers (Tracked); excluding the repo's `target` folders from Windows Search indexing.

Dormant (no near trigger; not asked now):
- **ADR 0045** (bodies, bake resolution, storage budget): proposed. Trigger: multi-body work.
- **Region keep-inside** (`openspec/changes/region-mode1`): soft influence for Cities, not a hard clip (owner, 2026-10-21, Q8). Trigger: Cities.
- **Coarse-first refinement of an edit** (owner, 2026-10-20): parked. Trigger: a brush case of the edit-latency scene that misses its budget.

## Away-week queue (Option B, `docs/briefings/week-2026-11-02.md`; started 2026-10-30)
| # | Step | Lane |
|---|---|---|
| NR | Real-GPU window checks NOT RUN on a locked desktop (TEST-073; owner, 2026-10-30) | crusader |
| B4 | Automated owner checks (`openspec/changes/b4-owner-checks/`): B4-5 package command (TEST-074), B4-4 edit scenario, B4-6 delivery | explorer |
| B2, B3 | One job system (PRs 1a, 2 merged; 3a plan worker; 3b `Par` in core after B6), then the reserve test (ADR 0083, BUDG-008 to 010) | explorer |
| B5 | Test robustness: the rest after #912 (`test fast` budget mostly falls with B6) | crusader |
| B7 | The terrain scorecard (`openspec/changes/terrain-scorecard/`, 3 PRs) | crusader |
| M9 spikes | SPIKE-66 relief factor in the generator (after SPIKE-61 to 65: bake 4.5-6 h, amplitude per landform is the relief lever, ground-detail stills sent to the owner) | spikes |
| HK | M8 housekeeping, mechanical pass and evidence diet (declutter batches 3, 4, 7) | spikes |
| R4b | `GeoPoint::of_dir` and `dir` in `editor/polyline.rs` onto the core helpers | crusader |

After Option B: ADR 0085's build (after the scorecard; opus-planner, 2026-10-31), then M9 R3 to R6 (week plan section 4). B6 (chunk stage removed) and B8 (ground-detail stills) done 2026-10-31.

## Tracked
- **Window checks need an unlocked desktop** (2026-10-30): on the RTX, present stalls 1.3-2.7 s or the device is lost while the session is locked or in modern standby; unlocked the dense sweep takes 26 s. The device losses since 10-18 most likely had this cause. NR handles it. Display off after 2 h (owner, 2026-10-31); the owner unlocks remotely for a batch, which the planner requests 07:00-22:00.
- **CI WARP jobs over budget** (BUILD-014, `ci-stats` 2026-10-30): A+B 17.1, C 1/2 18.8, C 2/2 16.8 min medians against 16; B6 removed the chunk tests: re-read `ci-stats` after the next runs.
- **naga panic on a constant ground-detail octave** (SPIKE-67, 2026-10-31): naga 30.0.1 SPIR-V backend `block.rs:1022` "Expression [241] is not cached!" when the lit shader binds an octave to a constant (`let o = vec4<f32>(0.0);` in place of the 3 cm `gd_octave` call), Vulkan on the RTX; not reduced or reported upstream.
- **Local WARP aborts under nextest** (2026-10-30, after the Windows update): 2 of 177 in `test gpu` (access violation in `d3d10warp.dll`), pass alone; CI judges the step.
- **Fast tier over budget** (BUILD-003, BUILD-014): `test fast` 44-71 s under load; 391 s single-thread total, the largest items were chunk tests (removed in B6): re-measure.
- **Detail blocks** (BUILD-027): 10 in `xtask/src/detail_blocks.txt`: split or shorten each, delete its line; no new blocks.
- **Frame tail** (SPIKE-44): `draw_plan_transient` 8-14 ms on a plan delivery (24-48 frames/min) + ~9 ms frame work drops a frame at 60 fps; ADR 0083 frame share.
- **Coupled run key read**: about 25M bake samples are read for the key even when the run is stored; the K view is coarse (climate shading).
- **Static memo** (CON-06): `global::landform_lattice` is a static Mutex memo of a pure function of the seed; later the bake's owner builds it and passes it to `ErosionGenerator::field`.
- **Water cut keeps more nodes after the hill band** (#673): kept share 53-73 % (was 40-50 %); check the water pass's GPU time in the next perf window.
- **Horizon-map shadows retiring** (ADR 0085): their latency (window 28: 131 of 269 maps over 10 s on large edits), disk cache and edge limits are not worked on. Anything added to the terrain pass keeps criterion 5's 1.5 ms (max 1.42).
- **Drag residuals** (owner test 2, 2026-10-30): after #900 some tiles are clipped or not drawn while dragging and settle at rest. Real-store pair (#949, RTX and software adapter agree within 2 px, 2026-10-31): preview residue at most 6 % of changed pixels (contour lines), 0 at river-r1; lit frame and metres NOT RUN. No fix scheduled.
- **`--site` in the editor** (owner, 2026-10-30): no descent, the window opens at the default view; `--marker` with G works.
- **Window resize (P2)** (owner, 2026-10-30): the 3D target keeps its size after a resize; `--size WxH` at start works around it.
- **K view stall at orbit** (report `20261030-110527Z`): 1.3 s frame, 1,063 ms in camera, ground heights and overview with 4 main-thread block decodes (within VIEW-061's ceiling).
- **Blocky sun glint while turning** (owner, 2026-10-23, RTX; `docs/dev_screenshots/2026-10-23-bug-sun-glint-*.png`): pitch or yaw turns the sea glint into rectangles; a still camera restores it. Suspects: the turn cover (STRM-033), a per-tile water normal. Next: F9 while turning, F3 overlay.
- **Orbit flicker and dark-pixel flicker not reproduced** (owner, 2026-10-22): still-camera views identical (TEST-068 to TEST-070); not covered: streaming on the lit path while moving, the editor hover, wall-clock water. Needs the owner: editor off, F3 overlay, a recording or F9.
- **Editor-mode CPU frame cost** (owner, 2026-10-21): over 90 ms CPU at cube edges and corners in the editor (marker "TheEdgeHill", 35.2538 N 135.0217 E), under 2 ms with the editor off. Hypothesis: `frame::pick::pick_surface` across face seams.
- **Authored items at scale** (SPIKE-29): cold plan over empty ground that sees a 5k/20k cluster 3.75/9.4 s; candidate: a per-cell bound aggregate (`spikes/edit-scale/results/results.md`).
- **Tiles flash to a coarser level while moving** (owner, P1-4, P2-8): not reproduced (TEST-067, TEST-070, TEST-072 on a generated world); next: the owner's F9 capture; open: a margin round the plan's frustum.
- **Base LOD refused below about 31 km over a field with only a global bound** (VIEW-032): 6.8 to 9.4 % of sweep poses; possibly a harness artefact. Next: sweep the real eroded world.
- **RTX known-answer hashes** in `gpu_compact_blocks.rs` and `gpu_water.rs` removed under MESH_VERSION 8: re-record with `test gpu --real` (now MESH_VERSION 9).
- **Item store revisit trigger** (ADR 0066 point 6): more than 20,000 items in a body or a load over 2 s decides the backend. Nothing counts or times it yet.
- **M3 gate condition 2**: the declared-versus-recorded reads contract test (ADR 0021 point 7) and a tile-level GEN-017 test are approved, not written.
- **Flaky `gpu_window` flight test**: one failure after 7.5 s, 0 of 5 since; probably the lock (first item). On a second failure keep the full output.
- **WARP device removal with the compact format and the atmosphere**: test-only, accepted; the software smoke stays on the vertex format. Re-test when WARP, wgpu or Windows changes (Windows changed 2026-10-30).
- **Beach shading flashes on WARP** (36.8 % of shore-band pixels on the coastal descent): unchecked since REND-030; check with the next look review.
- **ADR 0056 limits**: steep coastal ground and inlets under 4 km stay dry (fix: per-vertex sea mask); `WaterBook` is never evicted (watch its growth).
- **ADR 0051 (superseded by 0055)**: point 1 (liquid parameters in the definition) awaits the planner's acceptance; point 6 (per-tile W storage) is open.
- **Cold start without the pre-fill**: 384 roots in 17.76 s (indicative, software adapter); no budget set.
- **Flight clock collapse follows run order** (windows 10, 11): the first run of a flight pair drops to P5 or P8; cause open.
- **Editor verifier gaps** (minor): `Editor::set_line_params` skips the inspector ranges; `WidthTooLarge` at the part budget untested; the brush chain unhardened for radius 0 or NaN; poles and antimeridian untested for the brush chain.
