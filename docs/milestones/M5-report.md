# M5 Atmosphere & ocean: report (draft of 2026-10-18)

Draft by sonnet-crusader for opus-planner; docs only. Rows come from window 8 (`perf m5`, main 42508bf, 2026-10-18 00:29-00:50) and one dive re-run (main 211bf99, 01:46), judged by `cargo xtask accept M5` (generated report, explorer's `sonnet-m4/docs/milestones/M5-report.md`, not committed). The coast water rows wait for **window 9**. Everything marked TO FILL (planner) is the planner's: the verdict, the recommendations, the go / no-go.

## Verdict: TO FILL (planner)

Reading of the evidence today: hardware criteria **INCOMPLETE** (no gated row failed; two coast water rows are inconclusive at a clock below the floor); every other gated row passes. The automated steps pass.

## 1. Exit criteria

Architecture report §15, M5 row, plus the owner's M5 budgets of 2026-10-15 (ADR 0034, ADR 0036, ADR 0057).

| Criterion | Result | Evidence |
|---|---|---|
| Atmosphere LUTs match the CPU reference within tolerance (tier B) | PASS | ATMO-001, -005 to -008; `oracle_atmosphere` and the f64 references (`cargo xtask test` tiers A+B green in the accept run, 124.7 s) |
| Golden images for ground, limb and sunset views | PASS | 6 `sky`, 8 `lit`, 4 `water` goldens blessed (AUTO-M5 under ADR 0037 and ADR 0052); `tests/goldens/pending.txt` is empty |
| Atmosphere <= 1 ms (sum of sky view, aerial perspective, composite) | PASS | ground-1m 0.214, aerial-20km 0.228, orbit 0.645 (valid clocks); coast-2km 0.254 and coast-3m 0.234 pass at a low clock (ADR 0057) |
| Coastal flight without shoreline flicker | PASS on the metric | ATMO-017: coverage flicker 0.0000 % on a glide and a descent (budget 0.1 %, ADR 0053); the owner's look review of the videos is open (decision 1); the beach shading flash is tracked (section 4) |
| Terrain pass <= 8 ms on the lit path | PASS | 0.052 (orbit) to 1.321 ms (coast-3m, at a low clock) |
| Water (pass + scene copy) <= 2 ms, held views | INCOMPLETE | ground 0.088, aerial 0.160, orbit 0.160 pass; coast-2km and coast-3m: window 9 (window 8: 2.013 and 2.929 ms at P0 1312 and 1320 MHz, over budget below the floor, inconclusive) |
| Waited frame p99 <= 20 ms (any clock) | PASS | 2.26 to 7.53 ms on the five held views |
| Streaming scenes on the lit path (M4's criteria carried): dive, teleport-1m, teleport-2km, look-1m, look-2km | PASS | no blank or uncovered node; dive median 5.41, p99 13.04, max 21.70 ms, descent lag mean 0.426 (worst 2.82); teleports and looks in section 2 |
| Automated steps (check, spec-lint, tiers A+B, tier C on the software adapter, run-mode smokes, test fast) | PASS | `test fast` 23.0 s of tests on a whole-workspace run: reported, not judged (BUILD-003) |
| Coastal flights, compact against vertex, GPU span, dive-fast, dive-cold, host load | reported, never gated | section 2 |

## 2. Performance

### Held views (window 8; GPU clock "low" = under the 1,400 MHz floor)

| View | terrain <= 8 | atmosphere <= 1 | water <= 2 | p99 <= 20 | GPU clock, thermal |
|---|---|---|---|---|---|
| ground-1m | 1.109 | 0.214 | 0.088 | 3.46 | P0/P2 1500, 55-56 C |
| aerial-20km | 0.782 | 0.228 | 0.160 | 4.44 | P0 1515, 58-59 C |
| coast-2km | 1.036 (low) | 0.254 (low) | window 9 | 5.67 | P0 1312 low, 67-68 C |
| coast-3m | 1.321 (low) | 0.233 (low) | window 9 | 7.53 | P0 1320 low, 70-71 C |
| orbit | 0.052 | 0.645 | 0.160 | 2.26 | P0 1616, 63-64 C |

Software power cap (0x4) read in every view, 88 to 96 W. Run-to-run noise of the atmosphere and water sums at most 0.027 (0.065 ms). Per-pass medians are in the generated report; the largest are the water pass on the coast views (1.94 ms at 2 km, 2.86 ms at 3 m), the terrain pass at 3 m (1.32 ms) and the sky composite from orbit (0.56 ms).

### Coastal flights (report only; water pass + scene copy, ms)

| Flight | shore_filter 1 | shore_filter 0 | frame p99 (1 / 0) |
|---|---|---|---|
| glide | window 9 (window 8: 6.252 at P5 937 MHz) | window 9 (window 8: 3.062 at P0 1327) | 16.5 / 7.5 ms |
| descent | window 9 (window 8: 5.262 at P8 277 MHz) | window 9 (window 8: 2.972, valid) | 31.3 / 7.7 ms |

The filter-on flights ran at collapsed clocks in windows 7 and 8, so the on/off ratio is confounded. Window 6's filter-off glide was 1.79 ms at a valid clock.

### Streaming scenes on the lit path (frame verdicts valid; GPU time is not measured in the pipelined loop)

| Scene | median <= 16.6 | p99 <= 20 | max <= 40 | upload p99 <= 2 | detail lag | result |
|---|---|---|---|---|---|---|
| dive | 5.41 | 13.04 | 21.70 | 0.66 | descent mean 0.426 (<= 1), worst 2.82 (<= 3) | PASS (re-run, main 211bf99) |
| teleport-1m | 6.12 | 8.90 | 11.08 | 0.44 | tracked: under 1 level after 2.37 s (2 s), complete 6.40 s (9 s) | PASS |
| teleport-2km | 4.52 | 5.82 | 11.76 | 0.44 | tracked: 1.38 s (1 s), complete 3.97 s (4 s) | PASS |
| look-1m | 3.79 | 6.82 | 8.30 | 0.00 | rotation 0 | PASS |
| look-2km | 2.02 | 3.79 | 4.54 | 0.00 | rotation 0 | PASS |
| dive-fast (report) | 8.86 | 15.51 | 17.66 | 0.68 | descent mean 0.633, worst 2.78 | not gated |
| dive-cold (report) | 6.61 | 15.38 | 19.48 | 0.69 | descent mean 5.290, worst 11.28 | not gated |

RAM 0.52 to 1.60 GiB (<= 8), slots 231,090 of 231,090, at most 15,719 resident; terrain pools 0.18 to 0.31 GiB. The window-7 stalls (61 ms single frames, 4 s plans) are gone: STRM-024 (water fill helpers at the job's priority) and the coarse water summary (ADR 0058).

### Tools

`test fast`: 22.8 to 23.7 s of tests for the whole workspace on a quiet machine, 17 to 21 s for a one-crate selection (frame 18.1, generators 19.4 to 20.9, app 3.4 s); the 20 s budget gates a selection (BUILD-003).

## 3. What changed since the plan

- **ADR 0053** (accepted): the water pass's readings of ADR 0041: Gerstner waves with CPU-reduced phases, shoreline from `W - h_morph`, `shore_filter` default 1, flicker budget 0.1 %.
- **ADR 0054** (accepted): compact tiles in storage buffers (at most 128 MiB each) instead of texture atlases; amends ADR 0033 point 3.
- **ADR 0055** (accepted, owner): documentation model: specs hold current rules, ADRs record why, short terse writing; consolidation is queue item 10b.
- **ADR 0056** (accepted): water selection: the coastal band, tile water computed in the tile job, wave fade by pixel footprint; amends ADR 0053 points 4 and 5.
- **ADR 0057** (accepted, owner confirms here): the GPU cools to 60 C before each view, and a GPU figure within budget below the 1,400 MHz floor is a pass (over budget stays inconclusive); amends ADR 0036 point 2.
- **ADR 0058** (accepted, owner confirms here): a coarse level of the water field: a summary the bake writes once; coarse tiles read no block.

## 4. Tracked risks

| Risk | State | Next |
|---|---|---|
| Intermittent device loss in the lit dive | window 8: 2 of 3 dive runs lost the device (`Buffer with 'globals' label is invalid` at `write_buffer` right after "rig built", exit 101); 0 of 1 since REND-029 (main 211bf99), which passed every gated row. Both losses came after long GPU runs on a hot GPU. The cool start (ADR 0057) does not cover the streaming scenes | a recurrence prints `wgpu device lost (<reason>)` and the first error; then stop and investigate; the GPU-clock fix covers the lit views only |
| SPIKE-12: retiring the vertex tile format | recommended in early M6, about 8 coder days in four changes (slot v2 with one wide texel, pool arithmetic, a render floor at level 3 that changes only the orbit view, removal of about 750 lines); the slot grows 5.9 % and re-cuts ADR 0033's share table | owner decides retire or keep (decision 5) |
| Coast water budget | held coast views read 2.01 and 2.93 ms at a clock below the floor; flights 3.0 ms filter off, 5 to 6 ms filter on (collapsed clocks). CPU attribution (`docs/perf/water-attribution.md`): against window 6 the glide draws 3.7x and the descent 3.5x the water vertices for the same sea on screen; the selection alone is x2.95 and x2.74, the new place of the flights x1.26; after ADR 0056 every node of the plan carries water (about 30 % before). The GPU half (the old selection at the new place, the fade on and off) is open | window 9; a tighter selection than "every node" is the remedy the numbers suggest (explorer's code) |
| Terrain shading at the beach flashes | 36.8 % of the shore-band pixels flash by more than 25 % between frames on the descent with no water drawn (6.3 % on the glide), probably the albedo's beach band aliasing on a 1:50 beach | the owner's look review checks it in the videos |
| Clock below the floor on the coast views and flights | cause: the laptop GPU's software power cap and its temperature (54-59 C held 1,455 to 1,567 MHz, 66-77 C read 1,282 to 1,380), not a light load; mitigated by ADR 0057 | window 9 shows whether the cool start lifts the held views |
| Smaller items | WARP device removal with the compact format and the atmosphere (test-only, accepted); flaky `gpu_window` (one failure, then green); the cold start without the pre-fill (dive-cold lag 5.29 levels, report only) | status.md Tracked |

## 5. The owner's gate decisions

Evidence per decision; each recommendation is the planner's.

1. **Look review (ADR 0037 point 2).** Sign off the look or ask for changes, from the stills and the four 1440p videos (section 6), shore_filter 0 and 1. After the review, look changes are parameter changes in look files, re-blessed by `golden-review` as AUTO-L (TEST-036). Evidence: ground, limb, sunset and coast views in the goldens; flicker 0.0000 %; the beach flash (section 4). Recommendation: TO FILL (planner).
2. **`shore_filter` default** (ADR 0053): 1 (the filter on) or 0. Evidence: the filter-on flights cost about twice the filter-off ones in windows 7 and 8, but at collapsed clocks (confounded); the lit shore-flash measure (reported only) with the filter off then on: descent 41 % to 7 %, glide 6.5 % to 16.2 %. Recommendation: TO FILL (planner).
3. **ADR 0037 window** (`ADR_0037_M5_CLASSES`): keep blessing first goldens of `lit`, `sky` and `water` under policy, or empty the constant by a superseding ADR. Evidence: 18 goldens (6 sky, 8 lit, 4 water) blessed under it. Recommendation: TO FILL (planner).
4. **Cross-adapter golden agreement after M5**: drop the requirement that WARP, llvmpipe and the RTX agree on new goldens (ADR 0037, ADR 0052) and keep goldens as per-adapter regression checks; correctness stays with the bound tests. Evidence: the cost so far was two ADRs, three tool changes and a reversal. Recommendation: TO FILL (planner).
5. **Retire the vertex tile format** (SPIKE-12, `spikes/vertex-retirement/results/results.md`): about 8 coder days in early M6; the orbit view changes visibly (52 level-2 nodes become 181 level-3 nodes); the slot grows 5.9 %. Recommendation: TO FILL (planner).
6. **Planner decisions to confirm** (reversible, made while the owner was away): ADR 0056; ADR 0057 (amends owner-accepted ADR 0036 point 2); ADR 0058 and the coarse water summary chosen over a geometry-first stopgap; the `test fast` reading of BUILD-003 (the 20 s budget counts test time, a whole-workspace run is reported; a stale local `main` had made every run select the whole workspace). Recommendation: TO FILL (planner).
7. **ADR 0036 point 3** (proposed: a noise floor of 0.1 ms for the GPU noise rule). Evidence: the first window with GPU verdicts, run-to-run noise of the atmosphere and water sums at most 0.027 (0.065 ms), no view above the 10 % rule. Recommendation: TO FILL (planner).
8. **Proposed ADRs awaiting the owner**: ADR 0045 (bodies, bake resolution, storage budget; point 3 decided). Recommendation: TO FILL (planner).
9. **Viewer manual checks**: mouse look locally (SPIKE-11, about ten minutes at the laptop; the remote part was dropped); the keyboard feel after the changes of VIEW-004 (speed limits scale with the body's radius, bit-identical for the Earth; the ground clamp). Recommendation: TO FILL (planner).
10. **Go / no-go for M6** (milestone gate after M5): TO FILL (planner).

## 6. Where the material is

| What | Where |
|---|---|
| Stills (1280x720 JPEG, from the 1440p capture on main cc6c9d2) | `docs/milestones/M5/coast-descent-frame010-filter1.jpg`, `coast-glide-frame100-filter1.jpg`, `coast-glide-frame100-filter0.jpg` |
| Four 1440p flight videos (3.4 to 5.7 MB each, not in git) | `sonnet-m4/target/review/m5-flights/coast-glide-filter1.mp4`, `coast-glide-filter0.mp4`, `coast-descent-filter1.mp4`, `coast-descent-filter0.mp4` |
| All 240 frames per flight (PNG, 2560x1440) and `capture.txt` | `sonnet-m4/target/review/m5-flights/<flight>-filter<k>/frame_NNNN.png` |
| Contact sheets and six-frame strips | `sonnet-m4/target/review/water-inspection/` (`flight_*_strip.png`, `coast_*_atmosphere_ev14.png`, `orbit_atmosphere_ev14.png`, `lava_lake_lit.png`) |
| Generated acceptance report (all rows, per-pass table, host facts) | `sonnet-m4/docs/milestones/M5-report.md`, `sonnet-m4/target/review/accept-M5.md` (uncommitted) |
| Window notes | `docs/perf/window-7.md`, `docs/perf/window-8.md`, `docs/perf/water-attribution.md` |
| Dive re-run (the passing run) | `sonnet-m4/perf/logs/dive-lit.json` and `dive-lit.frames.txt` (untracked) |

The capture was made on main cc6c9d2 (2026-10-17 21:21-21:26), after the coastal band (ADR 0056) and the height-based `find_coast`; a later change of the water selection or `find_coast` needs a new capture (`cargo xtask capture flights`) before the owner looks.

## 7. Open before the verdict

Window 9 (coast water rows and flights; whether the cool start lifts the clocks), the planner's recommendations (section 5), the verifier's table over the milestone, the learnings of the milestone, and the owner's gate decision.
