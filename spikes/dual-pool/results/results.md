Opened: 2026-11-01
Question: Does a tiered dual pool (critical work on the P-cores, background work on the E-cores) keep the frame and the visible streaming at least as good as today, also while the background pool is saturated?
Why it matters: owner, 2026-11-01, on SPIKE-89: distrust the E-cores for critical work; divide only the P-core capacity among the shares (terrain about 6 on Rasierklinge) and let the E-cores serve background work, as long as it never slows the critical work. A measurement decides.
Time box: 3 h (Sonnet; prototype and plan); the measurement is explorer's, in a perf window on the planner's go.
Success criteria: a minimal prototype behind a switch; a measurement plan with four configurations, three flights, loaded and quiet, three interleaved rounds; the verdict rule stated before the run.
Fallback if it fails: the plan alone, with the prototype's limits.
Code location: branch `spike/dual-pool` (a4878fe9; never merged); patch in `proto/dual-pool.patch.txt`.
Status: open; the quiet half is measured (2026-11-01), the next step is a throttled background pool.

## Prototype
| Piece | What it does |
|---|---|
| `streaming::cpusets` | `GetSystemCpuSetInformation`: each logical CPU's CPU set id and `EfficiencyClass`; the highest class is the P-cores. `pin_current(Performance | Efficiency)` calls `SetThreadSelectedCpuSets` (a soft affinity the scheduler honours). One class, or Linux: `false`, nothing changes. Needs the feature `Win32_System_SystemInformation` on the existing `windows = 0.62`; no new crate |
| `streaming::jobs` | with `PLANET_DUAL_POOL=1` on a hybrid CPU: the any-class and frame-only workers are pinned to the P-cores and no longer take the prefetch class; the idle-only workers become the background pool, pinned to the E-cores, and take the prefetch and idle classes; their count is the E-core thread count unless `PLANET_DUAL_POOL_BG=N` |
| synthetic load | `PLANET_DUAL_POOL_LOAD=N`: N idle-class jobs on the background pool that spin for the system's life (xorshift writes over 4 MiB each, so they load caches and memory as a bake does); they stop when the job system closes |
| terrain 6 | `--gen-workers 5`: 5 tile workers plus the dedicated frame worker (`terrain_sizes`, cap 12) = 6 critical threads |
- Detected on Rasierklinge: P = logical 0-11, E = logical 12-19 (as SPIKE-58 found by hand).
- Smoke (software adapter, `test-render --perf-smoke --scene dive`): the switch prints `critical 5 + 1 frame workers on P [0..11], background 8 workers on E [12..19]`, the flight completes with and without `PLANET_DUAL_POOL_LOAD=8`, and the process exits (the load jobs stop at close). Frame times on WARP are not evidence (the software rasteriser competes for the same cores).
- Known limits of the prototype: the coupled run's own thread (`coupledworld`, not a job-system worker) and the hover and ground threads are not pinned (none runs in the streaming flights); the job-system tests that assume one idle worker (`idle_jobs_run_on_the_idle_workers_only...`) do not hold under the switch (8 background workers), which is expected and not a bug of the default path (25 of 25 pass without the switch).

## Configurations
| | Name | Switches | Critical threads on | Background on |
|---|---|---|---|---|
| a | today | none (`--gen-workers` default: 11 tile + 1 frame = terrain 12) | any core | any core (idle workers) |
| b | 10 unpinned | `--gen-workers 9` | any core | any core |
| c | dual pool | `PLANET_DUAL_POOL=1 --gen-workers 5` | P-cores (6) | E-cores (8 workers: prefetch, idle) |
| d | dual pool, background saturated | `PLANET_DUAL_POOL=1 PLANET_DUAL_POOL_LOAD=8 --gen-workers 5` | P-cores (6) | E-cores, all 8 busy with the synthetic bake |

