# SPIKE-58 Perf windows that block less

Opened: 2026-10-30
Question: what process cuts the wall-clock time that perf windows block the other lanes by at least half, without losing the validity of verdicts?
Why it matters: Option B item B9 (owner, 2026-10-30). M8 windows took 1-5 h each on the one machine, every other lane stopped meanwhile, and the month's plan is at risk.
Time box: phase 1 (desk) about 2 h; phase 2 (noise measurement) about 1 h of machine time, only on opus-planner's go.
Success criteria: phase 1: an inventory of M8's windows; options (a)-(e) evaluated with numbers; a process with an estimated cut of at least 50 %; a proposed ADR. Phase 2: measured noise of one GPU-timestamp scene and one CPU-timed scene, quiet against loaded, with and without core pinning.
Fallback if it fails: if loaded measurements are too noisy to trust at all, keep exclusive windows and cut their length only (options b and d).
Code location: none in the repo; phase 2 drove the existing scenes with scratch scripts (process list, affinity, priority, CPU sampler), not committed.

Closed: 2026-10-30 (phase 2: about 1.5 h of machine time, 18:44 to 19:54).

## Inputs read
- `cargo xtask log` (M8), `git log` of main since 2026-10-20 (window commits with times), `docs/briefings/estimates.md` (planned against actual), `docs/milestones/M8/` (`criterion-6-edit-window.md`, `acceptance-runbook.md`, 20 edit-latency tables, envelope logs), `docs/status.md`, `docs/learnings.md`, `perf/logs/*.json`.
- ADR 0020 (readiness gate, noise 10 %), ADR 0034 (clock state), ADR 0036 (power cap normal, clock floor 1,400 MHz).
- `crates/perf/src/gate.rs`, `calibration.rs` (TEST-039, a fixed CPU kernel timed before and after each scene), `load.rs` (competing load sampled during a case, recorded only), `clock.rs`; `xtask/src/heavy.rs` (the quiet flag).
- Limit: no perf log records a window's wall time (`acceptance-runbook.md` says so for edit-latency), so durations below come from the estimates ledger and from the spacing of the commits that recorded each window. They are a reconstruction, not a measurement.

