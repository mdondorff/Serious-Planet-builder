## ADDED Requirements

### Requirement: TEST-026 Milestone M5 acceptance
`cargo xtask accept M5` SHALL run the automated steps of `accept M4` (format, lints and layering, spec-lint, tiers A and B, tier C on the software adapter, the three run-mode smokes, the `test fast` timing step; the discrete-GPU tier C run stays on the manual list so that acceptance may sit beside the architect's window) and SHALL judge M5's hardware criteria only from the logs of a valid M5 window (`lit.json`, `lit-formats.json`, and `<scene>-lit.json` with `<scene>-lit.frames.txt` for the seven streaming scenes, in `perf/logs/` or `--logs DIR`: what `cargo xtask perf m5` writes), never from a measurement of its own. The gated rows SHALL be, for each of the held views ground 1 m, aerial 20 km, coast 2 km, coast 3 m and orbit of `lit.json` (the median of the runs' medians, the largest p99): the atmosphere (sky view, aerial perspective and composite summed per frame) at most 1 ms, the water (pass and scene copy) at most 2 ms, the terrain pass at most 8 ms, each only when the view is valid for GPU figures (ADR 0036) and the session fit for GPU verdicts, and the p99 of the waited frame at most 20 ms at any clock state; and, for the scenes `dive`, `teleport-1m`, `teleport-2km`, `look-1m` and `look-2km` on the lit path, every verdict of TEST-021 recomputed by the harness's own functions from the per-frame logs. The report-only streaming logs (`dive-fast`, `dive-cold`) are shown only when they come from the source, adapter and power scheme of `lit.json`. The terrain pass's 8 ms is M2's budget carried to the lit path (ADR 0034 point 4); the p99 of the waited frame is judged at any clock state (ADR 0034 as amended by the owner, 2026-10-15). The coastal flights (both shore-filter settings), the compact against vertex ratios, the GPU span of a waited frame, `dive-fast` and `dive-cold` on the lit path and the host load SHALL be reported and never gated. The outcome SHALL be PASS, FAIL (a gated row over its budget) or INCOMPLETE (no row failed but a gated GPU row is not valid for GPU budgets or not measured, or a coast view has no water node in some run), and only PASS exits zero. The command SHALL REFUSE, listing every reason, print the commands of the window and the acceptance, write no report and exit non-zero, when a gated log is missing, a session is invalid for frame verdicts (the readiness gate, throttling, smoke, another adapter or power scheme, the owner's override, noise: `planet_perf::validity`), the frame size is not 2560 by 1440, the source is not the stored production bake, `protocol.field` of `lit.json`, `lit-formats.json` or a streaming log is not the production field drawn on the lit path with compact tiles (each log states its tile format), a held view has fewer than three runs (a partial log, `--views`, is not a window), a streaming log is old-format, partial or not one scene's own process, the gated dive does not say pre-filled to level 4 or deeper, or the logs come from different sources, adapters or power schemes. With valid logs it SHALL write `docs/milestones/M5-report.md` (as `target/review/M5-report.generated.md` when that file exists or an automated step failed) holding the verdict, the gated and the reported rows, the per-pass table, the facts of the window and a slot marked TO FILL for every finding no log carries (the look review of ADR 0037, the M4 budgets on the lit path, the orbit atmosphere cost, the water pass, the compact against vertex ratio, the per-pass costs, ADR 0036 point 3, the shore filter in motion, the store regeneration, the host load, the proposed ADRs, the ADR 0037 window, the viewer's manual checks, the recommendation).

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
- **WHEN** a session is throttled, the frame size is 640 by 360, the bake is in memory, the tiles are vertex (in `lit.json` or in a streaming log), a view has one run, a view is missing, a stream's source differs, or `lit.json` and a gated stream log are missing
- **THEN** the command refuses with every reason, the commands of the window and `cargo xtask accept M5`, writes nothing and exits non-zero

#### Scenario: Optional parts
- **WHEN** `lit-formats`, the flights, `dive-fast` and `dive-cold` are absent
- **THEN** the window is judged and the report says each was not run

### Requirement: TEST-027 Flight capture for the look review
`cargo xtask capture flights` (and `test-render --scene capture-flights --out-dir DIR`, with `--store DIR` or `--allow-memory-bake`, `--size WxH` default 2560x1440 and `--flight-frames N` default 240) SHALL draw the two coastal flights of TEST-025 on the lit path, each with `water.shore_filter` at the look's default and at the other setting, and write every frame as `DIR/<flight>-filter<k>/frame_NNNN.png` with an index `capture.txt` (directory, frames, size, shore filter); `cargo xtask capture flights` SHALL then encode each sequence at 60 frames per second as `<name>.mp4` (H.264, yuv420p) when `ffmpeg` runs, and otherwise say that the PNG sequences stand. The default directory is `target/review/m5-flights`. The capture is a tool beside the harness and not a measurement: it takes no timestamp query, applies no gate, writes no perf log, prints no budget and is not to be run during a perf window (a rule of the project, not enforced by the tool); the world needs the ocean channel and a coast, and a missing output directory or world source is refused by the run before it draws.

Verify: A, C
Status: active
Source: M5 item 10 (status.md: the 1440p flight videos of the glide and the descent with shore_filter 0 and 1 for the owner's look review, ADR 0037), TEST-025, ATMO-017

#### Scenario: Frames per flight and filter
- **WHEN** the capture runs on the software adapter over an in-memory bake at 320 by 180 with two cameras per flight
- **THEN** four directories `coast-glide-filter1`, `coast-glide-filter0`, `coast-descent-filter1` and `coast-descent-filter0` each hold `frame_0000.png` and `frame_0001.png` of 320 by 180, `capture.txt` lists them, and no perf log or budget is written

#### Scenario: Command lines
- **WHEN** `capture flights` is given a store, a frame count and the smoke flag
- **THEN** the cargo arguments run the capture scene (on the hardware adapter without the flag, on the software adapter with it), carry no perf or gate option and end with the output directory, and the `ffmpeg` arguments read the sequence at 60 frames per second and write H.264 in yuv420p

#### Scenario: Refusals
- **WHEN** no output directory, or neither a store nor `--allow-memory-bake`, is given, or the sub-command is not `flights`
- **THEN** the run refuses before any work and names what is missing
