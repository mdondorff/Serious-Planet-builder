# Status

Forward state only (owner, 2026-10-16): open owner items, the current queue's open items, the current unanswered stop report, tracked items and quarantines. History goes to the log (`cargo xtask log`). When a change is archived the coder deletes its queue item's text (the log line is the record); when the architect answers a stop report it deletes it. A Tracked entry has at most 3 lines; a finished one is deleted (ADR 0075). Cap: 15 KB. Earlier states: `docs/archive/M4/status-at-m4-acceptance.md`, `docs/archive/M6/status-at-m6-close.md`.

## Waiting for the owner
- **Quota (owner, 2026-10-21):** the M6 housekeeping runs this week; M7 work starts after the Sunday reset or on the owner's word.
- **Optional, when convenient:** the M6 retest list (`docs/archive/M6/briefings/m6-owner-test-plan.md`, "Retest"); an F9 capture of tile flashing (Tracked); the SPIKE-27 sheets to keep in `docs/dev_screenshots/`.
- **At the laptop (postponed):** the NVIDIA driver update (591.86; device losses under parallel GPU load, Tracked); excluding the repo's `target` folders from Windows Search indexing.

Dormant (no near trigger; not asked now):
- **CON-08 reading** (ADR 0022): may a field read another field of its own stage? Trigger: a regional chain that needs same-stage reads.
- **ADR 0045** (bodies, bake resolution, storage budget): proposed. Trigger: multi-body work.
- **Region keep-inside** (`openspec/changes/region-mode1`): soft influence for Cities, not a hard clip (owner, 2026-10-21, Q8). Trigger: Cities.
- **Coarse-first refinement of an edit** (owner, 2026-10-20): parked. Trigger: a brush case of the edit-latency scene that misses its budget.

## M7 queue: ground detail and edit health (plan: `docs/briefings/m7-planning.md`; scope reshuffled by the owner, 2026-10-21)
Global authoring (A1, A3, A4, E1, overview maps) moves to M9; M8 is generation principles (driver maps).

| # | Step | Lane |
|---|---|---|
| H2 | The ADR 0055 consolidation not done in the M5 housekeeping (item 10b, only its point 4 done): amendment chains into their specs (0033 with 0048 and 0054; 0037 with 0052 and 0061; 0041 with 0051 and 0053), readings ADRs (0027, 0044, 0048, 0050, 0051, 0053) set to superseded, spec-lint's countable style checks; word counts before and after | crusader, after H1 |
| I1 | Instancing spike at walking height, 1440p | spikes, after the reset |
| D1 | Driver-maps design spike for M8: the first maps (physical drivers, process results such as soil depth and moisture), resolution and bake cost, how response curves combine into suitability, what materials and instances read; ends with a proposed ADR | spikes, after I1 |