## Inventory: M8's windows
| Day | Windows | What they measured | Machine time (reconstructed) |
|---|---|---|---|
| 2026-10-26 | GEN-052 window; 3 envelope chunk curves (#752, #763, #769); criterion 6 window 2 | chunk time (GEN-052); envelope floor with the chunk stage (criterion 7); edit latency 6x2 (criterion 6) | about 8-10 h (commits 06:27 to 23:35) |
| 2026-10-27 | windows 8 to 25 (about 18) | criterion 6 (edit latency, 12 tables), criterion 7 (envelope steady curves, windows 11, 12, 14), criterion 4 (coupled make, window 21), criterion 5 (shadow cost, windows 22-25) | about 19 h (commits 01:51 to 20:28, windows back to back) |
| 2026-10-28 | window 26 (criteria 4-7) | acceptance run | 2.5 h wall (ledger; planned 1-1.5 h): edit-latency 1.5 h, one GPU cooldown retry |
| 2026-10-29 | windows 27, 28 | criteria 6, 7; criterion 6 drawn tiles | window 27: 4.8 h wall, 0.3 h active (ledger: 5 edit-latency repeats dominate) |
| 2026-10-30 | window 29 | criterion 7 (920 m/s at 300 m) | not recorded |

- **About 29 numbered windows in 5 days**, plus 4-5 unnumbered on 2026-10-26. Machine time in windows: roughly 35-45 h of about 110 h, **a third or more of the calendar**, with every other lane stopped.
- Side cost recorded once: SPIKE-54 took 1.1 h of work plus 6 h waiting for a quiet slot (ledger, 2026-10-29).

What went wrong in windows (from the records):
| Kind | Cases | Cost |
|---|---|---|
| Refused by the readiness gate (competing load), rerun | 2 first attempts on 2026-10-26 (`criterion-6-edit-window.md`) | minutes each, plus clearing lanes again |
| Refused at option parsing, rerun | 2 (6x2, 8x2) | minutes |
| GPU cooldown retry | 1 (window 26) | part of the 1 h over plan |
| **Measured, then the reading was redefined** (method, not noise) | windows 13 to 20 read "drawn exact = uploaded"; ADR 0072 changed the reading at window 21, so 8 windows' criterion 6 column was superseded | about 8 windows |
| **Measured on the wrong setup** | the 2026-10-26 envelope curves ran with 18 chunk workers, while the viewer has 4 (`m8-chunk-curve-superseded`) | 2 windows |
| Place effects mistaken for limits | window 12's 78 m/s were slow-chunk places; window 14 repeated at fixed start places | 1 window |
| Waiting inside a window | window 27: 0.3 h active of 4.8 h wall | 4.5 h |

**Noise was not M8's main loss.** No window is recorded as invalid for run-to-run noise. The losses were exclusive windows for questions that did not need a quiet machine, readings redefined after the fact, and long fixed repeat counts.

## Margins of M8's verdicts
| Verdict | Measured | Budget | Margin | Needs a quiet machine? |
|---|---|---|---|---|
| Criterion 4: coupled run (GEN-042) | 19.7 s | 600 s | 30x | no: a loaded PASS stands |
| Criterion 7: envelope floor at 300 m | 460-920 m/s | 200 m/s | 2.3-4.6x | no for the PASS; yes for the curve's exact value |
| Criterion 6: drawn tiles (M6 column) | 53-761 ms | 300-1,000 ms | 1.3-19x | no for most rows; quiet for rows within 25 % |
| Criterion 6: preview | 13.1 ms | 16 ms | 18 % | yes, or a GPU or count proxy |
| Criterion 6: chunk 10 s column | 10.65, 11.05 s over; the rest under | 10 s | about 0 | yes |
| Criterion 5: shadows + material (REND-044) | 0.96-1.42 ms | 1.5 ms | 5-36 % | no CPU quiet needed (GPU timestamps); no other GPU user |
| GEN-052: chunk time | 3.30 s | 4 s | 17 % | yes (CPU wall time, tight) |

## Options (a)-(e)
Key fact for all of them: **competing load only adds time** to a wall-clock or GPU-time measurement on this machine; it never makes the measured work faster. One exception: a starved streaming scene may draw fewer tiles and get a faster frame, so this holds for budgets on work done, not for frame time with streaming on. So a time budget that passes under load would pass on a quiet machine, while a fail or a near miss under load says nothing. Turbo boost goes the same way: fewer busy cores, higher clocks.

### (a) Measurements robust to concurrent load
| Technique | Exists? | Effect | Estimate (phase 2 measures) |
|---|---|---|---|
| GPU timestamp queries | yes (`gpu_timing.rs`, shadow-cost, SPIKE-56) | immune to CPU load; not to GPU clock state (ADR 0034/0036 floor) or another GPU user | SPIKE-56 under parallel CPU load: rounds within 1-10 % of each other inside one session, sessions up to 1.5x apart (clock state); the clock floor rule already catches that |
| A/B interleaved in one process | yes for shadow-cost (round-robin, compiled-out variant) | load hits A and B alike: the difference survives | difference noise roughly the per-round spread (1-10 %) |
| Ratio to a reference kernel | the TEST-039 calibration kernel is timed before and after each scene but used only as a gate | normalising CPU times by the calibration ratio removes clock and load shifts that hit both alike | unknown: phase 2 measures whether the ratio holds under a build |
| Core affinity and priority | no | i7-12800H: 6 P-cores (12 threads) and 8 E-cores. Perf on P-cores at High priority; builds and tier A on E-cores at BelowNormal (`start /affinity`, job objects) | builds slow down by about 2x (8 E-cores instead of 20 threads); perf noise unknown (phase 2) |

### (b) Shorter windows
- **Only the scenes a diff touches.** Most M8 windows already ran one scene. The cut comes from not re-running passed rows whose code did not change: window 26's acceptance re-ran all of criteria 4-7.
- **Statistical stop rules instead of fixed repeats.** Edit-latency is 19 rows x 3-5 repeats (window 27: 5 repeats, 4.8 h wall). Rule: stop a row after its first repeat when the reading is under 50 % of its budget, after 3 when under 80 %, and run all 5 only within 20 % of the budget. In window 24's table (`edit-latency-w24-main819-5x.md`, worst of the preview and drawn-tile ratios per row), 7 of 13 gated rows are under 50 %, 5 between 50 and 80 %, and 1 above (the 1 km brush preview, 82 %). With the 8 recorded rows (no budget) at one repeat, repeats fall from 105 to 35: **about 3x shorter**. Rows of the chunk 10 s column near 10 s keep all repeats. The envelope curve's bisection (11 trials of 72 s) can stop once the floor is passed by 2x; criterion 7 is a floor, not a maximum.

### (c) Verdicts as counts
| Verdict | Count proxy | Where it can run |
|---|---|---|
| Edit refresh work (criterion 6) | tiles refreshed per edit, refresh jobs on the critical path, chunks re-solved, held-back tiles (already in the tables and gate counters) | tier B or CI on the small store: a regression in counts flags before any window |
| Envelope stand-in share | stand-in nodes per frame with a fake clock in which a chunk solve takes a fixed time (STRM fakes) | tier B: the scheduling logic gets a verdict without timing; the time per chunk stays a window number |
| GEN-052 chunk time | cells and steps per chunk, routing segments, thread-CPU seconds | tier A/B for counts; CPU time per thread is much less load-sensitive than wall time (phase 2 checks) |
| Frame budgets | draws, triangles, bytes uploaded per frame | already partly in TEST-012 counts |

Counts do not replace a time budget. They move the *regression* question out of the window, so a window confirms a time number once instead of hunting a regression over many windows (criterion 6 took 12 windows).

### (d) Batched windows
- One window per day at a fixed time, holding every queued measurement, instead of a window per question: 29 windows become about 5. Each window carries a fixed overhead: clearing the lanes (messages, idle confirmations), the gate's 10 s and 70 s preflight, GPU warm-up to the clock floor (up to 30 s a view), and cooldown retries. Estimated 20-40 min per window; 24 fewer windows save 8-16 h of blocked time in a milestone like M8.
- At night: the lanes are agents and run at night too, so a night window blocks them the same way. A night window costs less only when the planner lines up desk work (reviews, specs, spikes without builds) for that time. The owner's laptop must stay on and plugged in (ADR 0020 gate).

### (e) Verdicts that truly need an exclusive quiet machine
1. A time budget that **fails or passes within 20 %** under load (GEN-052 at 17 %, the preview at 18 %, the chunk 10 s column).
2. **Frame time** median and p99 with streaming on (CON-21's user-visible numbers; the one non-monotone case above).
3. **Regression comparisons across sessions** of CPU wall time below 10 % (TEST-012 baselines).
4. The **milestone acceptance run** of the criteria that are close to their budgets (not those with 2x margin).
Everything else can run loaded: GPU timestamps with the clock floor, counts, and time budgets that pass with margin.

## Proposed process (estimate: at least a 60 % cut)
1. **Loaded first.** A perf scene runs whenever its lane is ready, without a window, at the scene's normal settings. A time verdict that **passes with at least 20 % margin** under load is a valid PASS (one-sided rule, labelled "loaded pass" in the log). Anything else (fail, within 20 %, frame time with streaming, cross-session regressions under 10 %) is queued for the next window. Load is recorded as today (`load.rs` notes).
2. **GPU budgets by timestamps.** GPU-time verdicts use timestamp queries with the ADR 0036 clock floor and an A/B or compiled-out pair in the same process. They need no CPU-quiet window, only no other GPU user (`test gpu --real` is the only other one).
3. **Counts in tier B.** Each time verdict gets a count companion in tier B (tiles refreshed, jobs, chunks re-solved, stand-in nodes on a fake clock), so regressions are caught in CI and a window confirms a number, not a search.
4. **One batched window per day** at a planned time for the queued verdicts, with lanes given desk work in advance.
5. **Stop rules.** Rows far from their budget stop after one repeat; full repeats only within 20 % of the budget.
6. **Window wall time logged.** The harness writes start, end and active time per scene, so the next inventory is measured, not reconstructed.

Estimated effect on M8's pattern:
| Step | M8 windows affected | Blocked time saved |
|---|---|---|
| 1: loaded first | criteria 4 and 7 floor, criterion 6 rows with margin (about two thirds of the rows), envelope curves | about 40-50 % of windows need no exclusive time |
| 5: stop rules | the remaining edit-latency and envelope runs | about 3x shorter |
| 4: batching | the remaining 8-12 windows become about 5 | 3-5 h of overhead |
| 3: counts | the 8 superseded criterion 6 windows (regression and reading hunts) move to tier B | about 8 windows |

Together: from about 35-45 h of blocked time in M8 to about 10-15 h, **a 60-75 % cut**. Phase 2 confirmed the one-sided rule and measured the added time (below). The estimate depended on phase 2 confirming that loaded measurements only add time on this machine, and on the size of that added time.

## Phase 2: noise under load (2026-10-30, 19:11 to 19:54)
### Method
- Binary: `target/release/planet.exe` of this worktree, main 10db3b63, release. Each run under `cargo xtask heavy`; one round = `perf preflight` (the TEST-039 kernel, 3 calibrations), `perf stream shadow-cost` (river-r1, pose 300m-sunset, 1440p, 3 interleaved rounds of the compiled-out variants), `perf anet-chunk --store C:/DEV/SeriousPlanetData/global16384` (8 m production threads, 8 m one thread, 4 m, 10 m).
- Conditions, 3 rounds each:
  - **quiet:** nothing beside (process list at start and end of every run: no cargo, rustc or nextest); rounds 1-3 at 19:11-19:20, round 4 at 19:51-19:54 with the CPU sampler on.
  - **loaded:** one loop of clean `cargo build --workspace --all-targets` (debug, own target directory) at normal priority on all 20 logical CPUs.
  - **pinned:** the same loop on the E-cores (logical CPUs 12-19, BelowNormal); `planet.exe` on the P-cores (0-11, High).
- `--machine-ready` on the loaded and pinned rounds only: owner's approval for tonight's SPIKE-58 phase 2 (relayed by opus-planner, 2026-10-30: "This is ok for tonights test"); these logs are spike measurements, never verdicts; the approval does not carry over.
- GPU state: NVIDIA power management "Prefer maximum performance" globally and `planet.exe` at High performance in Windows Graphics (owner, 2026-10-30 about 18:30). Earlier windows' GPU numbers are the old baseline (ADR 0034).
- CPU state: `PercentProcessorPerformance` (_Total) sampled every 10 s from 19:21 (`raw/cpuperf.txt`); the frequency counter reads a fixed 2,160 MHz and is not used.
- Not run: `cargo xtask test fast` as load (it waits on the heavy lock the perf run holds); a debug build loop stands in.
- Raw: `raw/table.txt` (every run), `raw/runs.jsonl` (process lists, process CPU time, affinity, priority), the scene logs `raw/*.json`, the build times `raw/load-*.txt`.

### Results
| Measure | Quiet (4 rounds) | Loaded (3) | Pinned (3) |
|---|---|---|---|
| Shadows + material, together (GPU ms) | 1.236-1.290, median 1.26 | 1.276, 1.485, 1.478 (+1 to +18 %) | 1.375, 1.434, 1.378 (+9 to +14 %) |
| Terrain pass, full (GPU ms) | 2.36-2.38 | 2.46-2.91 | 2.53-2.66 |
| Spread across the scene's 3 rounds (GPU ms) | 0.003-0.095 | up to 0.66 | up to 0.14 |
| Chunk 8 m, production threads (s) | 3.74-3.84, median 3.82 | 13.6, 17.8, 21.3 (3.6-5.6x) | 4.80-4.88 (+26-28 %) |
| Chunk 8 m, one thread (s) | 5.81-5.93 | 13.0-20.5 (2.2-3.5x) | 7.31-7.39 (+25-27 %) |
| Chunk 4 m (s) | 16.0-16.3 | 45.8-63.1 | 19.6-19.8 (+22 %) |
| Process CPU time of the chunk scene (s) | 77.5-78.0 | 155-159 | 100-101 |
| Calibration single (ms; limit 52) | 47-55 | 100-344 | 65-79 |
| Calibration parallel median (ms; limit 112) | 85-129 | 145-228 | 109-120 |
| Processor performance (%) | 113-147 (round 4) | 71-75 (brief 123-126) | 78-95 |
| Wall time of a round (s): preflight, shadow-cost, chunk | 73, 40, 60 | 93-97, 74-134, 149-201 | 75, 45, 72 |
| Debug clean build beside (s) | none | 221-243 | 357 (+50 %) |

- **One-sided rule holds:** no loaded or pinned reading is below the quiet minimum (GPU: loaded minimum 1.276 against quiet 1.236 to 1.290; CPU: every loaded reading above).
- **GPU timestamps are not load-immune here:** full CPU load adds up to 18 % to the terrain pass's shadow and material share, pinned load 9-14 %. Processor performance falls from about 140 % to 73 % under full load (all cores busy, not the 2026-10-29 latch: 132 % on a short load afterwards, opus-planner); the GPU on the shared laptop power budget runs the pass slower. Phase 1's "immune to CPU load" is withdrawn.
- **CPU wall time under full load is useless for verdicts** (3.6-5.6x, and the rounds differ by 1.6x). Pinned load costs 22-28 % and repeats within 2 %.
- **Process CPU time is not a load-immune substitute:** it doubles under load (hyper-thread sharing and the lower clock) and rises 29 % pinned.
- **The calibration ratio does not cancel load:** pinned, the single kernel rises 44 % while the chunk rises 27 %; loaded, the single kernel varies 2-7x between calibrations of one round.
- **The calibration gate refuses a quiet machine:** 2 of 4 quiet preflights were refused (round 1: single 55.5 ms over 52; round 4: parallel median 129 ms over 112, at 137-147 % processor performance). The limits sit within this machine's quiet noise.
- **Overload breaks the scene:** a first loaded block (18:47-19:11) ran two clean builds at once (debug and release, about 34 rustc processes): the chunk scene panicked twice spawning a thread (`PermissionDenied`, Windows error 5), the preflight and shadow-cost crashed with 0xc0000005, the gate could not read the process list; the 2 completed shadow-cost runs read 1.485 ms and the chunk 55.8 s (`raw/overload-*`). Not investigated (time box); filed as an overload case, not as data.
- Side reading, 18:44, under crusader's `test fast`: calibration single 49.7, 60.5, 52.6 ms (2 of 3 over the limit), parallel median 94-97 ms.

### Margin and consequences for the process
| Question | Answer |
|---|---|
| Does load ever make a reading faster? | No (30 loaded and pinned readings: 6 GPU, 24 chunk). The one-sided rule stands. |
| Validity margin of a loaded pass | The quiet noise: 4.4 % (GPU, 4 rounds), 2.7 % (CPU). A loaded reading at least **5 %** under budget is a valid PASS. Phase 1's 20 % was a guess for the added time, not for validity. |
| When is a loaded run worth starting? | GPU verdicts with at least 20 % margin (the pass's added time under full load is up to 18 %). CPU wall-time verdicts only with at least 6x margin under full load, or 30 % with the load pinned to the E-cores. |
| What pinning buys and costs | CPU added time from 3.6-5.6x to 1.27x, GPU from 18 % to 14 %; the parallel build takes 1.5x as long. |
| Calibration ratio as a normaliser | No: it over-corrects pinned and is erratic loaded. |
| CPU-timed scene after B6 | `anet-chunk` is removed by B6 PR 2; the rules above are per measure kind (GPU timestamps, CPU wall time), and the next CPU-timed scene inherits them; its noise is measured once in its first window. |

Effect on the estimate: criterion 4 (30x) and the GPU verdicts with 20 % margin pass loaded as estimated; criterion 5 (shadows within 1.5 ms) needs a window at today's 1.26 ms (16 % margin); GEN-052 type CPU budgets near 17 % margin need a window or a pinned run. The 60-75 % cut stands, with pinned runs as the means for CPU scenes.

## Visuals
None: both phases are measurement tables; the shadow-cost stills of phase 2 show one pose and are not evidence.
