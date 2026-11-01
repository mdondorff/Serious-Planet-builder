Opened: 2026-11-01
Question: Does a tiered dual pool (critical work on the P-cores, background work on the E-cores) keep the frame and the visible streaming at least as good as today, also while the background pool is saturated?
Why it matters: owner, 2026-11-01, on SPIKE-89: distrust the E-cores for critical work; divide only the P-core capacity among the shares (terrain about 6 on Rasierklinge) and let the E-cores serve background work, as long as it never slows the critical work. A measurement decides.
Time box: 3 h (Sonnet; prototype and plan); the measurement is explorer's, in a perf window on the planner's go.
Success criteria: a minimal prototype behind a switch; a measurement plan with four configurations, three flights, loaded and quiet, three interleaved rounds; the verdict rule stated before the run.
Fallback if it fails: the plan alone, with the prototype's limits.
Code location: branch `spike/dual-pool` (a4878fe9; never merged); patch in `proto/dual-pool.patch.txt`.
Status: open, ready for the perf window (the results section is filled after the run).

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
To be filled after the perf window.

## Visuals
Nothing to look at yet: timings only.
