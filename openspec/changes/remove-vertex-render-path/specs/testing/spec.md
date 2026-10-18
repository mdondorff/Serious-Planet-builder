## MODIFIED Requirements

### Requirement: TEST-011 Count-based budgets
Draw calls, triangles, resident tiles and upload bytes SHALL be derived from the plan and asserted against budgets in tier A tests on any machine, and tier C tests SHALL check that the renderer drew exactly the plan's draw calls and triangles and uploaded exactly the bytes of the tiles it was given (the plan's upload bytes are the resident tiles times the compact slot of GEN-025: header, climate block and texels, 9,168 B at cells 32; the former vertex model of 48 bytes per vertex is gone, ADR 0062) (allocated VRAM joins them when the renderer allocates budgeted pools); the counts of each standard view (the scripted cameras at the production parameters, including the production finest level, LOD-010) SHALL also be pinned per view, over the default world (the eroded field over a named bake) and over the reference field the goldens keep, so that a change that raises the nodes, triangles, resident tiles or upload bytes of any view fails with the view, the count and the pin, and a pin is raised only by a change that says why; the ground-1m counts at the reference finest level 26 SHALL stay pinned beside (SPIKE-07 stays reproducible); time-based budgets SHALL be measured only on the reference machine.

Verify: A
Status: active
Source: CON-20, M4 item 7d, LOD-010, GEN-025, ADR 0062

#### Scenario: Budget exceeded
- **WHEN** a frame plan exceeds its draw-call budget
- **THEN** the test fails and prints the count and the budget

#### Scenario: Pinned view
- **WHEN** the plan of a standard view has more nodes than its pin
- **THEN** the test fails naming the view, the count and the pin

#### Scenario: Renderer matches the plan
- **WHEN** a terrain plan is rendered
- **THEN** the draw calls, triangles and uploaded tile bytes the renderer reports equal the counts derived from the plan

#### Scenario: Default world pins
- **WHEN** the ten standard views are planned over the default world (eroded field, 256-cell bake) at the production finest level (22)
- **THEN** each view's nodes, triangles, resident tiles and upload bytes stay within its pin, the test names the bake the pins were made over, and ground-1m at level 26 equals its reference pin

#### Scenario: Compact upload bytes
- **WHEN** a compact tile is uploaded
- **THEN** the bytes counted are its slot (9,168 B at cells 32, GEN-025), and the pinned upload bytes of the standard views are re-pinned with that change and its reason

### Requirement: TEST-024 Store check in the smoke run
A smoke run over a stored bake (`test-render --perf-smoke --scene S --store DIR`, `cargo xtask perf stream S --smoke --store DIR`) SHALL first check the store read-only and print the result before any scene: the store, `GLOBAL_VERSION` and `MESH_VERSION`, the ocean channel read at a fixed grid of 288 directions (12 latitudes by 24 longitudes) with the counts of sea and land, and the number of tiles of levels F to L (`--prefill-levels L`, default 4; F is the render floor, LOD-011, default 3) the tile pack holds, each present tile read and decoded through the pack's checks. The run SHALL fail before any scene, naming the problem and the command that fixes it, when the store has no ocean channel, when the channel reads all land or all sea on the grid, or when the pack lacks tiles of levels F to L (the compact form, the only form). The check SHALL write nothing to the store, and the smoke run is labelled as no measurement.

Verify: A, C
Status: active
Source: opus-planner request of 2026-10-17 (M5 item 8, before window 6), TEST-019, STRM-021, GEN-022, ADR 0045 point 3, LOD-011, ADR 0062

#### Scenario: A complete store
- **WHEN** the smoke run reads a stored bake pre-filled to level 3 with `--prefill-levels 3`
- **THEN** it prints `store check (smoke, read-only, NOT a measurement)`, `ocean channel: present, 288 directions read, N sea and M land`, `compact form 384 of 384 present` and `store check ok`, runs its scenes, and the listing of the store directory (names, sizes, modification times) is unchanged afterwards

#### Scenario: An incomplete pack
- **WHEN** the same store is smoked with `--prefill-levels 4`
- **THEN** the run fails with `store check FAILED`, the count `of 1,920 compact tiles of levels 3-4`, `MESH_VERSION` and the `generate --prefill-dir` command, before any scene runs, and the store is unchanged

