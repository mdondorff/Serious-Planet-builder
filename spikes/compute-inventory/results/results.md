# SPIKE-44: Runtime compute inventory

Opened: 2026-10-26
Question: What threads, pools and job queues does the app run today, what does each use of the cores and of a 16.6 ms frame?
Why it matters: the ADR "runtime compute budgets" (owner decision 2026-10-26: game/viewer mode targets 1440p at 60 fps on Rasierklinge) needs measured shares per subsystem, a shared job system with a core budget, and memory budgets.
Time box: 1 day of agent work
Success criteria: (1) the machine; (2) every pool and long-lived thread with owner, count, sizing, priority, wait; (3) busy share per pool, main-thread CPU split, GPU time per pass, RAM and VRAM peak in the scripted flight and in an editor session with stamps; (4) where the pools oversubscribe the cores; (5) a first table of what terrain uses of a 16.6 ms frame and of the cores.
Fallback if it fails: the static inventory (2) and the machine (1) alone, with the measured parts named as open.
Code location: spikes/compute-inventory/ (read-only; no behaviour change)
Requested by: opus-planner, 2026-10-26.

## 1. The machine (Rasierklinge)
| Item | Value |
|---|---|
| CPU | Intel Core i7-12800H, 14 cores (6 P with SMT + 8 E), 20 logical processors |
| RAM | 31.7 GB |
| GPU | NVIDIA GeForce RTX 3080 Ti Laptop, 16384 MiB VRAM |

## 2. Static inventory (grep of crates/*/src, 2026-10-26, main at fa96d481)
Runtime = reachable from `planet editor` / the viewer window. Tool-only = bake, perf, stills, tests.

| Name | Owner | Count | Sizing | Priority | Waits on |
|---|---|---|---|---|---|
| main thread (winit loop, render, upload, scheduler steps) | app/window | 1 | fixed | normal | GPU present, locks (stall probe names them) |
| `pool-worker-N` tile generation pool | streaming `ThreadPool`, `GenWorkers` | cores - GEN_RESERVE(2) = 18 | cores-based, `--gen-workers`/`--gen-reserve` | below-normal | job channel (one shared FIFO, no classes) |
| plan pool `pool-worker-0` | frame `PlanWorker` | 1 | fixed | normal | plan request; the job runs the plan on `threads` = 20 scoped threads inside |
| `heights-worker` scoped threads | generators `height` (plan job, base heights) | up to 20 per call | cores-based | inherits | the plan job |
| A-net chunk pool | window.rs `GenWorkers::new(gen/2)` | 9 | half of the tile pool | below-normal | chunk jobs (`ChunkConfig.workers` = 9 at once) |
| horizon jobs | shares the A-net pool (`HorizonConfig.workers` 1) | 0 (jobs on the A-net pool) | 1 at once | below-normal | settle 1 s after an edit |
| `anet-load` | app `WorldTerrain::request_anet` | 1, once at start | fixed | normal | store read, then spawns horizon enable |
| `coarse-horizon` | app `horizonworld`, `generators/coarsehorizon` | 1 + scoped march threads | `min(cores, 4)` for enable | normal | once per run |
| `reduce-bake`, `bake-*`, `lattice-gather`, `water-heights`, `atmosphere-rows` | tool paths and start-up steps | scoped, up to cores / 8 | cores-based | normal | the step |
| `coupled-run` | app `coupledworld` | 1 | fixed | normal | tool path (the coupled run) |
| `clock-sampler` | perf `streamperf` | 1 | fixed | normal | perf only |
| driver threads | wgpu, winit, egui, the OS audio-less stack | not counted by the app | driver | driver | not controlled |

