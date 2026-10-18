# M5 Atmosphere & ocean: report (2026-10-18)

Drafted by sonnet-crusader, completed by opus-planner. Streaming rows: window 8 (`perf m5`, main 42508bf, 2026-10-18 00:29-00:50) and one dive re-run (main 211bf99, 01:46). Held views and flights: window 10 (`perf lit`, `perf lit-formats`, main a4479cb; `docs/perf/window-10.md`), judged by `accept M5` with ADR 0060 (#291), verdicts derived from the logged data. Sections 1 and 2 below show window 9's figures; window 10's differ by at most 0.03 ms (table below).

## Verdict (opus-planner): hardware criteria PASS; GO for M6 recommended

- `cargo xtask accept M5` (window 10 logs, ADR 0060): **PASS** on every gated row; every automated step green on a quiet run (tiers A+B 81.6 s, tier C software 117 of 117, smokes, `test fast` 24.1 s whole workspace, reported).
- The owner's decisions 11 and 12 are taken (ADR 0060, owner, 2026-10-18): a P0 view limited only by the power cap is GPU-valid; coast-3m water budget 2.2 ms.
- Open for the gate: decisions 1, 2, 13 (look review, `shore_filter`, `/bless` of #275) and 3-8, 10, 14. Decisions 9 and 15 (at the laptop) are postponed by the owner and do not block M6.

| Window 10 view | terrain <= 8 | atmosphere <= 1 | water | p99 <= 20 |
|---|---|---|---|---|
| ground-1m | 1.079 | 0.209 | not in view | 3.54 |
| aerial-20km | 0.788 | 0.235 | 0.112 / 2 | 2.94 |
| coast-2km | 1.058 | 0.258 | 1.863 / 2 | 5.69 |
| coast-3m | 1.350 | 0.236 | 2.170 / 2.2 | 6.58 |
| orbit | 0.052 | 0.679 | 0.168 / 2 | 2.94 |

Known gap: the exact dry-tile cut (#286) does not yet take effect in the lit scene path (window 10's node counts equal window 9's); sonnet-m4 is fixing it. It is margin, not a criterion.

## 1. Exit criteria

Architecture report §15, M5 row, plus the owner's M5 budgets of 2026-10-15 (ADR 0034, ADR 0036, ADR 0057).

| Criterion | Result | Evidence |
|---|---|---|
| Atmosphere LUTs match the CPU reference within tolerance (tier B) | PASS | ATMO-001, -005 to -008; `oracle_atmosphere` and the f64 references (`cargo xtask test` tiers A+B green in the accept run, 124.7 s) |
| Golden images for ground, limb and sunset views | PASS | 6 `sky`, 8 `lit`, 4 `water` goldens blessed (AUTO-M5 under ADR 0037 and ADR 0052); `tests/goldens/pending.txt` is empty |
| Atmosphere <= 1 ms (sum of sky view, aerial perspective, composite) | PASS | window 9: ground-1m 0.219, orbit 0.654 (valid clocks); aerial-20km 0.243, coast-2km 0.259, coast-3m 0.238 pass at a low clock (ADR 0057) |
| Coastal flight without shoreline flicker | PASS on the metric | ATMO-017: coverage flicker 0.0000 % on a glide and a descent (budget 0.1 %, ADR 0053); the owner's look review of the videos is open (decision 1); the beach shading flash is tracked (section 4) |
| Terrain pass <= 8 ms on the lit path | PASS | window 9: 0.051 (orbit) to 1.357 ms (coast-3m, at a low clock) |
| Water (pass + scene copy) <= 2 ms, held views | INCOMPLETE | window 9: aerial 0.116, orbit 0.160 pass; coast-2km 1.871 passes at a low clock; **coast-3m 2.175 at P0 1,290 MHz, inconclusive** (window 8: 2.929, before the horizon cut ATMO-020) |
| Waited frame p99 <= 20 ms (any clock) | PASS | window 9: 2.12 to 6.55 ms on the five held views |
| Streaming scenes on the lit path (M4's criteria carried): dive, teleport-1m, teleport-2km, look-1m, look-2km | PASS | no blank or uncovered node; dive median 5.41, p99 13.04, max 21.70 ms, descent lag mean 0.426 (worst 2.82); teleports and looks in section 2 |
| Automated steps (check, spec-lint, tiers A+B, tier C on the software adapter, run-mode smokes, test fast) | PASS | `test fast` 23.0 s of tests on a whole-workspace run: reported, not judged (BUILD-003) |
| Coastal flights, compact against vertex, GPU span, dive-fast, dive-cold, host load | reported, never gated | section 2 |

## 2. Performance

### Held views (window 9, after the horizon cut ATMO-020 and the cool start; GPU clock "low" = under the 1,400 MHz floor)

| View | terrain <= 8 | atmosphere <= 1 | water <= 2 | p99 <= 20 | GPU clock | water nodes |
|---|---|---|---|---|---|---|
| ground-1m | 1.136 | 0.219 | not in view | 3.48 | P0 1477 | 0 of 2,417 |
| aerial-20km | 0.799 (low) | 0.243 (low) | 0.116 (low) | 2.92 | P0 1365 | 56 of 913 |
| coast-2km | 1.060 (low) | 0.259 (low) | 1.871 (low, pass) | 5.32 | P0 1245 | 1,082 of 1,410 |
| coast-3m | 1.357 (low) | 0.238 (low) | **2.175 (low, inconclusive)** | 6.55 | P0 1290 | 1,181 of 2,588 |
| orbit | 0.051 | 0.654 | 0.160 | 2.12 | P0 1537 | 48 of 52 |

The cool start reached 60 C before every view. The coast views still sit at P0 1,245-1,365 MHz with the software power cap as the only throttle reason: the laptop's sustained clock under that load, not heat or idling. Water attribution (`docs/perf/water-attribution.md`): the footprint fade costs nothing; the rise since window 6 was water nodes that put no pixel on screen; the horizon cut removed 21-54 % of them with identical sea pixels; a terrain-occlusion cut would remove under 1 % (not worth building); an exact dry-tile cut (2-3 coder days) would bring coast-3m to about 2.0 ms at 1,290 MHz.

### Coastal flights (report only; window 9; water pass + scene copy, ms)

| Flight | shore_filter 1 | shore_filter 0 | frame p99 (1 / 0) |
|---|---|---|---|
| glide | 5.505 at P5 712 MHz | 2.281 at P2 1327 | 16.1 / 7.2 ms |
| descent | 5.898 at P8 247 MHz | 2.480 at P0 1365 | 31.0 / 7.0 ms |

With the filter on, the flights fall to P5/P8 clocks in every window (7, 8, 9) and the descent's frame p99 reaches 31 ms; with the filter off they hold P0/P2 and 7 ms. The filter's own GPU cost is not separated.

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
- **ADR 0059** (accepted, owner confirms here): noise floors, 0.1 ms per pass and 0.5 ms per frame total; accepts and amends ADR 0036 point 3.
- **ATMO-020**: water nodes beyond the sea horizon or above W plus the wave height are not drawn; sea pixels unchanged on the 16384 store.
- **REND-030** (#275, waits for `/bless`): the shore ramp width follows the mesh's height uncertainty per vertex; the terrain's own beach flash goes from 36.8 % to 0.0 %.
- **Tooling**: `accept M5` derives every verdict from the logged data, so a rule change re-judges old logs; `test fast` selects against `origin/main`.

## 4. Tracked risks

| Risk | State | Next |
|---|---|---|
| Device loss on the RTX | window 8: 2 of 3 measured dive runs lost the device; 0 of 1 since REND-029. Full parallel `test gpu --real` runs (about 20 GPU processes) lost it in 3 of 3 runs with the printed reason "driver implementation is at fault"; the same tests pass alone. 902 `nvlddmkm` Event 153 entries at every GPU-heavy period; driver 591.86 (2026-01-20). The streaming scenes now cool to 60 C first | a cap of 4 concurrent real-GPU test processes (sonnet-crusader, verification pending); a recurrence in a measured run prints the reason; a driver update is the owner's step (decision 15) |
| SPIKE-12: retiring the vertex tile format | recommended in early M6, about 8 coder days in four changes (slot v2 with one wide texel, pool arithmetic, a render floor at level 3 that changes only the orbit view, removal of about 750 lines); the slot grows 5.9 % and re-cuts ADR 0033's share table | owner decides retire or keep (decision 5) |
| Coast water budget | coast-3m 2.175 ms at P0 1,290 MHz after the horizon cut (section 2) | owner's decision 12 |
| Terrain shading at the beach flashes | fixed by REND-030 (#275): 0.0 % on both flights, asserted <= 0.5 %; a first version stepped at LOD borders and was corrected (per-vertex width, a border-continuity test) | `/bless` of its 7 goldens (decision 13) |
| Clock below the floor on the heavy views | the laptop's software power cap holds P0 at 1,245-1,365 MHz under the coast views' load even after the cool start; `nvidia-smi -lgc` would not lift a power-capped clock | owner's decision 11 |
| Smaller items | WARP device removal with the compact format and the atmosphere (test-only, accepted); flaky `gpu_window` (one failure, then green); the cold start without the pre-fill (dive-cold lag 5.29 levels, report only) | status.md Tracked |

## 5. The owner's gate decisions

Evidence per decision; each recommendation is the planner's.

1. **Look review (ADR 0037 point 2).** Sign off the look or ask for changes, from the four 1440p videos `m5-flights-v3` (section 6), shore_filter 0 and 1; they include the beach-flash fix (#275) and the horizon cut. After the review, look changes are parameter changes in look files, re-blessed by `golden-review` as AUTO-L (TEST-036). Evidence: ground, limb, sunset and coast views in the goldens; coverage flicker 0.0000 %; beach flash 0.0 %. Recommendation: watch the descent with filter 0 and 1 side by side; the planner's check of the contact sheet found no defect.
2. **`shore_filter` default** (ADR 0053): 1 (on) or 0. Evidence: with the filter on the flights fall to P5/P8 clocks in windows 7, 8 and 9 and the descent's frame p99 reaches 31 ms (7 ms with it off); the lit shore-flash measure, off then on: descent 41 % to 7 %, glide 6.5 % to 16.2 %; the terrain's own flash is now 0.0 % either way (REND-030). Recommendation: **0** unless the videos show the filter is needed; the cost signal is consistent and the benefit is mixed.
3. **ADR 0037 window** (`ADR_0037_M5_CLASSES`): keep blessing first goldens of `lit`, `sky` and `water` under policy, or empty the constant by a superseding ADR. Evidence: 18 goldens blessed under it. Recommendation: **close it** after the look review; later look changes go through AUTO-L and new first goldens follow ADR 0017. **Decided (owner): closed, ADR 0061; the constant is empty (#298).**
4. **Cross-adapter golden agreement after M5**: drop the requirement that WARP, llvmpipe and the RTX agree on new goldens (ADR 0037, ADR 0052); goldens stay per-adapter regression checks; correctness stays with the bound tests. Evidence: two ADRs, three tool changes and a reversal so far. Recommendation: **drop it**. **Decided (owner): dropped, ADR 0061 (#298).**
5. **Retire the vertex tile format** (SPIKE-12): about 8 coder days; the orbit view changes visibly (52 level-2 nodes become 181 level-3 nodes); the slot grows 5.9 %. Recommendation: **retire it first in M6**, before the edit path (SPIKE-13) touches tile refresh, so that path is built once, for one format.
6. **Planner decisions to confirm** (reversible, made while the owner was away): ADR 0056; ADR 0057 (amends owner-accepted ADR 0036 point 2); ADR 0058; ATMO-020; the `test fast` reading of BUILD-003 (the 20 s budget counts test time; a whole-workspace run is reported). Recommendation: confirm.
7. **ADR 0059** (noise floors 0.1 ms per pass, 0.5 ms per frame total; accepts and amends ADR 0036 point 3). Evidence: window 9's orbit spread was 0.23-0.27 ms on a 2 ms frame, which invalidated two sessions under the 10 % rule alone. Recommendation: confirm.
8. **ADR 0045** (bodies, bake resolution, storage budget; point 3 decided): stays proposed; its own trigger is the multi-body milestone, or M6 if the editor needs its point 7. Recommendation: no decision now.
9. **Viewer manual checks** at the laptop (about 15 minutes): mouse look (SPIKE-11); the keyboard feel after VIEW-004 (speed scales with the body's radius, bit-identical for the Earth; the ground clamp). Recommendation: do them at the gate session; mouse look then becomes queue item 1b.
10. **Go / no-go for M6**: recommendation **GO**, once decisions 11 and 12 close the coast-3m row.
11. **Clock rule under the power cap** (ADR 0036 point 2 and ADR 0057): the heavy views hold P0 at 1,245-1,365 MHz with the software power cap as the only throttle reason, which ADR 0036 point 1 calls normal. Recommendation: a view at P0 whose only throttle reason is the power cap is GPU-valid; `accept M5` then re-judges the logged windows without a new run. **Decided (owner): ADR 0060.**
12. **Coast-3m water budget**: 2.175 ms against 2 ms (decision 11 makes it a fail rather than inconclusive). Options: (A) accept about 2.2 ms for this worst-case view, frame p99 6.6 ms, and build the exact dry-tile cut early in M6 (2-3 coder days, about 2.0 ms); (B) build the cut before the gate (2.0 ms at 1,290 MHz, no margin); (C) keep the row open. Recommendation: **A**. **Decided (owner): A, 2.2 ms, ADR 0060.**
13. **`/bless` of #275's 7 goldens** (REND-030, coastline pixels in `lit`, `sky` and `water` views, 3 to 91 pixels each). Recommendation: bless after the look review. `golden-review` (report mode, #275 at its merge with main of 2026-10-18; RTX local, WARP and llvmpipe from CI run 39099050211) escalates all 21 candidates (7 goldens on each of the 3 adapters) under ADR 0017 ("is a lit / sky / water view"; all declared in the change's proposal; every adapter agrees on the changed pixels, which lie on the coastline only): `lit_aerial20km_albedo_ev15` 90 px (max delta 136), `lit_aerial20km_ev15` 91 (136), `lit_orbit_ev15` 4 (151 to 152), `lit_orbit_ev_bands` 3 (192 to 223), `sky_aerial20km_ev15` 90 (96), `sky_orbit_ev15` 3 to 4 (117 to 118), `water_class_orbit` 1 to 7 (64 to 118). The triptychs (golden, candidate, diff) are in `target/review/golden-review/<adapter>/` of the sonnet-crusader worktree (`cargo xtask golden-review --proposal openspec/changes/shore-ramp-width/proposal.md --extra <CI review-Linux candidates> --extra <CI review-Windows candidates>` writes them again). The first CI run of the branch gave WARP and llvmpipe candidates for the sky goldens only (fail-fast cancelled 78 tests: fixed by #284, `--no-fail-fast`).
14. **egui fonts** (SPIKE-14): `default_fonts` fails `cargo deny` (OFL-1.1, Ubuntu-font-1.0). Recommendation: ship one font under an allowed licence (for example Roboto, Apache-2.0); no change to the allow-list. **Decided (owner): an allowed-licence font.**
15. **NVIDIA driver** 591.86 (2026-01-20): an update is the owner's step (admin) and a cheap test against the device losses; not claimed as the fix.

## 6. Where the material is

| What | Where |
|---|---|
| Stills (1280x720 JPEG, from the 1440p capture on main cc6c9d2) | `docs/milestones/M5/coast-descent-frame010-filter1.jpg`, `coast-glide-frame100-filter1.jpg`, `coast-glide-frame100-filter0.jpg` |
| Four 1440p flight videos for the look review (3.4 to 5.7 MB each, not in git; #275's branch merged with main f02bc9f, RTX, 2026-10-18 05:49-05:56) | `sonnet-m4/target/review/m5-flights-v3/coast-glide-filter1.mp4`, `coast-glide-filter0.mp4`, `coast-descent-filter1.mp4`, `coast-descent-filter0.mp4` |
| All 240 frames per flight (PNG, 2560x1440) and a six-frame contact sheet | `sonnet-m4/target/review/m5-flights-v3/<flight>-filter<k>/frame_NNNN.png`, `sonnet-m4/target/review/m5-flights-v3-sheet-a.jpg` |
| Superseded captures (do not use) | `m5-flights` (before the horizon cut and REND-030), `m5-flights-v2` (REND-030's first version, stepped at LOD borders) |
| Contact sheets and six-frame strips | `sonnet-m4/target/review/water-inspection/` (`flight_*_strip.png`, `coast_*_atmosphere_ev14.png`, `orbit_atmosphere_ev14.png`, `lava_lake_lit.png`) |
| Generated acceptance report (all rows, per-pass table, host facts) | `sonnet-m4/docs/milestones/M5-report.md`, `sonnet-m4/target/review/accept-M5.md` (uncommitted) |
| Window notes | `docs/perf/window-7.md`, `docs/perf/window-8.md`, `docs/perf/water-attribution.md` |
| Dive re-run (the passing run) | `sonnet-m4/perf/logs/dive-lit.json` and `dive-lit.frames.txt` (untracked) |

The stills in `docs/milestones/M5/` come from the earlier capture (main cc6c9d2) and show the coast before REND-030; the videos above are current.

## 7. Open before the verdict

The owner's decisions (section 5), then `/bless` of #275 and its merge, and `accept M5` re-judged under decision 11.
