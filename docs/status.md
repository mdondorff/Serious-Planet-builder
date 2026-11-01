# Status

Forward state only (owner, 2026-10-16): open owner items, the current queue's open items, the current unanswered stop report, tracked items and quarantines. History goes to the log (`cargo xtask log`). When a change is archived the coder deletes its queue item's text (the log line is the record); when the architect answers a stop report it deletes it. A Tracked entry has at most 3 lines; a finished one is deleted (ADR 0075). Cap: 15 KB. Earlier states: `docs/archive/M4/status-at-m4-acceptance.md`, `docs/archive/M6/status-at-m6-close.md`. M8 closed 2026-10-30: `docs/milestones/M8-report.md`. New requirement IDs, ADR and SPIKE numbers are reserved through the planner (2026-10-30).

## Waiting for the owner
- **For M9 planning** (after the away week): owner test 2026-10-27 groups E (edits on top as default, erosion keeping mass), F and G scope, the 1 m / 500 m exception (`docs/milestones/M8/owner-test-2026-10-27/README.md`); where edits sit relative to M9's erosion (D3, owner 2026-10-26: under the erosion by default).
- **M9 decisions** (`docs/briefings/m9-decisions.md`): ADR 0089, ADR 0091 (45° ceiling) and ADR 0088 accepted 2026-11-01. Owner: the SPIKE-80 world map "looks generally wrong" (thin continuous belts, unchanged continents); first M9 step is the engine's whole-planet preview and iterative generator quality with preview images, before any planet bake. Open: the ADR verdict table and its status lines; ADR 0090; the 2° shadow cost (ADR 0085 options).
- **ADR 0083 shares** (`compute-budgets`, BUDG-001 to BUDG-007): initial values ratified provisionally by opus-planner (2026-10-30). Terrain cap (BUDG-003) 12 → 10 proposed (2026-11-01, asked by push): unpinned 10 workers match 12 on throughput, tile-ready -26 % on dive, dive p99 +0.6 ms; 8 workers leave teleport-1m at 83 % of its tiles.
- **Optional, when convenient:** the M6 retest list (`docs/archive/M6/briefings/m6-owner-test-plan.md`, "Retest"); a live check of the flickers (Tracked); excluding the repo's `target` folders from Windows Search indexing. One editor session at the marker "TheEdgeHill" on the RTX (SPIKE-87 measured 0.17 ms editor cost on WARP; the 10-21 report predates the off-main block decodes).

Dormant (no near trigger; not asked now):
- **ADR 0045** (bodies, bake resolution, storage budget): proposed. Trigger: multi-body work.
- **Region keep-inside** (`openspec/changes/region-mode1`): soft influence for Cities, not a hard clip (owner, 2026-10-21, Q8). Trigger: Cities.
- **Global authoring preview** (owner, 2026-10-31): a coarse whole-planet preview of relief and landforms from the inputs, before the bake, as the step before detailed editing; not runtime. SPIKE-69 rates its k-sources for this. Trigger: after M9's bake, or when SPIKE-69 shows a pre-bake source.
- **Coarse-first refinement of an edit** (owner, 2026-10-20): parked. Trigger: a brush case of the edit-latency scene that misses its budget.