## Measurement plan (explorer, perf window on the planner's go)
- Build: branch `spike/dual-pool` at a4878fe9, release, in its own worktree (never merged).
- Flights: `dive`, `teleport-1m`, `teleport-2km` (`cargo xtask perf stream <flight> --store <the window's usual store> --gen-workers N`, env as in the table).
- Order: 3 rounds; in each round every flight runs a, b, c, d in a rotated order (round 1 a-b-c-d, round 2 b-c-d-a, round 3 c-d-a-b), so drift and heat spread over the configurations: 3 flights x 4 configurations x 3 rounds = 36 runs per load state.
- Load states: loaded (`--loaded`, other lanes building; ADR 0087) and quiet (the harness's gate passes): 72 runs, about 1.5 to 2 h.
- Metrics per run (from the perf log): tile-ready = `load_run_us_median` + `load_wait_us_median`, frame `p99_ms`, frame `median_ms`, jobs done per second of the terrain share, and `reserve_terrain_peak_threads` where logged.
- Deliver: one table per flight (configuration x round x load state), the medians over the 3 rounds, and the verdicts below.

## Verdict rule (fixed before the run)
- A metric's value for a configuration and flight is the median of its 3 rounds; "worse" compares medians of the same flight and load state.
- **Owner's condition, (d) against (c): "does not slow".** Quiet: the median of (d) is at most 5 % above (c) for tile-ready and for frame p99, in every flight, and no single (d) round is worse than the worst (c) round by more than 10 %. Loaded: only a gross regression counts (more than 30 % on either metric, ADR 0087's margin for CPU times with load on the E-cores); the loaded runs cannot pass a 5 % question.
- **Dual pool against today, (c) against (a):** the same 5 % rule on tile-ready and frame p99 (quiet decides). A pass means terrain 6 on the P-cores plus the background pool is no worse than terrain 12 on any core.
- **10 unpinned, (b) against (a):** the same rule; it answers the owner's open 10-or-12 question beside the dual pool.
- Frame median over 16.6 ms or p99 over 20 ms in a configuration where (a) is within budget fails that configuration outright.
- A rule that cannot be judged (the gate refuses quiet runs for more than the window) is reported as "no verdict", never as a pass.

