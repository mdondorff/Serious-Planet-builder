## What exists
- Core shares and counters: `planet_streaming::jobs` (`JobSystem`, `Share` with ceiling, `running()`, `peak_running()`), the terrain share with the tile pool and the plan worker (merged).
- Frame timing: `planet_perf::streaming::FrameRecord` (`plan_ms`, `scheduler_ms`, `upload_ms`, `record_ms`, `gpu_ms`, `wall_ms`), the stall probe's phase marks (VIEW-056), GPU timestamp queries (`GpuContext::timestamps_supported`).
- Loaded and pinned runs: ADR 0087; `planet_perf::load` records competing load.

## Design (to confirm with the planner)
1. **Loads are calibrated work, not sleeps.** A `SyntheticLoad` has a name, a main-thread ms per frame, a GPU ms per frame and a core count. The CPU spin is a fixed-iteration integer loop whose iteration count is calibrated at start to the target ms on the machine (median of repeated runs, checked against the clock), so a slower machine spins fewer iterations for the same ms; the check is a tier-A test on fake clocks plus a real-clock tolerance test (5 %).
2. **Cores through the job system.** Each reserved load owns a `Share` of its BUDG-003 ceiling and keeps `ceiling` spin jobs of the idle class running for the whole run (a job per core, re-spawned when it ends), so the load is a real competitor for the 20 hardware threads and the terrain share's ceiling is what keeps terrain inside 12.
3. **Main thread and GPU per frame.** The scene calls `load.frame()` once per frame on the frame thread (spin for the main-thread ms) and encodes the GPU burn pass (a compute pass calibrated with timestamp queries to the GPU ms) before the terrain passes. Terrain's own main-thread time is the frame's record time less the loads' spin; terrain's GPU time is read from its own timestamp pairs.
4. **Verdict.** For each standard flight, with loads and without: terrain medians within BUDG-001/002/003 and the frame median within 16.6 ms; the difference between the runs is reported as the cost of the reserves on terrain. Under ADR 0087 the median must clear 16.6 ms by the loaded margin (5 %) or the verdict goes to the window queue.
5. **Windows.** The scene is a CPU- and GPU-timed scene: pinned-load mode (ADR 0087 point 4) and GPU timestamps; the planner opens the window; the run is not run unprompted.

## Decisions (opus-planner, inbox #6, 2026-10-31)
1. IDs: BUDG-011 (synthetic load definition and calibration, tier A) and BUDG-012 (the reserve scene and its verdict, tier C). BUDG-013 is held for this change.
2. Standard flights: the gate's streaming flights (dive, look-1m, look-2km, teleport-1m, teleport-2km). The lit flights join after ADR 0085's build.
3. The GPU burn pass is a plan node of a synthetic scene in `planet_render` (CON-03: the renderer executes a plan, the frame crate decides). Only the plans of test scenes carry it; a production plan never does.
4. The job system is complete (change `job-system`, BUDG-004, BUDG-008 to BUDG-010): every pool is on a share, so the reserve test is meaningful.

## Windows under ADR 0087
- The scene runs the two variants (loads on, loads off) alternately inside one session, so the cost of the reserves on terrain is a relative reading.
- Absolute verdicts follow BUILD-031: the frame median with streaming needs a quiet window; GPU timings carry the +18 % note; CPU times need pinning or 30 % margin. A loaded run records everything and gives `loaded pass`, `inconclusive` or `needs a quiet window` per row.
- The planner is told before each measured run and gives the go; the run is a loaded A/B (`perf --ab` for a comparison of builds) unless the planner asks for a quiet window.
