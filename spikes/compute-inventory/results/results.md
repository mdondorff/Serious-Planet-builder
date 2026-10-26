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