Notes:
- Nothing uses rayon. Pools are the hand-written `ThreadPool` (one mpsc queue behind a mutex, jobs in submission order, no classes) or `std::thread::scope` threads created per call.
- Measured (section 3): the running viewer has pool threads named `pool-worker-N` in three pools (tile 18, chunk 9, plan 1; the names repeat), `horizon-bake` (4 thread ids seen; the horizon jobs do not run on the A-net pool as the table above assumes), `heights-worker` (scoped, created per plan job), `main`, and 4 `D3D Background Thread` (driver).
- The plan job is the largest consumer: each plan creates up to 20 `heights-worker` threads (354 threads in 60 s, about 6 plans a second in the flight), outside every pool budget.
- No priority exists beyond the pool-wide below-normal of the tile and chunk pools (STRM-020, ADR 0046).

## 3. Measured use
Method: `planet editor --store C:/DEV/SeriousPlanetData/global16384 --site 30,-110`, key `c` sent with SendKeys (crusader's recipe), release build of main at fa96d481. A PowerShell sampler read per-thread kernel+user CPU time (GetThreadTimes) and thread name (GetThreadDescription) of the one process every 0.25 to 1 s, the window title every 0.5 s (fps, CPU ms and GPU ms), and `nvidia-smi` once a second (VRAM, GPU load). The sampler scripts are not committed (no `.ps1` in the repo); raw samples are `flight-threads.csv.gz`, `steady-threads.csv.gz`, `flight-gpu.csv`, `flight-names.csv`.
Caveats: other sessions ran their own viewers on the machine; thread data is per PID, titles are filtered to the segment that carries this store path. No perf window, so every number is indicative, not a gate measurement. The viewer computed chunks on demand at 30N 110W and appended them to the store. The flight lasted longer than the 290 s sampled.

### 3a. Scripted flight (orbit to 1 m, first 290 s, sampled at 1 s)
| Quantity | Value |
|---|---|
| Cores busy (sum of thread CPU per second) | median 5.4, p90 13.0, max 17.5 of 20 logical |
| Threads created | 1,768 distinct thread ids, about 1,720 short-lived scoped threads |
| Short-lived scoped threads | 3.3 cores mean, the largest share (understated: a thread's last sample is before it exits) |
| `pool-worker-0` (three pools share the name; the plan pool dominates) | 0.73 cores |
| tile pool workers | about 0.05 cores each |
| `main` | 0.42 cores |
| Process RAM | working set 2.57 GB max, private 3.1 GB max |
| VRAM | 1,683 to 1,734 MiB |
| GPU load (nvidia-smi) | mean 37 %, max 64 % |

### 3b. Flight end, 1 m altitude, 60 s steady (sampled at 0.25 s, names kept)
| Quantity | Value |
|---|---|
| Frame rate (title) | median 39 fps, minimum 2 |
| Frame CPU (title) | median 21.3 ms, p90 60 ms, max 635 ms |
| Frame GPU (title) | median 2.3 ms, max 5.3 ms |
| `heights-worker` scoped threads | 2.35 cores (354 threads in 60 s) |
| `horizon-bake` | 1.03 cores (16 thread ids) |
| `pool-worker-N` (all pools) | 0.65 cores |
| `main` | 0.33 cores |
| unnamed or exited | 0.21 cores |
| total | 4.7 cores of 20 |

### 3c. Editor session with stamps: not obtained
`--edit` at orbit (alt 19,623 km), key 3 (hill) and four mouse clicks at the window centre: the session ran idle (57 fps median, frame CPU 2.3 ms, GPU 0.23 ms, 0.19 cores in total, RAM 1.4 GB) and nothing shows that a stamp was placed (the title does not show edits and no republish work appears). The editor-with-stamps numbers are open: a run needs stamps placed near the ground and a way to see the edit count.

## 4. Oversubscription
- Threads that can run at once: tile pool 18, chunk pool 9, plan pool 1, up to 20 scoped plan threads, horizon jobs and main: more than 48 against 20 logical processors.
- Measured over the flight: p90 13.0 and max 17.5 busy cores of 20; the bursts come from the scoped plan threads.
- The scoped `heights-worker` threads are outside any budget: sized by `available_parallelism` per plan, not by the tile pool's reserve (GEN_RESERVE 2).
- The P and E cores differ (6 P with SMT, 8 E); no thread is pinned or classed.

## 5. First table: terrain's share of a 16.6 ms frame and of the cores (1 m altitude, flight end, 60 s steady)
| Item | Measured | Share |
|---|---|---|
| Frame budget at 60 fps | 16.6 ms | 100 % |
| GPU frame time, median | 2.3 ms | 14 % |
| Frame CPU (title), median | 21.3 ms, p90 60 ms | over the budget |
| Frame rate | 39 fps median | below 60 |
| Cores in use (all pools, scoped threads, main) | 4.7 of 20 | 24 % |
| of which the plan job's scoped threads | 2.35 | 12 % of the cores |
| of which horizon bake | 1.0 | 5 % |
| of which the tile and chunk pools | 0.65 | 3 % |
| of which main | 0.33 | 2 % |
Reading: at 1 m in the flight the cores (15 of 20 idle) and the GPU (2.3 ms) have headroom, yet the title's frame CPU is over the 60 fps budget: the main thread uses 0.33 core, so the frame waits (the stall probe names the phases) and is not bound by compute. The phase split of the main thread was not collected in this window.

## Open
- The editor session with stamps (3c).
- The main-thread phase split per frame (stall report or F3) and per-pass GPU time (the title has the frame GPU time only).
- Memory per subsystem (peak private 3.1 GB, VRAM 1.7 GB in the flight, no split).
- Inputs for the ADR "runtime compute budgets": steady terrain use is a median 4.7 of 20 logical cores and 2.3 ms of GPU, with bursts to 17.5 cores; half the CPU is per-call threads outside any pool.

## Visuals
No images: the question is thread counts and CPU time; numbers are in the tables, raw samples next to this file.

## Step 2: where the 1 m frame spends its time (opus-planner, 2026-10-26)
Method: a local, never-merged patch of the stall probe (`crates/app/src/stall.rs`, `window.rs`, reverted) wrote every frame's 16 phase times and lock waits (VIEW-056) to a file, and the timed frame (one a second, which waits for the GPU) wrote the per-pass GPU times (REND-028, `TIMESTAMP_QUERY` is supported on the RTX). Release build of main at fa96d481 (after #737), `planet editor --store C:/DEV/SeriousPlanetData/global16384 --site 33,-100`, the scripted flight, the 60 s at 1 m altitude of flight time 690 to 750 s; a second 60 s with the thread sampler running on the process shows the same frame times (60.4 fps against 59.6), so the sampler does not disturb the frame. Raw rows: `step2-phases-flight.txt.gz` (last 8,000 frames; one line per frame: frame ms, phase=ms;..., lock waits), `step2-passes-flight.txt`.

### Correction of finding 3
Step 1 reported 39 fps and a frame CPU of 21 ms at 1 m. This run, on the same code, shows **59.6 fps, mean frame 16.8 ms, median 15.8 ms, p90 18.2 ms**: at 1 m the frame is paced by vsync (Fifo at 60 Hz), not late. The step 1 steady window was measured while other sessions' viewers and builds ran on the machine; the cause of the difference is not isolated (machine load is the likely one), so the 39 fps figure is not a property of the app and is withdrawn as a headline. What stays: the p99 frame is 60 ms and the max 760 ms (below).

### Phase split of a 1 m steady frame (3,578 frames in 60 s; mean frame 16.8 ms)
| Phase | Mean ms | Median | p90 | Max | Share |
|---|---|---|---|---|---|
| editor panels (egui) and present | 7.44 | 7.96 | 13.4 | 78 | 44 % |
| plan and residency (`begin_frame`) | 4.78 | 4.10 | 6.8 | 124 | 29 % |
| render submit | 2.61 | 1.81 | 3.7 | 70 | 16 % |
| chunk frame, refusals, editor update | 0.94 | 0.12 | 0.26 | 729 | 6 % |
| horizon frame and upload | 0.38 | 0.31 | 0.61 | 58 | 2 % |
| wait for the GPU (frames in flight) and lit plan | 0.31 | 0.22 | 0.38 | 51 | 2 % |
| title, exposure readback | 0.25 | 0.00 | 0.00 | 50 | 1 % |
| the other eight phases | under 0.1 | | | | 0 % |

- The first phase holds the present call: with Fifo it blocks until the vblank, so it is the idle share; the frame's work is about 9 ms of 16.8 (CPU headroom about 7 ms at 60 fps).
- `begin_frame` is the largest work phase (4.8 ms mean, 124 ms max): the plan result, the streamer steps and the residency for the frame.
- Lock waits of 2 ms or more on the main thread: 3 in 60 s, all `horizon jobs lock (read)`, 36 ms in total. No other lock (plan, chunk, definition) waited 2 ms or more; the heights-worker join happens in the plan job on the plan pool, which the main thread polls, so it does not appear as a main-thread wait.
- Slow frames (over 30 ms): 115 of 3,578 (3.2 %); the dominant phase of those: `begin_frame` 46, render submit 23, present 20, chunk frame 15, the rest 11. The tail is plan, streamer and submit work (and a chunk landing: the 729 ms `chunk frame` max), not a lock.

### GPU time per pass, timed frames at 1 m (median / max, ms)
| Pass | Median | Max |
|---|---|---|
| terrain | 3.39 | 7.03 |
| sky composite | 0.16 | 2.78 |
| aerial perspective | 0.20 | 0.36 |
| sky view | 0.12 | 0.22 |
| exposure histogram + adapt, tonemap | 0.11 | |
| water | not drawn here | |
| frame GPU span | 4.28 | 8.48 |
`TIMESTAMP_QUERY` works on the RTX, so the per-pass times are available in the viewer; the title shows only the terrain pass as "gpu". The timed frame costs a GPU wait, so it is a separate frame from the paced ones (its `total` is 7.3 ms median).

### Editor session with stamps (item 3): not obtained
Two attempts failed on input, not on the app: the SendKeys `c` and `p` keys did not freeze the flight (the flight restarts or loops, about 85 s per dive), the mouse clicks of the step 1 script and a second attempt never reached an edit (a script variable bug, then SendKeys "access denied"), so no stamp was placed near the ground and no edit-frame phases were recorded. To get it: a viewer start option for the free camera at a given altitude or a scripted edit (a `--smoke`-style script with stamps) would replace the key injection; a person at the keyboard places the stamps in a minute.

### What this means for the 60 fps target (numbers only)
- At 1 m, flight, 1440p on the window size used, RTX 3080 Ti, the frame holds 60 fps on vsync with about 7 ms of CPU and 12 ms of GPU headroom (GPU 4.3 ms span).
- The risks are the tail (p99 60 ms, max 760 ms, from `begin_frame`, render submit and chunk landings) and the CPU use of the worker side (4.7 to 5.4 cores, half of it the plan job's per-call threads, section 3), which competes with the main thread when other sessions or a game's own threads load the machine.

### Patch notes (local only, reverted; a debug switch is a tracked item)
- `crates/app/src/stall.rs`, `StallProbe::end_at`: before the stall test, when the env var `PLANET_PHASES` names a file, append one line per frame: `frame_ms,phase=ms;phase=ms;...|lock=ms;...` (phases in run order, waits from the thread-local of the main thread, 2 ms or more).
- `crates/app/src/window.rs`, after `st.gpu_ms = timing.gpu_ms` in the timed frame: when `PLANET_PASSES` names a file, append `cpu, total, gpu, span, copy` and `timing.passes.get(p)` for each `GpuPass::ALL` pass.
- Both write with `OpenOptions::append` per frame (about 40 µs); no change to what a frame does. Launch: `PLANET_PHASES=<file> PLANET_PASSES=<file> planet editor --store <store> --site LAT,LON`.