## Away-week queue (Option B, `docs/briefings/week-2026-11-02.md`; started 2026-10-30)
| # | Step | Lane |
|---|---|---|
| NR | Real-GPU window checks NOT RUN on a locked desktop (TEST-073; owner, 2026-10-30) | crusader |
| B4 | Automated owner checks (`openspec/changes/b4-owner-checks/`): B4-5 package command (TEST-074), B4-4 edit scenario, B4-6 delivery | explorer |
| Shadows | ADR 0085 clipmap march (ADR 0090 proposed): PRs 1 to 4 merged (#985, #1000, #1014, #1017, #1024, #1041); next 5a render path (flag, off by default), 5a2 app wiring and J view, 5b apply (every lit golden to the owner's /bless), then the perf window at the 2° pose; PRs 6 to 9 (7, 8 remove GEN-086, STRM-061) | crusader |
| B5 | Done: SPIKE-84 fixes 1 to 6 and the follow-ups (#1021, #1025, #1027, #1029 to #1032, #1034, #1035, #1037). Further Tracked fixes: camera ground read off the main thread (SPIKE-88, spikes) | spikes |
| M9 | ADR 0088/0089 proposed (#1013); planned spec deltas GEN-093 to 102, STRM-065 to 068, TEST-081 to 084 in `m9-bake` (#1018, #1019); unmeasured values marked start values for R7. Waits for the owner | owner |

After Option B: M9 R3 to R6 (week plan section 4). B6 (chunk stage removed) and B8 (ground-detail stills) done 2026-10-31.

## Tracked
- **Reserve test result** (BUDG-012, loaded, 3 rounds, 2026-11-01): dive, look-1m, look-2km, teleport-2km pass; teleport-1m fails by +1 % (frame median 16.75 against 16.60 ms; the spin runs 5-18 % over its 6-9 ms target). Frame p99 over 20 ms includes the spin by construction. A quiet window decides the absolute rows.
- **teleport-1m reserve marginal** (quiet window, 2026-11-01): reserve verdict pass in 3 of 3; frame medians with loads 14.33/16.31/16.40 ms against 16.60, but 5 of 6 sessions had no frame verdict (ADR 0065 CPU calibration out of limits). Re-check on a calm CPU, after the terrain cap decision.
- **Shadow resolve cost** (5a probe, 2026-11-01, RTX by hand, not a perf window): resolve 4.2 ms at a 45° sun, 2.3 ms at 2°, against REND-052's 1.5 ms for layer 1; includes per-frame buffer creation. Re-probe after pooling in 5a2; over 2x the budget goes back to ADR 0085 before 5b.
- **Fine-level bound looseness** (SPIKE-86): below 1 km over steep land 5.5 % of real-store poses cap leaves at levels 16-21; tile bounds are 5-37x the sampled range. Fix with M9 R4's bounds pyramid (SPIKE-83) or a per-octave tile bound, with a tier-A bound-vs-sampled test.
- **Reserve test gaps** (2026-11-01): terrain GPU is sampled at the last pose only; the share ceilings of ADR 0083 are provisional, pending the owner. Scaling (P-cores, 3 rounds): tile-ready rises with every worker, throughput saturates at 8-10; proposal to restate BUDG-003 sent to the planner.
- **`gpu_stream_perf` 300 s timeout on Windows WARP C 2/2** (2026-10-31): did not recur after the jobs.rs shutdown fix (#1001), cause not proven; the capture names the stage if it returns.
- **Upload pacing after a teleport** (job-system PR 4, 2026-10-31): lit teleport-2km frame p99 4.5 -> 6.2 ms (+36 %, accepted) as tiles land 2.7x sooner and their uploads bunch; pace uploads in B3 or the upload work.
- **Pooled fan-out** (job-system PR 3b): helper threads taken from the job system's workers instead of scoped threads per call could save about 1.5 % of a cold plan job; needs lifetime erasure, own PR with a numerics-reviewer pass.
- **Store write** (2026-10-31 16:02): `generate --prefill-dir` into the read-only `global16384` appended 1,920 tiles of levels 3-4 (18,116,378 B) to pack-00000007; the definition hash is unchanged and the data is cache only. Kept (no removal tool). Perf preparation prefills a copy or a separate cache directory from now on.
- **gpu_horizon flake** (2026-10-31): `a_slot_written_through_set_horizon_reads_back_as_written` failed once in a loaded prepush ("an unwritten slot is zero", gpu_horizon.rs:77); passes alone and on CI. Likely uninitialised GPU memory on the software adapter under load. Watch for a recurrence.
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
- **K view stall at orbit** (report `20261030-110527Z`): cause found (SPIKE-88): the camera's ground read decodes on the main thread and loaders evict its blocks (70-317 ms under load); fix in progress (`camera-ground-off-main`, spikes).
- **Blocky sun glint while turning** (owner, 2026-10-23, RTX; `docs/dev_screenshots/2026-10-23-bug-sun-glint-*.png`): pitch or yaw turns the sea glint into rectangles; a still camera restores it. Suspects: the turn cover (STRM-033), a per-tile water normal. Next: F9 while turning, F3 overlay.
- **Orbit flicker and dark-pixel flicker not reproduced** (owner, 2026-10-22): still-camera views identical (TEST-068 to TEST-070); not covered: streaming on the lit path while moving, the editor hover, wall-clock water. Needs the owner: editor off, F3 overlay, a recording or F9.
- **Authored items at scale** (SPIKE-29): cold plan over empty ground that sees a 5k/20k cluster 3.75/9.4 s; candidate: a per-cell bound aggregate (`spikes/edit-scale/results/results.md`).
- **Tiles flash to a coarser level while moving** (owner, P1-4, P2-8): not reproduced (TEST-067, TEST-070, TEST-072 on a generated world); next: the owner's F9 capture; open: a margin round the plan's frustum.
- **Flaky `gpu_window` flight test**: one failure after 7.5 s, 0 of 5 since; probably the lock (first item). On a second failure keep the full output.
- **WARP device removal with the compact format and the atmosphere**: test-only, accepted; the software smoke stays on the vertex format. Re-test when WARP, wgpu or Windows changes (Windows changed 2026-10-30).
- **Beach shading flashes on WARP** (36.8 % of shore-band pixels on the coastal descent): unchecked since REND-030; check with the next look review.
- **ADR 0056 limits**: steep coastal ground and inlets under 4 km stay dry (fix: per-vertex sea mask); `WaterBook` is never evicted (watch its growth).
- **ADR 0051 (superseded by 0055)**: point 1 (liquid parameters in the definition) awaits the planner's acceptance; point 6 (per-tile W storage) is open.
- **Cold start without the pre-fill**: 384 roots in 17.76 s (indicative, software adapter); no budget set.
- **Flight clock collapse follows run order** (windows 10, 11): the first run of a flight pair drops to P5 or P8; cause open.
- **Editor verifier gaps** (minor): `Editor::set_line_params` skips the inspector ranges; `WidthTooLarge` at the part budget untested; the brush chain unhardened for radius 0 or NaN; poles and antimeridian untested for the brush chain.
