# SPIKE-95: a perf window that blocks half as long (B9)

Opened: 2026-11-02
Question: where does a perf window's wall-clock time go after ADR 0087, and which 2-4 PRs halve the time it blocks the other lanes without losing verdict validity?
Why it matters: Option B item B9, carried to the spikes lane (owner, 2026-10-30: halve the blocked time; opus-planner, inbox #71). Window 2 of SPIKE-90 took 1 h 44 min for 11 valid attempts of 42.
Time box: 4 h, analysis only, no perf runs, no code changes; used about 1 h 30 min.
Success criteria: a wall-clock inventory from the harness code and the last 3 windows; each candidate with an expected saving and a validity argument; a ranked plan of 2-4 PRs, each with the measurement that proves it.
Fallback if it fails: ADR 0087 stays the process as built; windows keep their current length.
Code location: none (reading only). SPIKE-58 (`spikes/m9-perf-windows/results/`) covered M8's windows and led to ADR 0087; this spike covers what is left after it.
Closed: 2026-11-02

## Inputs
- Harness code: `crates/perf/src/` (gate, clock, calibration, validity, loaded, ab), `crates/app/src/streamperf.rs`, `litperf.rs`, `preflightperf.rs`, `xtask/src/testcmd.rs`, `perfab.rs`, `heavy.rs`.
- Windows (the last three, all after ADR 0087's build of 2026-11-01):

| Window | Date, time | Kind | Attempts | Valid | Source |
|---|---|---|---|---|---|
| W-reserve | 2026-11-01, about 10:20-12:25 | `perf reserve` and lit flights, quiet and loaded | 6 quiet sessions (plus 5 loaded lit flights) | 1 of 6 quiet with a frame verdict | `docs/status.md` (teleport-1m reserve marginal); logs in the worktree `start-879e4b/perf/logs/` (local, not committed) |
| SPIKE-90 W1 | 2026-11-01 21:17-23:15 (118 min) | stream, 4 configurations x 3 flights x 3 rounds | 76 | 36 kept | `spikes/dual-pool/results/results.md` |
| SPIKE-90 W2 | 2026-11-02 09:05-10:55 (110 min) | stream, 5 configurations x 2 flights; upload run | 42 + 3 | 11 + 3 | `spikes/dual-pool/results/raw/window2-attempts.md` |

## 1. Where the wall-clock goes
### One run, from the code
| # | Phase | Cost | Where |
|---|---|---|---|
| 0 | heavy lock or quiet flag | polls every 2 s, unbounded | `xtask/src/heavy.rs:18` |
| 1 | build | `cargo run --release` on every run: a check of a few seconds when nothing changed, a full release build after any edit | `testcmd.rs:712`, `:982` |
| 2 | process start, store open, prefill check | input-dependent, seconds | `streamperf.rs:763-786` |
| 3 | readiness gate | 10 s CPU load + 10 `nvidia-smi` calls 1 s apart (in parallel), then 4 PowerShell queries in sequence: about 12-16 s, once per process; a refusal exits (no wait, no retry) | `gate.rs:19`, `:390-425`; `app/src/lib.rs:807` |
| 4 | calibration "before" | about 0.15 s: **one** ~50 ms single-thread kernel on an unpinned thread, then one kernel on each of 18 threads | `streaming.rs:666-687`, `streamperf.rs:535` |
| 5 | settle (teleport, look) | 1 s of quiet frames, cap 900 s | `streamperf.rs:543-566` |
| 6 | cool-down | waits for **GPU** temperature 60 C, polls 5 s, cap 120 s; stream: once before the frames; lit: before each of 7 views | `clock.rs:24-27`, `streamperf.rs:571`, `litperf.rs:201` |
| 7 | measured frames | dive 2,400 frames (40 s at 60 Hz); teleport and look 600 (10 s); lit static 3 x 300 waited frames | `accept.rs:37-39`, `litperf.rs:287` |
| 8 | retry | lit static only: one retry of all 3 runs above 10 % noise; streaming: none; a calibration miss is never retried | `litperf.rs:331-357` |
| 9 | calibration "after", lag post-pass | 0.15 s plus the post-pass | `streamperf.rs:690-698` |

- The calibration "before" (phase 4) runs before the cool-down (phase 6): it reads the CPU while it is still hot from the previous run.
- A calibration outside its limits voids frame **and** GPU-timestamp verdicts in a quiet session (`validity.rs:12`, `:36`, `:44-50`); in `--loaded` it is recorded only.
- `perf --ab` runs each run as its own `cargo xtask perf` child (build check, store open, gate per run); a refusal waits 30 s, up to 40 times (`perfab.rs:14`, `ab.rs:12`).
- ADR 0087 point 7 is not built: no early stop, and no start, end or active time in the logs (`lib.rs:164-182`); window durations below come from console lines and the runners' notes.

### SPIKE-90 window 2 (110 min), reconstructed
| Item | Minutes | Share | Basis |
|---|---|---|---|
| Measured frames (valid attempts) | 4 | 4 % | 11 valid: 7 dive x 40 s, 4 teleport x 10 s; plus the upload runs' 3 x 10 s |
| Measured frames (INVALID attempts) | 10 | 9 % | 31 attempts: 13 dive x 40 s, 18 teleport x 10 s |
| Cool-down waits at the 120 s cap | 26 | 24 % | 12 redo attempts of block 2 plus 1 in block 1, 120 s each |
| Idle pauses set by the runner | 26 | 24 % | 45 s before each of block 3's 23 runs (17 min) and the 9 min pause |
| Gate, start-up, settle, post-pass | 30 | 27 % | 45 runs: teleport 40 s and dive 77 s wall per run less their frames |
| Setup before the first run | 6 | 5 % | 09:05-09:11 |
| Analysis gaps between blocks | 8 | 7 % | the rest of 110 min |

- 31 of 42 attempts INVALID, every one by the CPU calibration (single before over 52 ms or after over 56 ms): **about 75 % of run time produced no verdict**.
- Block 2 (35 min): every attempt hit the 120 s cool-down cap and then missed the calibration by 3-5 %; a total loss.

### All three windows
| Window | Wall time | Attempts per valid result | Main loss |
|---|---|---|---|
| W-reserve, quiet sessions | about 2 h with the loaded flights | 6 per frame verdict | 5 of 6 sessions void by the calibration (single 62-73 ms against 52) |
| SPIKE-90 W1 | 118 min | 2.1 (76 for 36) | 40 refusals or INVALIDs, 39 of them the calibration; **15 on the unloaded configurations a, b, c (36 % of their attempts)** |
| SPIKE-90 W2 | 110 min | 3.8 (42 for 11) | calibration (31), cool-down cap (26 min), runner idle (26 min) |

- Calibration misses are mostly small: under 10 % over the limit (W1: 53.2 against 52 ms; W2: 52.5-58.5 ms).
- On the unloaded configurations they are noise, not load: the limits are the quiet maximum plus about 5 % (`calibration.rs:6-14`), and SPIKE-58 phase 2 saw 2 of 4 quiet preflights refused.
- On the loaded configurations (W2's d-plain, BG2, Eco, Duty) the calibration measures the synthetic load the experiment put there on purpose: "the gate and the question are the same measurement" (SPIKE-90 W2 reading 4).

## 2. Candidates
| Candidate | Expected saving | Validity argument | Verdict |
|---|---|---|---|
| **A. A robust calibration estimator**: the single reading as the minimum of 5 kernels (fixed work: interference only adds time, so the minimum is the machine's state without transients), the parallel one as the median of 3, taken right before the measured frames (after the cool-down and settle) | W1: quiet INVALIDs 36 % -> under 5 % (estimate): 76 attempts -> about 50, **-30 %**; W-reserve: 5 of 6 void -> most valid | ADR 0065 protects against a **persistent** slow state (the planner terminal: 56.7-78.1 ms on every reading, a 1.4x per-thread state); a minimum of 5 still shows it. It removes only transients (an interrupt, a context switch, a hot first kernel) that do not last the run | **PR 1** |
| **B. Scope the calibration to the verdicts it protects**: CPU wall-time verdicts (stream frame and tile times, plan times); GPU-timestamp verdicts are judged by the clock floor (ADR 0036/0057) and the noise rule, not by a CPU kernel | lit windows: every calibration void of a GPU verdict (window 25's `lit-full-r2`: 52.9 against 52 ms) | A GPU pass time does not depend on a single CPU thread's speed; the CPU's share of the power budget shows as GPU clock, which the floor already judges | **PR 1** |
| **C. Declared in-harness load**: a run that starts its own load (`PLANET_DUAL_POOL_LOAD`, `perf reserve`'s spin) records the calibration as loaded mode does and is judged by its question (c against d), not voided | W2: 20 d attempts -> 10 (one per run); W1: 24 d attempts -> 9 | The load is the variable under test, logged with the run; voiding it makes the question unanswerable (SPIKE-90 W2 reading 4). The unloaded reference runs keep the full rule | **PR 1** |
| **D. Temperature-aware gate**: stream scenes drop the GPU cool-down (their verdicts are CPU wall time; the GPU's temperature does not enter them); lit views keep the clock-floor warm-up and cap the cool-down at 30 s, recording the temperature. Heat on the CPU is handled by a fixed duty cycle in the runner (idle 0.5 x the previous run's wall time, at most 45 s), not by a wait on a sensor the CPU does not follow | W2: 26 min of capped waits -> 0, **-24 %**; lit: up to 7 x 90 s per session | The cool-down is a means, never a verdict input: the verdicts' guards are the clock floor (GPU) and the calibration (CPU), both kept. W2 block 3 (45 s idle) ran at 52-58 C with no waits, and its unloaded runs were valid | **PR 2** |
| **E. Count-based stop (ADR 0087 point 7)**: stop a configuration after its first valid round when it misses the reference by more than 3x the margin (here 15 %), after 2 when within the margin both times; full rounds only in between | W2: every knob attempt raised p99 by 39-65 % (8x the 5 % margin): after one valid round each, 25 planned runs -> about 15, **-40 %** of runs; W1: d failed by 20-82 % | Round-to-round spread on this machine is up to 10 % (W1 caveat); a 40 % regression cannot reverse in two more rounds of that spread. One-sided: only a far-off reading stops early; anything near the margin gets every round | **PR 3** |
| **F. A/B interleaving in one process / one window runner**: an xtask runner for a declared window plan: build once, full gate once, then a 2 s re-check per run (process scan and load), rotation, bounded retries, the stop rule of E, start, end and active time per run, one process (TaskStop-safe, no log-copy slips) | gate and start-up 30 min in W2 -> about 15 (**-14 %**); the 9 min runaway-runner pause and the copied-log slip of W1 removed | The full gate still opens the window; the per-run re-check refuses on any competing build or test process, as the gate does; the load record during each case is unchanged | **PR 3** |
| **G. Diff-scoped scenes**: run only the flights a change touches | accept and regression windows only (M8 window 26 re-ran criteria 4-7); none for experiment windows like SPIKE-90 | A scene a diff does not touch can still regress through shared code; acceptance keeps the full set | process note for the planner, no PR |
| **H. Other lanes build at idle priority on the E-cores during GPU-bound runs** | the whole window, if it were valid | **No.** Windows exist for verdicts within 5-20 % of their budget (ADR 0087 point 3). Measured added time from E-core load: GPU +9-14 % pinned (SPIKE-58), a P-core thread +2-19 %, frame p99 +39-65 % even at 28 % of the background's throughput (SPIKE-90 W2). The channel is shared power, L3 and memory bandwidth, which priority and core choice do not isolate | rejected; lanes do CPU-free work (docs, reviews, specs) in a window instead |

## 3. Ranked plan
| PR | Change | Spec or ADR touched | Measurement that proves it |
|---|---|---|---|
| 1 | `perf-calibration-robust`: A + B + C. Single = minimum of 5 kernels, parallel = median of 3; calibration after the cool-down and settle; voids only CPU wall-time verdicts; declared in-harness load is recorded, not voiding. Limits re-derived from the new estimator's quiet readings | TEST-039, PERF validity rules; an ADR 0065 amendment (the estimator and the scope; "quiet maximum plus about 5 %" stays) | In the next window's first 15 min: 20 `perf preflight` rounds on the quiet machine with both estimators in one process: false refusals old against new (target under 5 %, W1 had 36 %); 3 rounds from the planner's VS Code terminal: the slow state is still refused |
| 2 | `perf-duty-cycle`: D. No GPU cool-down for stream scenes; lit cool-down cap 120 -> 30 s with the temperature in the log; the runner's duty idle | ADR 0057 amendment (cool-down per scene kind); testing spec | 10 back-to-back stream runs (dive, teleport-1m) with the duty idle against 10 with today's cool-down, interleaved: valid share and tile-ready and p99 medians within the quiet noise (2.7 % CPU, 4.4 % GPU) |
| 3 | `perf-window-runner`: E + F. `cargo xtask perf window <plan>`: one build, one full gate, per-run re-check, rotation, bounded retries, the stop rule, start, end and active time per run in each log (ADR 0087 point 7) | BUILD requirement for the command; testing spec for the stop rule's thresholds | WARP dry run of W2's plan (orchestration and stop rule at tier B with fake readings); in the first real window: minutes per valid result against W2's 10 (110 min for 11) and W1's 3.3, target 3 or less |

- Order: PR 1 first (the largest loss and the precondition for any count-based stop); PRs 2 and 3 are independent of each other.
- Combined estimate on W2's plan: 110 min -> about 45 min (-27 min cool-down, about -30 min of INVALID attempts and their idle, about -10 min of stopped knobs, about -8 min of gate and start-up). On W1's plan: 118 -> about 60 min. **About 50-60 % less blocked time: an estimate from three windows, not a measurement**; the first window after PR 1 measures it (PR 3's timestamps make it exact).
- Not changed: the readiness gate's conditions (ADR 0020), the clock floor, the loaded-pass margins (ADR 0087), the owner's `--machine-ready` override.

## Risks
- The minimum of 5 could hide a slow state that comes and goes within seconds; the after-calibration and the frame data's own spread still show it, and PR 1's measurement includes the slow terminal.
- Dropping the stream cool-down assumes the GPU's temperature does not reach a CPU-timed verdict; if a hot GPU lowers the shared power budget enough to slow the CPU, the calibration (taken right before the frames) catches it.
- The reconstruction of W2's minutes is from console lines and runner notes (no timestamps in the logs); shares are within a few minutes.

## Visuals
Tables only: no images.

## Proofs (quiet slot 2026-11-03)
Details: `slot/results-2026-11-03.md`.
- PR 2 and PR 3 (duty idle, window runner): 10 of 10 valid, 1.4 min per valid run (windows 1 and 2: 3.3 and 10 min).
- PR 1 (the estimator): 4 of 21 quiet rounds over a limit, all single and all the first round of a preflight; the slow state (1b) was not available.
- Flight verdicts at the terrain share (ADR 0092) and REND-052's 2° verdict: void (another lane's load, a heat soak, then a changed machine state); moved to the next window.