#### Scenario: A store without a usable ocean
- **WHEN** the channel is missing, or reads all land or all sea
- **THEN** the problems name `no ocean channel` and `generate --global-dir`, or `reads land on all 288` or `reads sea on all 288`

### Requirement: TEST-025 M5 lit perf scenes
The performance harness SHALL provide the scene `lit` (`test-render --perf --scene lit`, `cargo xtask perf lit`) that measures the lit path (the look's sun, the atmosphere, the water pass, adaptive exposure, the tone map) at 1440p over the views ground 1 m, aerial 20 km, coast 2 km, coast 3 m and orbit (the viewer's cameras at 20 N 30 E and two cameras 2 km and 3 m above the water at the first shoreline east of 30 E whose sea reaches 2 km out and land 500 m in, found by `planet_frame::litscenes::find_coast`) and over the two coastal flights (`coast-glide`: 6 m over the water from 400 m out, 228 m towards the shore; `coast-descent`: 300 m to 4 m, log-spaced, 1,200 m out closing to 500 m; 240 cameras each, `--flight-frames N`), the flights once with `water.shore_filter` at the look's default and once at the other setting (`<flight>@filter0` or `@filter1`, for inspection). It SHALL refuse a world without a coast or without the ocean channel, naming the remedy. Each held view SHALL be planned and uploaded once, drawn in waited timed frames (`render(.., true)`) after `enable_lit`, the atmosphere and the water pass were enabled and warmed with an empty water plan, the exposure was seeded at the view and the timed warm-up frames ran (REND-017: no first-use work in a measured frame; an empty water plan draws no water, so the first frame with water nodes of a streaming scene on the lit path may pay the water pass's first use, which the held views and flights absorb in their warm-up frames), brought to the GPU's loaded state (ADR 0034, ADR 0036) and measured in three runs of `--frames` frames with the run-to-run noise rule and its one retry (ADR 0020); each flight camera SHALL be planned, its new tiles uploaded, drawn once untimed and twice timed, in one run. The log of a run SHALL carry, besides the waited frame time, the metrics `cpu_ms`, `gpu_ms` (the terrain pass, as in the terrain scene), `gpu_water_ms`, `gpu_sky_view_ms`, `gpu_aerial_perspective_ms`, `gpu_sky_composite_ms`, `gpu_exposure_histogram_ms`, `gpu_exposure_adapt_ms`, `gpu_tonemap_ms`, `gpu_scene_copy_ms`, the per-frame sums `gpu_atmosphere_ms` (sky view, aerial perspective and composite) and `gpu_water_total_ms` (water pass and scene copy) and `gpu_span_ms` (REND-028), and the counts nodes, draw calls, water nodes and draws, tile format and the lit and atmosphere flags; the static tables' upload has no row (a queue write on a body's load, not a pass of a steady frame) and the console says so. A static view SHALL be judged, when it is valid for GPU figures (ADR 0036), against the owner's budgets of 2026-10-15: the median of the atmosphere sum at most 1 ms, the median of the water sum at most 2 ms, 2.2 ms for the held view coast 3 m (ADR 0060; a view with no water node says `not in view`), and the terrain pass at most 8 ms, and, at any clock state, the p99 of the waited frame at most 20 ms; the GPU span of the lit frame is printed as a reported figure (timestamps serialise the passes, so it is an upper bound) and the owner's frame median of 16.6 ms is the streaming scenes' with `--lit`; a view not valid for GPU figures prints its figures with `NOT VALID for GPU budgets (<state>)` and no verdict; a flight prints the same rows as reported (`over (not gated)` when above) and gates nothing; a flight's clock samples are taken after its warm-up. The console SHALL print per view one row of every pass's median, p99 and maximum, the host load (TEST-023), and for the atmosphere and water sums the run-to-run noise, its absolute spread and whether the proposed floor of ADR 0036 point 3 (also above 0.1 ms) would have kept the view noisy (recorded, not applied). `--views a,b,c` SHALL restrict the views (a partial log is not a window and never becomes a baseline). The former scene `lit-formats` (the same views in both tile formats) is gone with the vertex format (ADR 0062); logs of windows 1 to 11 may hold one. The console SHALL also print how many frames stand behind the atmosphere and water sums (a frame that lost a pass drops out of the median). The streaming scenes SHALL take `--lit` (`perf stream NAME|all --lit`): the same scripts, the same pipelined loop and the same verdicts of TEST-018 and TEST-019 with each frame drawn lit with the atmosphere and the water of its camera (the lit plan and the water plan inside the timed frame, the lit path warmed before the settle), the log named `<scene>-lit.json` in a log directory, the path `stream-lit/<scene>` and `protocol.field` ending in `; lit path (atmosphere, water)`, so that a lit log is never compared with a debug-path log; GPU time stays not measured in them. A smoke run (`--perf-smoke`, `perf lit --smoke`) SHALL run the same path on any adapter at 640 by 360 for the lit scenes, print the pass rows and no budget rows, run the compact format on a software adapter and never be a measurement. `cargo xtask perf m5 --store DIR [--out DIR]` SHALL run `lit` and the seven streaming scenes with `--lit` in that order, each under the readiness gate and the streaming scenes each in their own process, continue after a failing part and exit non-zero if one failed.

Verify: A, C
Status: active
Source: M5 item 8 (status.md), the owner's M5 decisions of 2026-10-15 (atmosphere 1 ms, water 2 ms, frame 16.6 ms and 20 ms), ADR 0060 (coast 3 m water 2.2 ms), ADR 0034, ADR 0036, ADR 0041, ADR 0053, ADR 0020, REND-017, REND-028, TEST-012, TEST-019, TEST-023

#### Scenario: Budget rows of a static view
- **WHEN** a view valid for GPU figures has an atmosphere sum of exactly 1 ms and a water sum of exactly 2 ms, and another has 1.5 ms and 2.5 ms
- **THEN** the first is within both budgets and the second fails the atmosphere and water rows, and the GPU span row is reported in both

#### Scenario: The water budget of coast 3 m
- **WHEN** the held view coast 3 m has a water sum of exactly 2.2 ms, of 2.21 ms, and another held view has 2.1 ms (all valid for GPU figures)
- **THEN** coast 3 m is within its budget at 2.2 ms and over it at 2.21 ms, the other view fails its 2 ms budget, and the row prints the budget it was judged against

#### Scenario: A view below the clock floor
- **WHEN** a view's clock samples read 1,150 MHz in P2 and its waited frame p99 is 31 ms
- **THEN** every GPU row prints its figure with `NOT VALID for GPU budgets (P2 1150 MHz ...)` and no verdict, and the frame row is OVER at that clock state

#### Scenario: Water and atmosphere not in the view, and flights
- **WHEN** a view has no water node, or the atmosphere is not drawn, and a flight is over a budget
- **THEN** the row says `not in view` and fails nothing, and the flight's row says `over (not gated)`

#### Scenario: The coast and the flights
- **WHEN** a world has sea on the western half of a parallel and land on the eastern half
- **THEN** the coast is the first sea-to-land crossing east of 30 E with the heading towards the land, the glide holds 6 m over the water and the descent runs from 300 m to 4 m with any frame count, and a world without a shoreline, or with a pond whose sea does not reach 2 km out, has no coast

#### Scenario: Smoke on the software adapter
- **WHEN** the scene `lit` and a streaming scene with `--lit` run in smoke mode over an in-memory bake
- **THEN** every view and both flights with both shore-filter settings print a pass row and a waited frame time, the log carries the per-pass metrics where the adapter times its passes, the lit scenes print no budget row, the streaming log is `stream-lit/<scene>` with the lit field, and `--views` with an unknown name, `--views` or `--flight-frames` with another scene and `--lit` with the terrain scene are refused

### Requirement: TEST-026 Milestone M5 acceptance
`cargo xtask accept M5` SHALL run the automated steps of `accept M4` (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step; the discrete-GPU tier C run stays on the manual list so that acceptance may sit beside the architect's window) and SHALL judge M5's hardware criteria only from the logs of a valid M5 window (`lit.json` and `<scene>-lit.json` with `<scene>-lit.frames.txt` for the seven streaming scenes, in `perf/logs/` or `--logs DIR`: what `cargo xtask perf m5` writes), never from a measurement of its own. The gated rows SHALL be, for each of the held views ground 1 m, aerial 20 km, coast 2 km, coast 3 m and orbit of `lit.json` (the median of the runs' medians, the largest p99): the atmosphere (sky view, aerial perspective and composite summed per frame) at most 1 ms, the water (pass and scene copy) at most 2 ms (2.2 ms for the view coast 3 m, ADR 0060), the terrain pass at most 8 ms, each only when the view is valid for GPU figures (ADR 0036) and the session fit for GPU verdicts, and the p99 of the waited frame at most 20 ms at any clock state; and, for the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km` on the lit path, every verdict of TEST-021 recomputed by the harness's own functions from the per-frame logs. The report-only streaming logs (`dive-fast`, `dive-cold`) are shown only when they come from the source, adapter and power scheme of `lit.json`. The terrain pass's 8 ms is M2's budget carried to the lit path (ADR 0034 point 4); the p99 of the waited frame is judged at any clock state (ADR 0034 as amended by the owner, 2026-10-15). The coastal flights (both shore-filter settings), the compact against vertex ratios of an older window's `lit-formats.json` (the scene is gone, ADR 0062), the GPU span of a waited frame, `dive-fast` and `dive-cold` on the lit path and the host load SHALL be reported and never gated. The outcome SHALL be PASS, FAIL (a gated row over its budget) or INCOMPLETE (no row failed but a gated GPU row is not valid for GPU budgets or not measured, or a coast view has no water node in some run), and only PASS exits zero. The command SHALL REFUSE, listing every reason, print the commands of the window and the acceptance, write no report and exit non-zero, when a gated log is missing, a session is invalid for frame verdicts (the readiness gate, throttling, smoke, another adapter or power scheme, the owner's override, noise: `planet_perf::validity`), the frame size is not 2560 by 1440, the source is not the stored production bake, `protocol.field` of `lit.json` or a streaming log is not the production field drawn on the lit path with compact tiles (each log states its tile format), a held view has fewer than three runs (a partial log, `--views`, is not a window), a streaming log is old-format, partial or not one scene's own process, the gated dive does not say pre-filled to level 4 or deeper, or the logs come from different sources, adapters or power schemes. With valid logs it SHALL write `docs/milestones/M5-report.md` (as `target/review/M5-report.generated.md` when that file exists or an automated step failed) holding the verdict, the gated and the reported rows, the per-pass table, the facts of the window and a slot marked TO FILL for every finding no log carries (the look review of ADR 0037, the M4 budgets on the lit path, the orbit atmosphere cost, the water pass, the compact against vertex ratio, the per-pass costs, ADR 0036 point 3, the shore filter in motion, the store regeneration, the host load, the proposed ADRs, the ADR 0037 window, the viewer's manual checks, the recommendation).

Verify: A
Status: active
Source: M5 item 8 and item 10 (status.md), the owner's M5 budgets of 2026-10-15, ADR 0034, ADR 0036, ADR 0037, TEST-021, TEST-025

#### Scenario: A valid window
- **WHEN** the logs come from one valid session over the production store with every gated row within its budget
- **THEN** the outcome is PASS, the report is rendered with its gated, reported and per-pass sections, and every finding slot is marked TO FILL

#### Scenario: A row over its budget
- **WHEN** the orbit view's atmosphere median is 1.3 ms, or a streaming scene on the lit path has a frame median above 16.6 ms
- **THEN** the outcome is FAIL naming the view or scene and the measure, and the flights over a budget change nothing

#### Scenario: Rows that cannot be judged
- **WHEN** a view's clock samples read 1,150 MHz, or the session started with the GPU in P8, or a coast view has no water node
- **THEN** the outcome is INCOMPLETE naming each row (`NOT VALID for GPU budgets (...)` or `a coast view must hold water`), the frame rows are still judged, and the command exits non-zero

#### Scenario: Logs that cannot be trusted
- **WHEN** a session is throttled, the frame size is 640 by 360, the bake is in memory, the tiles are not compact (in `lit.json` or in a streaming log), a view has one run, a view is missing, a stream's source differs, or `lit.json` and a gated stream log are missing
- **THEN** the command refuses with every reason, the commands of the window and `cargo xtask accept M5`, writes nothing and exits non-zero

#### Scenario: Optional parts
- **WHEN** the flights, `dive-fast` and `dive-cold` are absent
- **THEN** the window is judged and the report says each was not run

