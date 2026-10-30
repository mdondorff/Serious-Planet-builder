# SPIKE-58 Perf windows that block less

Opened: 2026-10-30
Question: what process cuts the wall-clock time that perf windows block the other lanes by at least half, without losing the validity of verdicts?
Why it matters: Option B item B9 (owner, 2026-10-30). M8 windows took 1-5 h each on the one machine, every other lane stopped meanwhile, and the month's plan is at risk.
Time box: phase 1 (desk) about 2 h; phase 2 (noise measurement) about 1 h of machine time, only on opus-planner's go.
Success criteria: phase 1: an inventory of M8's windows; options (a)-(e) evaluated with numbers; a process with an estimated cut of at least 50 %; a proposed ADR. Phase 2: measured noise of one GPU-timestamp scene and one CPU-timed scene, quiet against loaded, with and without core pinning.
Fallback if it fails: if loaded measurements are too noisy to trust at all, keep exclusive windows and cut their length only (options b and d).
Code location: none in phase 1 (desk only).

Status: phase 1 done. Phase 2 waits for opus-planner's go.

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

Together: from about 35-45 h of blocked time in M8 to about 10-15 h, **a 60-75 % cut**. The estimate depends on phase 2 confirming that loaded measurements only add time on this machine, and on the size of that added time.

## Phase 2 plan (on opus-planner's go; about 1 h of machine time)
- Scenes: one GPU-timestamp scene (`perf stream shadow-cost`, one pose, 1440p) and one CPU-timed scene (`perf anet-chunk --store`, the GEN-052 8 m chunk), plus the TEST-039 calibration kernel.
- Conditions, each three rounds:
  1. quiet;
  2. under a parallel `cargo build` of the workspace (a clean `target` in another directory) and `cargo xtask test fast`, at normal priority;
  3. the same load, with the perf process pinned to the P-cores at High priority and the load pinned to the E-cores at BelowNormal.
- Numbers per condition: the median and spread of the GPU ms, wall and thread-CPU seconds of the chunk, and the calibration ratio. Questions: does the load ever make a reading faster (the one-sided rule)? How large is the added time (the 20 % margin)? Does the calibration ratio cancel it? What does pinning buy, and what does it cost the build?
- Outputs: `raw/` logs and a table in this file; the ADR draft's numbers updated.

## Visuals
Phase 1 is a desk study: nothing rendered.