Open M6 items carried over (owner test, 2026-10-20): P1-4 tile flashing (Tracked), P1-5 editing frame rate on a quiet machine (owner's retest), P2-6 colour shift after fast surface-to-orbit moves, P2-7 colour step between 95 and 100 km, P2-8 missing tiles during an edit's refresh (with P1-4).

## Tracked
- **Editor-mode CPU frame cost** (owner, 2026-10-21, RTX, `target/owner-items`): at about 18 km near "Plateau 7", looking past "Plateau 4" in the direction of travel, CPU over 90 ms, GPU under 4 ms, under 10 fps; editor off: CPU under 2 ms; looking down at Plateau 7: CPU about 14 ms. Owner, later: it follows the cube's edges and corners, not the edits (it was there before any edit); marker item "TheEdgeHill" at the corner 35.2538 N 135.0217 E. Hypothesis (planner): a per-frame editor-only height sampling (the hover pick's ray march, `frame::pick::pick_surface`, or the picking-field rebuild) whose height lookup is slow across face seams. Next: time `pick_surface` and the picking field's height at a face centre, an edge and that corner (tier A), then an F9 profile.
- **Plan job over authored items: seconds per plan cold, 0.4 to 0.7 s warm while drifting** (windows 23 to 25, `docs/perf/window-25.md`): Ridge 3 2.5 s cold at 2 km against 0.43 s warm, 200 m 1.6 s against 0.67 s; the stamp stage's bounds queries take about 60 µs each; no code regression since window 23 (bisected). Candidate fix: stamp-stage bounds cached per item footprint. Trigger: the owner sees detail lag while flying over edits.
- **Tiles flash to a coarser level while moving** (owner, 2026-10-19 and 20; P1-4, P2-8): seen near the sea, in debug views and on a slow approach from orbit. Not reproduced; next: the owner's F9 capture, or a scripted approach asserting the drawn level never coarsens.
  Lead (owner, 2026-10-21) answered by `turn-cover` (STRM-033): the cover is the plan of the frame's camera over what is resident (not coarser with R resident); only the label was wrong, every cover tile was stand-in, now exact where it is the plan's tile. Still open: a margin round the plan's frustum so that small turns keep the plan (the plan culls to its own camera's exact frustum), and the flashing on movement (tiles missing while the position changes).
- **Keyboard dead after G** (owner, 2026-10-20; VIEW-034, VIEW-039): probable cause (Tab gave a panel the focus) fixed; the title shows the input state. Delete when the owner has not seen it again.
- **Editor smoke race on the RTX** (VIEW-022): fixed by waiting for a still camera. Delete once main's `test gpu --real` passes it.
- **Base LOD refused below about 31 km over a field with only a global bound** (2026-10-20; VIEW-032): 6.8 to 9.4 % of sweep poses; possibly a harness artefact. Next: sweep the real eroded world.
- **RTX known-answer hashes** in `gpu_compact_blocks.rs` and `gpu_water.rs` removed under MESH_VERSION 8: re-record with `test gpu --real` (now MESH_VERSION 9).
- **Item store revisit trigger** (ADR 0066 point 6): more than 20,000 items in a body or a load over 2 s decides the backend. Nothing counts or times it yet.
- **M3 gate condition 2**: the declared-versus-recorded reads contract test (ADR 0021 point 7) and a tile-level GEN-017 test are approved, not written; the per-tile node set id waits for the first stored regional field.
- **Device loss on the RTX** (windows 8, 2026-10-18): in the lit dive and in parallel `test gpu --real` runs; passes alone. A recurrence prints `wgpu device lost`; stop and investigate. Driver update is the owner's.
- **Flaky `gpu_window` flight test**: one failure after 7.5 s, 0 of 5 since. On a second failure keep the full output before any rerun.
- **Test timeouts under machine load** (2026-10-19 to 20): tier-C tests hit 300 s with other sessions building; all pass alone. Rerun once and note it.
- **WARP device removal with the compact format and the atmosphere**: test-only, accepted; the software smoke stays on the vertex format. Re-test when WARP, wgpu or Windows changes.
- **Beach shading flashes on WARP** (36.8 % of shore-band pixels on the coastal descent): unchecked since REND-030; check with the next look review.
- **ADR 0056 limits**: steep coastal ground and inlets under 4 km stay dry (fix: per-vertex sea mask); `WaterBook` is never evicted (watch its growth).
- **ADR 0051 (proposed)**: point 1 (liquid parameters in the definition) awaits the planner's acceptance; ADR 0051 point 6 (per-tile W storage) is open.
- **Cold start without the pre-fill**: 384 roots in 17.76 s (indicative, software adapter); no budget set.
- **Flight clock collapse follows run order** (windows 10, 11): the first run of a flight pair drops to P5 or P8; cause open.
- **Editor verifier gaps** (minor): `Editor::set_line_params` skips the inspector ranges; `WidthTooLarge` at the part budget untested; the brush chain unhardened for radius 0 or NaN (unreachable); poles and antimeridian untested for the brush chain.
