# gpu_skeleton::the_draw_flag_selects_the_path_and_every_loop_draws_the_same_frame_on_either: why it takes 450 s locally

Read-only diagnosis, 2026-11-12, spikes lane (BelowNormal, one test process at a time, bulldozer's go; no code or config change, no fix tried). Machine: Rasierklinge, 20 logical cores, Windows 11 (build 26300), WARP ("Microsoft Basic Render Driver") for `--adapter software`, the RTX 3080 Ti Laptop for `--adapter hardware`. Release `planet.exe` of main a74445d0 for the per-loop timings; the dev-profile test binary under nextest for the test itself.

## Result
- **The test is healthy alone and slow only under CPU oversubscription, and only the two `--pipelined` loops are slow.** Alone: 15.3 s for the whole test under nextest (with its sibling beside it), 2.0 to 3.0 s for each of the five loops on WARP (5 repeats), about 1 s each on the RTX (3 repeats). With busy-loop processes (BelowNormal, same priority as the run) beside it, the three single-frame loops stay at 2.5 to 7 s while the two pipelined loops take 28 to 69 s each at about the same CPU time (9 to 10 s): wall time follows scheduling latency, not work. The RTX is immune (1 s per loop at 40 busy processes).
- The 450 s limit is the declared limit of the test (BUILD-003, `.config/nextest.toml`, 5 x 90 s): at 450 s nextest terminates it, so a "timeout" there means the test did not finish in 450 s, not that a loop hangs (every loop finishes alone and under load; the slowest single loop measured was 69 s at 80 busy processes).
- **Dated:** the local logs put the step between 2026-11-09 04:47 (last 25 s pass, suite 806 s) and 2026-11-09 15:46 (first slow run: ABORT at 126 s, then 450 s timeouts); the status note ties it to the display changes of that day (not in the repo). Not code: no commit touching the loop, the nextest config or the scene landed in that window (`git log` of `.config`, `xtask/src/testcmd.rs`, `crates/app/src/pipelined.rs`, `skeleton.rs`, `crates/render`, `crates/streaming` for 2026-11-08 20:00 to 2026-11-09 18:00: two commits touch those paths: the class-source test slots #1269 at 13:03, a nextest slot count for the class-source store tests, and the preview-latency scene d575fd4d at 16:29, after the first slow run at 15:46).

## 1. Each loop alone (the five command lines of the test, `planet test-render --scene terrain --script aerial-200km --view relief --adapter <a> --draw ... --out ...`)
| Loop | WARP, 5 repeats (wall, s) | RTX, 3 repeats (wall, s) |
|---|---|---|
| per-draw | 2.0 to 2.6 | 0.8 to 2.0 |
| indirect | 1.9 to 2.1 | 0.8 to 0.9 |
| indirect, streamed | 2.0 to 2.2 | 0.9 |
| indirect, pipelined (20 frames) | 2.8 to 2.9 | 0.8 to 0.9 |
| per-draw, pipelined (20 frames) | 2.7 to 3.0 | 0.8 to 1.0 |
- The pipelined runs use about 9 to 11 CPU seconds in 2.8 s of wall (several threads); the other three use about 2 CPU seconds. A pipelined run is 22 or 23 iterations for the 20 frames (cover 1, stale 18, exact 1), with 18 waits for the GPU on WARP (peak 2 of 2 frames in flight) and 0 on the RTX.
- The test itself under nextest (dev profile, both loop-test siblings at once): PASS 15.3 s and 16.1 s (`raw/nextest-solo.txt`).

## 2. The loops under busy-loop load (one run of the five loops each, BelowNormal; busy-loop processes at BelowNormal on 20 logical cores)
| Busy processes | per-draw | indirect | indirect, streamed | indirect, pipelined | per-draw, pipelined |
|---|---|---|---|---|---|
| 0 (WARP) | 2.3 | 2.1 | 2.0 | 2.8 | 2.8 |
| 20 | 2.5 | 2.6 | 2.5 | 4.9 | 27.9 |
| 40 | 2.5 | 2.5 | 2.8 | 36.1 | 30.5 |
| 80 | 2.6 | 6.9 | 3.0 | 48.9 | 69.1 |
| 40 (RTX) | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
- The pipelined loop sleeps 2 ms per iteration (`pipelined.rs`, and 1 ms polls in lockstep) and waits for the GPU between frames; each of its iterations is a chain of cross-thread hand-offs (main, plan worker, the WARP rasteriser threads). Under oversubscription every hand-off waits for a time slice, so the wall time of a 20-frame run is the number of dependent wake-ups times the scheduling latency. A single frame has few hand-offs and does not feel it.
- Scale: the real mix is up to 8 gpu-tier test processes (`gpu-tier` = 8 threads) of which each WARP device spawns its own rasteriser threads, plus whatever the other lanes run (bakes, sweeps, prepushes); the test sits in `warp-heavy` (one at a time of that group) and so runs late in the queue, beside the other long tests. Two pipelined runs at 100 to 200 s each give the 450 s.

## 3. History of the test in the local `test gpu` logs (all worktrees; `raw/draw-flag-test-history.txt`, nextest summaries)
| Period | Runs | PASS (s) | TIMEOUT | ABORT |
|---|---|---|---|---|
| 2026-11-02 to 11-03 09:00 | 30 | 30 at 21 to 35 (median 26) | 0 | 0 |
| 2026-11-03 09:42 to 15:07 (the original incident, limit 300 s) | 10 | 0 | 9 at 300 s | 1 at 32 s |
| 2026-11-04 to 2026-11-08 | 1 | 0 | 1 at 454 s (11-06, 450 s limit) | 0 |
| 2026-11-08 to 2026-11-09 15:00 | 13 | 13 at 24 to 41 (median 25.5) | 0 | 0 |
| since 2026-11-09 15:00 | 44 | 18 at 29 to 206 (median 83) | 18 at 450 s | 8 (0xc0000005 in `d3d10warp.dll`, at 41 to 157 s) |
- The suite itself went from 806 to 890 s (11-08/09) to 1,150 to 1,830 s since; most tests are about as long as before (heavy tests 1.0 to 1.3 x), and the ones that slowed most are the ones that run a pipelined loop: the draw-flag test 24 -> 450 s, its sibling `the_upload_flag_selects_the_write_path...` 44 -> 104 s (x2.4), the coastal flight `a_coastal_flight_at_low_altitude_flickers...` 53 -> 121 s (x2.3), `the_terrain_perf_workload_refuses_unready_machines...` 53 -> 99 s (x1.9), the envelope smoke 47 -> 70 s (x1.5) (`raw/pre-post-nov09-durations.tsv`: the nextest logs of 11-09 04:47 and 11-09 17:46 on the same worktree).
- The aborts are the old WARP access violation (`spikes/warp-crash`), unrelated to the timing: the test is one of the tests that fall to it.

## 4. Reading (no fix tried; no verdict)
- Which loop: both pipelined loops (and, to a lesser degree, anything that waits on the GPU between frames); the three single-frame loops do not stall. WARP versus RTX: WARP only; the RTX does not feel the load.
- What changed: not the test or the code; the amount of CPU the machine is busy with while `test gpu` runs. The tier-C run is a mix of up to 8 processes with WARP threads, and since 2026-11-09 the lanes also run bakes and sweeps beside it; with the test late in the queue the pipelined runs find the machine oversubscribed.
- Levers the architect may weigh (none tried): a limit above 450 s for this test (not a fix of the cause); run the two pipelined loops of the test with the machine otherwise idle (a heavier group); fewer frames in the pipelined runs (each frame is a chain of hand-offs; the earlier note that fewer frames would not remove the load held for the per-run setup cost, not for the hand-offs); a process priority above the busy lanes' for the test run; replace the 2 ms sleeps and the waits for the GPU with a wake-up that does not wait for a time slice (a change in `pipelined.rs`, which the production loop shares).
- Not done: the full 8-process mix was not reproduced (one test process at a time, by the order); the busy-loop processes stand in for it. No per-loop timing inside the test (the test calls `planet::run` five times in one process; the per-loop numbers are the command lines run alone).

## Reproduction
- `cargo build --release -p planet`; the five command lines of section 1; `cargo nextest run --no-run -p planet --test gpu_all` then `cargo nextest run -p planet --test gpu_all -E "test(the_draw_flag_selects_the_path_and_every_loop_draws_the_same_frame_on_either)"`.
- Busy-loop scan: N `pwsh` processes at BelowNormal each looping for 8 minutes beside the five command lines (BelowNormal); raw in `raw/skel-*.log`.