## Results
Run 2026-11-01 21:17 to 23:15 on Rasierklinge (explorer, perf window on the planner's go, ADR 0087), the quiet half only: 4 configurations x 3 flights x 3 rotated rounds = 36 runs, `cargo xtask perf stream <flight> --store C:/DEV/SeriousPlanetData/global16384 [--gen-workers N]` (no `--lit`), release build of `spike/dual-pool` at a4878fe9 in its own worktree, no `--machine-ready`. The desktop stayed unlocked. The loaded half was not run (the window was used up).

Medians over the 3 rounds. Tile-ready = `load_run_us_median` + `load_wait_us_median` (us); p99 and median are the frame times (ms). Every p99 is far under 20 ms and every median under 16.6 ms.
| Flight | Config | Tile-ready | Frame p99 | Frame median | Jobs |
|---|---|---|---|---|---|
| dive | a today | 5345 | 8.36 | 5.13 | 14072 |
| dive | b 10 unpinned | 2951 | 8.89 | 5.13 | 14002 |
| dive | c dual pool | 4516 | 8.63 | 5.38 | 13642 |
| dive | d dual pool, E-cores saturated | 5430 | 15.69 | 7.17 | 13277 |
| teleport-1m | a | 5484 | 5.53 | 2.40 | 11349 |
| teleport-1m | b | 4522 | 4.73 | 2.43 | 10607 |
| teleport-1m | c | 4027 | 4.17 | 2.22 | 5899 |
| teleport-1m | d | 5863 | 6.74 | 3.41 | 5854 |
| teleport-2km | a | 5927 | 3.03 | 1.26 | 6355 |
| teleport-2km | b | 4378 | 2.68 | 1.29 | 6355 |
| teleport-2km | c | 4046 | 2.33 | 1.36 | 5882 |
| teleport-2km | d | 5995 | 3.79 | 2.52 | 5828 |

## Verdicts (by the rule above)
| Question | dive | teleport-1m | teleport-2km | Verdict |
|---|---|---|---|---|
| d against c ("does not slow") | tile-ready +20 %, p99 +82 % | +46 %, +62 % | +48 %, +63 % | FAIL in every flight |
| c against a (dual pool against today) | -16 %, +3.2 % | -27 %, -25 % | -32 %, -23 % | PASS in every flight |
| b against a (10 unpinned against 12) | -45 %, +6.3 % | -17 %, -14 % | -26 %, -12 % | PASS on two flights; dive FAILS the median rule on p99 (+0.5 ms), teleport-1m fails the worst-round rule (one round p99 8.25 against 6.24 ms) |
- A saturated E-core pool slows the critical work on the P-cores by far more than the 5 % margin (probable cause: shared last-level cache, memory bandwidth and the power budget; not tested).
- Terrain 6 on the P-cores plus the background pool (c) is no worse than terrain 12 on any core (a) and faster in tile-ready; the teleport flights run fewer jobs (the terrain share is smaller and the prefetch work moved to the E-cores).
- 10 unpinned (b) is no worse than 12 in tile-ready or jobs; its only miss is the dive p99, 8.89 against 8.36 ms.

## Caveats
- 7 of the 9 d runs were marked INVALID by the CPU calibration only (dive rounds 1 and 3, all three teleport-1m, teleport-2km rounds 2 and 3) and are the third attempt; by the rule they are "no verdict" on their own. Their margins are 4 to 15 times the 5 % limit.
- The loaded half was skipped; the threads of the hover pick, the ground read and the coupled run stay unpinned (as in the prototype); no `--lit`.
- Three rounds are few: single rounds vary by up to 10 % (b's teleport-1m p99 ranges 4.66 to 8.25 ms).

## Gate behaviour (ADR 0087's open point on the calibration limits)
- 40 attempts were refused or marked INVALID over the 36 runs: 15 on a, b and c (each retried until valid), 24 on d (7 runs kept at their third attempt) and 1 hard refusal.
- All 39 INVALID verdicts were CPU-calibration limits ("before" or "after", single or parallel), most by under 10 % (for example 53.2 ms against 52 ms, 59.2 ms against 56 ms); the largest was 60.3 ms against 52 ms (teleport-2km, round 1).
- The hard refusal: another lane's cargo and rustc were running at 23:10 (quiet-machine check), and the orchestration script copied the previous log for that run; found at analysis, deleted and the run repeated at 23:14 (valid). Scripts that copy a log after a run must check for "wrote perf" first.
- No run exceeded the 15-minute refusal limit.

## Next step (SPIKE-90 stays open)
A throttled background pool: `PLANET_DUAL_POOL_BG=2` and `4`, or EcoQoS on the E threads, to find a setting where d passes against c. The measurement plan and the verdict rule stay as written.

## Second window: a background setting that does not slow (plan, 2026-11-02)
Prototype: branch `spike/dual-pool-2` at 3af166f1 (from a4878fe9; never merged; patch in `proto/dual-pool-window2.patch.txt`). New knobs, all on the background pool only and only with `PLANET_DUAL_POOL=1` on a hybrid CPU:
| Knob | Switch | What it does |
|---|---|---|
| Pool size | `PLANET_DUAL_POOL_BG=N` (existed) | N background workers on the E-cores instead of one per E thread (8) |
| EcoQoS | `PLANET_DUAL_POOL_ECOQOS=1` | each background thread asks for power throttling (`SetThreadInformation(ThreadPowerThrottling, EXECUTION_SPEED)`, the existing `windows` crate); returns `true` on Rasierklinge |
| Duty cap | `PLANET_DUAL_POOL_DUTY=P` | a background worker pauses `t (100 - P) / P` after a job of `t`: busy at most P % of the time |
- The synthetic load (`PLANET_DUAL_POOL_LOAD=N`) now runs in jobs of about 10 ms that queue their successor (a bake is many short jobs; a cap between jobs could not act on window 1's endless jobs). At exit the run prints `dual pool: synthetic load chunks finished: K`: the background's throughput, so a knob cannot pass by starving the background unseen.
- Smoke (WARP, `test-render --perf-smoke --scene dive --gen-workers 5`, 2026-11-02): every configuration below runs and exits; EcoQoS `true` on every background thread; the duty cap and pool sizes are applied (startup lines); chunks finished in one dive smoke: 1,792 with 8 workers, 967 with BG 2, 646 with duty 50 % (WARP frame times are not evidence).

### Configurations
| | Name | Switches (all with `PLANET_DUAL_POOL=1`, `--gen-workers 5`) |
|---|---|---|
| c | dual pool, baseline | none |
| d-plain | 8 background workers, saturated by the new load, no knob | `PLANET_DUAL_POOL_LOAD=8` |
| d-BG2 | 2 background workers, saturated | `PLANET_DUAL_POOL_LOAD=8 PLANET_DUAL_POOL_BG=2` |
| d-Eco | EcoQoS, saturated | `PLANET_DUAL_POOL_LOAD=8 PLANET_DUAL_POOL_ECOQOS=1` |
| d-Duty50 | duty cap 50 %, saturated | `PLANET_DUAL_POOL_LOAD=8 PLANET_DUAL_POOL_DUTY=50` |
- d-plain is the like-for-like reference for the knobs (the load changed from window 1's endless jobs to ~10 ms chained jobs) and re-checks window 1's verdict "d slows c" with the new load (planner, 2026-11-02).
- Not in this window: BG 4 (more load than BG 2: only worth running if BG 2 passes) and combinations (BG 4 + EcoQoS); a third window takes the best one further if needed.

### Runs and time
- Flights: `dive` (3 rounds) and `teleport-1m` (2 rounds; window 1's teleport-2km behaved like teleport-1m in every verdict); quiet only; configurations rotated per round (round 1 c, d-plain, BG2, Eco, Duty; round 2 d-plain, BG2, Eco, Duty, c; round 3, dive only, BG2, Eco, Duty, c, d-plain).
- 5 configurations x (3 dive + 2 teleport-1m) = 25 runs: `cargo xtask perf stream <flight> --store C:/DEV/SeriousPlanetData/global16384 --gen-workers 5` with the environment of the row; release build of `spike/dual-pool-2` at 3af166f1 in its own worktree.
- Estimate: window 1 took 118 min for 36 runs including 40 refused or INVALID attempts (about 3.3 min per kept run), so 25 runs take about 80 min; without a refusal storm about 60 min. If the window runs short, stop after dive round 3's last configuration; teleport-1m's two rounds come first in the run order of each round.
- Each run also keeps its stderr line `dual pool: synthetic load chunks finished: K` and the run's wall time (chunks per second for the background's throughput).

### Verdict rule (unchanged from window 1, fixed before the run)
- For each knob and for d-plain against c, per flight, medians of the rounds (3 for dive, 2 for teleport-1m): tile-ready (`load_run_us_median` + `load_wait_us_median`) and frame p99 at most 5 % above c, and no knob round more than 10 % worse than the worst c round. A knob passes "does not slow" only in both flights.
- Report for each passing knob its background throughput (chunks per second) against d-plain's, so the owner sees what the pass costs the background.
- With 2 rounds the worst-round rule compares the worse of the knob's two rounds with the worse of c's two.
- A run INVALID by the calibration stays "no verdict" for its own number, as in window 1; a knob with fewer than 2 valid rounds in a flight is "no verdict" there.

## Second window: results (2026-11-02, 09:05-10:55)
Run by the spikes lane on the planner's go (inbox #70); build `spike/dual-pool-2` at 3af166f1 in its own worktree; quiet, gate-decided, never `--machine-ready`; teleport-2km skipped (planner); BG4 not run (time).

### What happened
| Block | Time | Runs | Outcome |
|---|---|---|---|
| 1 | 09:11-09:23 | round 1, both flights | 4 valid of 10; teleport-1m 40 s and dive 77 s per run |
| 2 | 09:23-09:58 | redos of round 1 (up to 3 tries) | all INVALID: the CPU stayed at 60-64 °C, the harness's cool-down hit its 120 s cap on every run (runs took 160 s) and calibration missed by 3-5 %; stopped |
| pause | 09:58-10:07 | | idle (the runner kept going until 09:58: see learnings 2026-11-02) |
| 3 | 10:07-10:51 | rounds 2-3, 45 s idle before each run, at most 2 tries | 52-58 °C, no cool-down waits; loaded runs still INVALID about half the time |
- 42 attempts, 11 valid, 31 INVALID; every INVALID is a CPU calibration limit (single before > 52 ms or after > 56 ms). Every attempt with its temperature and calibration: `raw/window2-attempts.md`.
- Once the CPU was cool (block 3), the INVALIDs are the load itself: c's single-thread calibration is 48.9-49.5 ms, the loaded configurations' 49.9-56.5 ms before and 52.5-58.5 ms after (medians over attempts). The background load on the E-cores slows a single P-core thread by 2-19 %.

### Verdicts by the fixed rule (valid rounds only)
| Flight | Config | Valid rounds | Tile-ready vs c | p99 vs c | Verdict |
|---|---|---|---|---|---|
| teleport-1m | c | 2 | 4.02 ms | 4.52 ms | reference |
| teleport-1m | d-plain | 1 | +10.1 % | +24.9 % | no verdict |
| teleport-1m | BG2 | 0 | | | no verdict |
| teleport-1m | Eco | 0 | | | no verdict |
| teleport-1m | Duty | 1 | +2.4 % | +42.7 % | no verdict |
| dive | c | 3 | 4.24 ms | 8.79 ms | reference |
| dive | d-plain | 1 | +20.3 % | +49.9 % | no verdict |
| dive | BG2 | 0 | | | no verdict |
| dive | Eco | 2 | +33.6 % | +67.4 % | **FAIL** |
| dive | Duty | 1 | +37.9 % | +87.7 % | no verdict |

### Every attempt, valid or not (indicative only, no verdict)
| Flight | Config | Attempts | Calib single before / after ms | Tile-ready ms | Frame p99 ms | Background chunks per run |
|---|---|---|---|---|---|---|
| teleport-1m | c | 2 | 49.1 / 49.5 | 4.02 | 4.52 | |
| teleport-1m | d-plain | 4 | 53.6 / 58.4 | 6.11 | 6.92 | 75,816 |
| teleport-1m | BG2 | 4 | 56.5 / 55.9 | 4.50 | 6.28 | 28,145 |
| teleport-1m | Eco | 4 | 51.5 / 57.8 | 6.17 | 7.11 | 99,975 |
| teleport-1m | Duty | 4 | 53.4 / 52.5 | 4.73 | 7.44 | 37,084 |
| dive | c | 4 | 48.9 / 49.1 | 4.38 | 9.18 | |
| dive | d-plain | 5 | 53.0 / 57.7 | 5.10 | 13.17 | 29,302 |
| dive | BG2 | 5 | 54.7 / 58.1 | 4.41 | 14.23 | 8,145 |
| dive | Eco | 5 | 49.9 / 58.5 | 4.79 | 13.63 | 31,757 |
| dive | Duty | 5 | 50.5 / 56.3 | 4.66 | 14.48 | 14,529 |
- teleport-1m's chunk counts include block 2's 160 s runs (more wall time per run); dive's are like for like.

### Reading
1. No knob passes "does not slow". Eco fails dive by the rule; every other knob has too few valid rounds for a verdict, and every attempt of every knob raises frame p99 by 39-65 % in dive (13.2-14.5 ms against 8.8-9.2 ms) and 39-65 % in teleport-1m.
2. EcoQoS does not throttle the background on Rasierklinge: its chunk count equals or exceeds d-plain's (31,757 against 29,302 per dive).
3. BG2 and the 50 % duty cap cut the background to 28 % and 50 % of d-plain's throughput and still raise dive's p99 by about 55 %: the slowdown is not proportional to the background's work. A shared resource (package power, L3, memory bandwidth) is the likely channel, not E-core scheduling.
4. The calibration check refuses most loaded runs, so "does not slow" cannot pass with any load that measurably slows a P-core thread: the gate and the question are the same measurement.
5. Recommendation (the planner's call): close SPIKE-90's background question as "a saturated background pool slows the critical path on this machine; no tested knob prevents it"; keep the dual pool (c, which passed window 1) without a saturated background, and size background work by a budget (BUDG reserve) instead of a throttle.

## Same window: upload attribution run (for crusader, #1076)
- `main` at 97c4bdd8 (with the frames log's `deferred` and `node_bytes`), `cargo xtask perf stream teleport-2km --store C:/DEV/SeriousPlanetData/global16384 --lit`, 3 rounds, 45 s idle between, all valid on the first try (54-56 °C).

| Round | Frame median ms | Frame p99 ms | Frames with upload bytes over the median (median 0) | Slowest 1 % (6 frames) with an upload | Slowest 1 % with `deferred` > 0 | Mean `upload_ms`, slowest 1 % / all |
|---|---|---|---|---|---|---|
| 1 | 1.98 | 4.28 | 37.5 % | 83 % | 0 % | 0.23 / 0.08 |
| 2 | 1.75 | 4.07 | 37.7 % | 100 % | 0 % | 0.23 / 0.08 |
| 3 | 1.91 | 4.81 | 37.8 % | 100 % | 0 % | 0.27 / 0.09 |
- p99 median 4.28 ms: the Tracked 4.5 -> 6.2 ms rise did not reproduce in this window (all three rounds at or under 4.81 ms).
- The slowest frames carry uploads (5.5 of 6 against 38 % of all frames), but the upload call itself takes 0.2-0.3 ms; no frame was upload-capped (`deferred` 0 everywhere).
- Their time is in `record_ms` when `node_bytes` is the full node buffer (288,420 bytes: record 2.1-5.1 ms in frames 78-101 of each round) and, in frame 14 of every round (5.97-6.94 ms with record 0.55-0.71 ms), outside the logged phases.
- Frames logs and JSON stay local (`target/perf-upload/` in the `spikes-main` worktree).

## Visuals
Timings only: no images.
