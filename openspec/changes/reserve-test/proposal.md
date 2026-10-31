## Why
ADR 0083 point 3 and BUDG-006: a share that no subsystem has claimed stays unused, and the perf harness runs a synthetic load in each reserved share at its full size to show that terrain stays inside its own shares and the frame inside 16.6 ms. Without the test the reserves are paper: sky and weather, physics, game logic and networking will claim them later, and nothing says terrain survives that. The job system (change `job-system`, BUDG-004 and BUDG-008/009 merged) gives the core shares their ceilings and counters; the reserve test is its first measured use. Planned item B3 (planner, 2026-10-31).

## What Changes
1. Synthetic loads, one per reserved share at its full size (BUDG-001 to BUDG-003): sky and weather (1.5 ms main thread, 3.0 ms GPU, 1 core), physics (2.0 ms main, 0.5 ms GPU, 2 cores), game logic (2.0 ms main, 2 cores), networking (0.5 ms main, 1 core). A load does fixed work calibrated to milliseconds, not a sleep: a spin on the frame thread per frame, a spin job of the full frame period on its share for the cores, a GPU pass calibrated with timestamp queries.
2. A reserve scene in the perf harness: the standard flights (the streaming scenes' flights) with all loads on, against the same flights with none; per flight the median and p99 of terrain's main-thread time, its GPU time, the threads busy for terrain (the `terrain` share's peak and running counts), and the frame time.
3. The report of BUDG-007: each share with measured value, limit and verdict, the largest overrun named; the reserve verdict is PASS only if terrain stays within its shares and the frame median within 16.6 ms with the loads on.
4. Spec: BUDG-006 stays the requirement; new requirements (IDs from the planner) for the load calibration (tier A) and the scene and its report (tier C).

## Impact
- New code: `planet_perf` (load definitions, calibration, report; GPU-free), `planet_render` (the GPU burn pass behind the timestamp calibration), `planet` (the scene and its options), `xtask perf reserve`.
- Needs a perf window on Rasierklinge (planner opens it; ADR 0087 rules: CPU scenes count only when pinned, a loaded PASS is valid at a 5 % margin). No `cargo xtask perf` before the planner's go.
- Touches the job system's shares: one share per reserved subsystem with the BUDG-003 ceilings (sky 1, physics 2, game logic 2, networking 1), created by the scene, not by the app.
