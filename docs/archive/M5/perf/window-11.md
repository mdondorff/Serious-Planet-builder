# Perf window 11 (sixth M5 window), 2026-10-18

Rasierklinge, main a32dafb or later (includes #275 beach fix REND-030 with `shore_filter` default 0, the exact dry-tile cut in every lit path #293/#296, #302 compact slot v2, #314 rust-lld); `cargo xtask perf m5` once over the 16384 store (re-pre-filled under MESH_VERSION 8), no `--machine-ready`. A regression check of the final M5 code, not a gate.

## Verdict

`lit` and `lit-formats` sessions VALID (frame and GPU), the seven streaming scenes VALID (frame verdicts), 0 gated measures over budget, no run refused. `accept M5` hardware criteria PASS. The `accept` run itself exits 1 on its tier C software step: 9 failures in `test gpu` (software adapter, run right after the window): 7 failed (where checked, `wgpu device lost (Unknown)` with `timestamp map failed` or `readback map failed`, or an editor smoke exiting 1) and 2 are stack-overflow aborts (0xc00000fd) in `gpu_water_coverage` and `gpu_stream_perf`; both abort tests pass alone (10 of 10 in 65 s). The WARP device loss is the one the removal change's smoke remedy (#304) addresses; the aborts are new and unexplained (load only, passing alone).

## Lit views, window 10 to window 11 (ms median; clock)

| View | terrain | atmosphere | water (<=2; coast-3m <=2.2) | frame median / p99 |
|---|---|---|---|---|
| ground-1m | 1.083 -> 1.083 | 0.209 -> 0.211 | not in view | 3.128 -> 2.908 / 3.471 -> 3.428 |
| aerial-20km | 0.790 -> 0.776 | 0.236 -> 0.235 | 0.112 -> 0.108 | 2.372 -> 2.332 / 2.893 -> 3.071 |
| coast-2km | 1.060 -> 1.024 | 0.258 -> 0.253 | 1.869 -> 1.647 | 4.737 -> 4.523 / 5.423 -> 4.921 |
| coast-3m | 1.347 -> 1.312 | 0.235 -> 0.233 | 2.160 -> 1.938 | 5.851 -> 5.586 / 6.442 -> 6.083 |
| orbit | 0.052 -> 0.053 | 0.679 -> 0.684 | 0.168 -> 0.169 | 1.463 -> 1.485 / 2.260 -> 2.078 |

Clocks: ground-1m P0 1590, aerial-20km P0 1424 (valid at the floor), coast-2km P0 1312, coast-3m P2 1327, orbit P0 1462. coast-2km and coast-3m are below the 1,400 MHz floor and within budget (ADR 0057, ADR 0060). Water nodes: coast-2km 1,082 -> 844 and coast-3m 1,181 -> 840, equal to the CPU run; "kept with no minimum" is 0 on every line. Coast water down 12 % (coast-2km) and 10 % (coast-3m), where 0.17 ms was predicted for coast-3m (0.22 ms found).

## Coastal flights (report only; water ms median)

| Flight | default run (filter 0) | clock | filter 1 | clock | frame p99 default / f1 |
|---|---|---|---|---|---|
| glide | 4.481 | P5 817 | 2.165 | P0 1327 | 15.093 / 6.869 |
| descent | 4.415 | P8 277 | 2.233 | P0 1372 | 30.748 / 6.623 |

Window 10 (default was filter 1): glide 4.650 at P5 802 and 2.302 at P0 1327 for filter 0; descent 4.260 at P8 262 and 2.450. The unsuffixed run (the first of each pair) collapses to P5/P8 in both windows, whichever filter it carries: the clock collapse follows run order, not the shore filter, so it is not a cost of the filter. The second run of each pair is at P0 1327-1372 (floor not reached). Node counts: glide 1,050, descent 1,150 of 2,597/2,598 (was 1,372 / 1,495).

## Streaming scenes (lit; window 9 logs compared for dive-lit and teleport-1m)

| Scene | frame median | p99 | max |
|---|---|---|---|
| dive-lit | 5.41 -> 5.33 | 13.04 -> 13.57 | 21.70 -> 15.73 |
| teleport-1m-lit | 6.12 -> 6.65 | 8.90 -> 8.34 | 11.08 -> 10.40 |
| teleport-2km-lit | 4.07 | 5.40 | 6.88 |
| look-1m-lit | 3.77 | 5.57 | 7.93 |
| look-2km-lit | 2.03 | 3.05 | 5.62 |
| dive-fast-lit | 8.96 | 15.71 | 18.07 |
| dive-cold-lit | 5.46 | 15.40 | 17.14 |

dive detail lag mean 0.426 -> 0.319 levels (worst 2.74). Terrain slots 219,585 of 219,585 on every scene (ADR 0063 pool, 15 blocks). RAM at most 1.59 GiB (dive-cold). No scene over budget; teleport-1m median is +8.7 % (6.12 -> 6.65), within the 10 % noise rule; attribution not established (candidate: the larger slot read per tile, not checked).

## lit-formats: compact against vertex (GPU median ratio)

| View | terrain | atmosphere | water | GPU span |
|---|---|---|---|---|
| aerial-20km | 1.264 (0.780 / 0.617) | 1.027 | 1.060 | 1.155 |
| coast-2km | 1.171 (1.039 / 0.888) | 0.986 | 1.158 (1.670 / 1.442) | 1.135 |
| orbit | 1.020 | 1.009 | 1.013 | 1.011 |

ground-1m ratio not comparable (clock states differ). The compact form (slot v2, wide texel) costs 17-26 % more on terrain and 6-16 % more on water than the vertex form on the aerial and coast views; absolute terrain stays below 1.1 ms and the frame stays far under budget. One regression line (not gated): lit-formats/ground-1m/vertex cpu_ms 1.01 -> 1.17 ms (+16 %).

## Open

Compact slower than vertex on terrain and water (to watch in the removal window); flights' first run at P5/P8 whichever the filter (open: why, and whether the cool-down or the previous scene leaves the GPU in a low state); the tier C software step (WARP device loss, stack-overflow aborts) fails inside `accept` while the hardware verdict passes.
