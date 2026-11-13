# Status

Forward state only (owner, 2026-10-16): open owner items, the current queue's open items, the current unanswered stop report, tracked items and quarantines. History goes to the log (`cargo xtask log`). When a change is archived the coder deletes its queue item's text (the log line is the record); when the architect answers a stop report it deletes it. A Tracked entry has at most 3 lines; a finished one is deleted (ADR 0075). Cap: 15 KB. Earlier states: `docs/archive/M4/status-at-m4-acceptance.md`, `docs/archive/M6/status-at-m6-close.md`. M8 closed 2026-10-30: `docs/milestones/M8-report.md`. New requirement IDs, ADR and SPIKE numbers are reserved through the lead (ADR 0120).

## Waiting for the owner
- **/bless: `water_class_lake_mask_stripes`** (TEST-133, #1459): the first lake golden, a class view; escalated as the first of its class and for a WARP driver difference. Nothing waits on it.
- **A new global store for the default world, seed 14**: every store on disk is refused by its keys; E1 runs on the in-memory production base (VIEW-069). Lead's recommendation: bake it after the flight, with ADR 0117's format.
- **For later planning**: owner test 2026-10-27 groups F and G scope, the 1 m / 500 m exception (`docs/milestones/M8/owner-test-2026-10-27/README.md`). Group E (where edits sit) is settled by ADR 0113.
- **Sunset shadow look** (ADR 0095 draft, crusader): the sky ambient as a hemisphere average is physically consistent and dark at a 7° sun; options: sky light plus a lift (planner's pick), sky light as is, or the old term. Every lit and sky golden moves with it. SPIKE-96 (sky visibility) follows the choice.
- **Architecture re-plan** (owner, 2026-11-08; SPIKE-104, SPIKE-105): D1 = option A, conditional on a prototype and a switch-to-B gate (S1-S5); phases and gates in `docs/briefings/terrain-prototype.md`; the owner decides at the A/B gate.
- **Smooth base as default** (GEN-107, `cpu-erosion-v2` on main): removes the 611 m "pyramid" facets; draft #1137 switches the default (no golden moves; two count pins and the water-cut range re-pinned with reasons). Decide with the erosion preview.
- **World-build share values** (ADR 0097, BUDG-014): terrain 40 %, future content 40 %, margin 20 % of a 24 h day, proposed; confirm or change.
- **Ocean square at orbit** (owner test 2026-11-03, `docs/dev_screenshots/2026-11-03-ocean-square-*.jpg`): press F9 when it shows again; the bundle lets crusader replay it (Tracked).
- **Cliffs** (SPIKE-94, #1082): asset library or procedural only; the Alps' 4.3 % of area above 45° as the v1 coverage. Trigger: after M9's generator rounds.
- **ADR 0110 acceptance** gates item 15 PR 5b (it retires ADR 0101 point 3); CON-08: resolved by constitution v1.7 (ADR 0116, #1335). ADRs 0110-0115: architect's text pass in P0, then owner acceptance.
- **Optional, when convenient:** the M6 retest list (`docs/archive/M6/briefings/m6-owner-test-plan.md`, "Retest"); excluding the repo's `target` folders from Windows Search indexing.

Dormant items moved to `docs/parking.md` (2026-11-12).

## Queue (2026-11-13, milestone P2: `docs/milestones/P2.md`)
Every row names an exit item (ADR 0120). Everything else is in `docs/parking.md`.
| Exit | Step | Lane |
|---|---|---|
| E1 | SPIKE-114: rivers routed at L on R3a (cut share, rises, agreement with H1 at the edge, cost, store or rebuild); then the ADR | crusader |
| E1 | VIEW-072 guard (the base under --commit is already bit-equal); then ADR 0122 point 1, pinned fine detail (GEN-187) | explorer |
| E2 | Acceptance: the owner's next long flight reads resident tiles at most and evicted from the run log | owner |
| E5 | Inland lakes and dams ADR, after crusader's rivers-at-L spike sets the lake numbers | owner |

P1 closed 2026-11-13 (owner sign-off in `docs/milestones/P1.md`).

## Tracked
- **River at a cube face edge** (REND-059, 2026-11-12): the store holds a segment crossing a face edge as two runs with a gap of about the H1 ratio in cells (a dry notch); the burn has the same dam, so drawn river and burned trench agree; the charts shear (x1.22 mid-edge, x1.41 at the corner). Fix (B): overlapping runs read by drawing and burn, width in metres through each face's metric: a store format change for the owner, with the face-seam work before the planet bake (REND-063 reserved).
- **Rock K residual** (#1216, seed 14 face 2): mountain 3 km relief p50 483 against 534 m (the band edge is 2/3 of it), elevation p50 +170-225 m from the K spread (f/(1+f) is concave), not the K mean; tune in the preview round. The carve adds about 1.55 GB at 12,288 cells: measure the peak against 8 GB (CON-21) on the next pass-1 run.
- **CI flake `gpu_stream_perf` lit-scene smoke** (2026-11-10): 240 s limit hit on Windows WARP C 5 times (11-06 to 11-13; the 5th at 254 s on #1472), each further one adds to the count, no limit change; times 130-240 s since the step at a600c56b (ground-detail-patch): slowness fits. TEST-106 adds times and phase marks: read them next time.
- **Older flakes** (WARP, loaded runner): `oracle_stream_smoke` (`load_running_max` 1, at least 2 expected, once; tighten its precondition, never weaken); `gpu_horizon` `a_slot_written_through_set_horizon_reads_back_as_written` (once, passes alone and on CI); `gpu_window` flight test (once, 7.5 s). On a recurrence keep the full output.
- **Fill depth across a mountain cube edge** (keep-pits-seams, #1201): the seam gate passed on a tile seam; no mountain cube edge was in the stored faces. Blocks the planet bake (I6); run with the integrated-preview re-run.
- **`test fast` 220-330 s** (BUILD-014): 320-329 s measured 2026-11-10 after the restart, machine busy.
- **Local `test gpu` worse since the display changes** (2026-11-09): solo on main 956-1,400 s: 2 WARP pins (`gpu_compact_blocks`, `gpu_water` lit frames), WARP aborts 0xc0000005 (intermittent), `gpu_skeleton` timeouts (diagnosed #1442: slow only under CPU oversubscription in the two pipelined loops), `gpu_preflight` smoke (its refusal of a bad `--out` must come under 8 s, `gpu_preflight.rs:22`; the test took 47 s under load on 2026-11-13). A `gpu_skeleton` timeout on CI goes to the architect. CI judges tier C.
- **Ocean square at orbit** (owner, 2026-11-03): at 12,767 km over 20 N 30 E after 240 s of flight, a light square in a ring of dark blotches on the sea; not reproduced headless. Hypotheses: clipmap shadow on water (`--no-shadow-march`, J view), the water-node cut or never-evicted WaterBook, the shadow's temporal history; needs the owner's F9 bundle.
- **Fine-level bound looseness** (SPIKE-86): below 1 km over steep land 5.5 % of real-store poses cap leaves at levels 16-21; tile bounds are 5-37x the sampled range. Fix with M9 R4's bounds pyramid (SPIKE-83) or a per-octave tile bound, with a tier-A bound-vs-sampled test.
- **Window checks need an unlocked desktop** (2026-10-30): on the RTX, present stalls 1.3-2.7 s or the device is lost while the session is locked or in modern standby; unlocked the dense sweep takes 26 s. The device losses since 10-18 most likely had this cause. NR handles it. Display off after 2 h (owner, 2026-10-31); the owner unlocks remotely for a batch, which the planner requests 07:00-22:00.
- **naga panic on a constant ground-detail octave** (SPIKE-67, 2026-10-31): naga 30.0.1 SPIR-V backend `block.rs:1022` "Expression [241] is not cached!" when the lit shader binds an octave to a constant (`let o = vec4<f32>(0.0);` in place of the 3 cm `gd_octave` call), Vulkan on the RTX; not reduced or reported upstream.
- **Detail blocks** (BUILD-027): 10 in `xtask/src/detail_blocks.txt`: split or shorten each, delete its line; no new blocks.
- **Frame tail** (SPIKE-44): `draw_plan_transient` 8-14 ms on a plan delivery (24-48 frames/min) + ~9 ms frame work drops a frame at 60 fps; ADR 0083 frame share.
- **Static memos** (CON-06): `global::landform_lattice` (generators) and the `class_inputs` cache (planet_cache, #1219) memoise pure per-definition functions; the bake's owner should pass them in.
- **Water cut keeps more nodes after the hill band** (#673): kept share 53-73 % (was 40-50 %); check the water pass's GPU time in the next perf window.
- **Authored items at scale** (SPIKE-29): cold plan over empty ground that sees a 5k/20k cluster 3.75/9.4 s; candidate: a per-cell bound aggregate (`spikes/edit-scale/results/results.md`).
- **Belt land and the polar relief** (`m9-generator-round-3`): belt land median 9.7 % against 13-23 % (seed 4 at 5.4 %, seed 14 at 7.9 %): gated with the convergent-only arc round; SPIKE-93 class bands approved (owner, 2026-11-02); seed 14's highest land 6.1 km against 7-9 km, its south polar land 31 % above 2 km. GEN-097 gates the p99 of the `k` steps; the class source may be wired into the bake.
- **WARP device removal with the compact format and the atmosphere**: test-only, accepted; the software smoke stays on the vertex format. Re-test when WARP, wgpu or Windows changes (Windows changed 2026-10-30).
- **Beach shading flashes on WARP** (36.8 % of shore-band pixels on the coastal descent): unchecked since REND-030; check with the next look review.
- **Coastal water limits** (ADR 0071 replaced ADR 0056's 4 km reach by connectivity, ATMO-019): (a) steep coastal ground cut under the level above `B(W)` does not conduct (ADR 0071 point 2; a fix is a slope-aware wet floor: a new store pass and coast goldens); (b) below the block level a band strip narrower than a parent's sample spacing can leave the parent dry (ATMO-019). Measure on the real store if the owner saw a dry coast.
- **ADR 0051 (superseded by 0055)**: point 1 (liquid parameters in the definition) awaits the planner's acceptance; point 6 (per-tile W storage) is open.
- **Ground-detail seams at LOD level boundaries** (2026-11-02): soil and moisture driver views are smooth at the seam pose, so the per-tile driver hypothesis is not supported; cause open.
