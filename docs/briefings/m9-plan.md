# M9 Realistic terrain: plan and iterations
Status: active; split into M9.1 and M9.2 (owner, 2026-11-06) (started 2026-11-01 with the owner's M9 decisions; plan written by opus-planner, 2026-11-02; re-planned 2026-11-04 after the erosion preview). Updated at every iteration boundary; an iteration closes with a line per item: done, carried or dropped.
Sources: `docs/briefings/m9-decisions.md`; ADR 0088, 0089, 0091, 0092 (proposed); `docs/roadmap.md`.

## Split (owner, 2026-11-06)
- **M9.1 Bake-ready:** everything that changes baked heights, the proof that it works together, and milestone-closure diligence. Ends with the owner's go for the planet bake (gate below).
- **M9.2 The baked world:** the bake and the runtime work on it (no re-bake needed). Ends with `cargo xtask accept M9`.

## M9.1 gate (the owner's go for I6)
1. Every M9.1 item below done or explicitly dropped by the owner.
2. The extended scorecard (TEST-091 to TEST-094, 3 tiles, 2 seeds, off-Earth presets) and the integrated preview (I4b) accepted by the owner.
3. The full-face all-driver run within budget (peak memory against 8 GB, CON-21; time per pass) and the cube-edge fill check passed.
4. Closure review findings fixed or accepted by the owner.

## M9.1 items
| # | Item | Lane | State (2026-11-06) |
|---|---|---|---|
| D | Drivers on main: keep-pits (ADR 0101), lithology and uplift (0102, 0103, #1216), climate, runoff and arid classes (0104, #1210, #1219, #1215), shelf (GEN-118, #1218) | explorer, crusader | merged |
| D2 | Pass-1 deposition and arid sinks (ADR 0105) | explorer | open |
| D3 | Coast distance transform, then the coastal process with its gate (ADR 0107) | crusader, explorer | open |
| D4 | Ice-history field, then glacial erosion with its gate (ADR 0106) | crusader, explorer | open |
| S | Smooth base as default (#1137): the owner decides at I4b; refreshed before the bake if yes | crusader | waiting |
| P1 | Extended scorecard and off-Earth presets (TEST-091 to TEST-094); runoff at the arid sites | spikes | after the reset |
| P2 | Full-face run with every driver (memory, time, a mountain cube edge) and one all-driver preview store opened by the viewer | explorer | after D2-D4 |
| P3 | Integrated preview (I4b) through the viewer's path; the preview tuning rounds (lithology, climate, class map, materials, glacier and coast gates) | explorer, crusader, owner | after P2 |
| P4 | Bake time re-projected on the restarted machine; disk space, write protection of the old stores, restart path for an interrupted bake | explorer, planner | after Sat 2026-11-07 |
| C1 | Independent review of the whole bake pipeline: key and version composition across drivers, determinism at full scale, memory | planner (Opus reviewer) | after D2-D4 |
| C2 | spec-guardian over the M9 deltas; specs synced; archived changes hold only proposals and deltas | planner, lanes | after D2-D4 |
| C3 | Earth-constant audit (SPIKE-103, ADR 0108); bake-relevant constants lifted to parameters with bit-identical defaults | spikes, lanes | after the reset |
| C4 | Housekeeping for M9.1: ADR statuses 0101-0108, domain glossary, learnings (at cap), this plan | planner | before the gate |
| C5 | Test health: near-limit tests (TEST-082, generate_bakes_the_class_source), flaky tests, CI job times | planner, lanes | ongoing |

## M9.2 items (after the bake; runtime, no re-bake)
| # | Item | Lane |
|---|---|---|
| I6 | Planet bake | explorer |
| I8b | Ground materials from the channels (rock, scree, soil, grass, wet ground) | crusader |
| I8a | Ground detail rest (2b 4c/4d) | crusader |
| I7 | Channels as per-point primitives (R5), behind a switch | tbd |
| I9 | Look against the reference on the bake; scorecard on the bake; walking-height live session | explorer, owner |
| E3 | No-degradation gate and speed envelope with the bake on; perf windows (REND-052, ground-detail and channel-read cost) | planner, explorer |
| B | k renamed to a per-class band amplitude; kinds 10/11 retired (SPIKE-99) | tbd |

## M9 exit (`cargo xtask accept M9`, after M9.2)
1. The baked world (ADR 0088) of the owner's seed passes the terrain scorecard (TEST-081 to TEST-084, TEST-091 to TEST-094) and the generator bands (TEST-082, TEST-087, TEST-088).
2. The owner accepts the look at the scorecard sites and at walking height (stills and a live session).
3. No-degradation gate and the speed envelope hold with the bake on.

## Done before the split
I1-I3 generator rounds, I5 bake code, I5a bake fixes, I5c runtime layer read, I8a ground detail first step (2a, 2b 4a-4b).

Out of M9 (terrain round 2 or M10): volcanoes (D5), glacial pass-2 forms, karst, lake water, cliff technology (ADR 0091), full flexural isostasy, dunes and gullies.

## Beside M9 in the same weeks (not M9 exit items)
| Item | Lane | State |
|---|---|---|
| Shadows ADR 0085: 5b-2 apply, PRs 6-9, the 2° perf window | crusader | #1084 blessed, #1085 acne fix |
| B4 rest: real-store pair, daily package | crusader | after 5b-2 |
| `node-buffer-delta` | crusader | after B4 |
| B9 perf-window process (SPIKE-95) | spikes | started 2026-11-02 |
| Relative compute shares (ADR 0092) | owner decision, then 4 PRs | proposed |

## Later, not M9 exit
- A runtime view of a pre-baked world from the snapshot (owner, 2026-11-02: interesting, a second step after I5b).
- The snapshot workflow is the base of the dormant "global authoring preview" item (status.md).

## Dates (estimates, not commitments)
- M9.1: about 1.5-2 lane-weeks after the Sunday 2026-11-08 reset; the gate and the bake about 2026-11-20 to 11-25, depending on the coastal and glacial gates.
- M9.2: from the bake; acceptance about a week after it.
