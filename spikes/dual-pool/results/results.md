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

## Visuals
Timings only: no images.
