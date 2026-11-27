# Perf window 8 (third M5 window), 2026-10-18

Rasierklinge, main 42508bf (includes #254, #257, #258, #261); `perf m5` once plus one re-run of the dive; store `sonnet-explorer/target/accept/global16384` (coarse water summary written, #261); authorization opus-planner "GO: perf window 8"; run by the coordinator session, no `--machine-ready`. Quiet check 0.8 % CPU over 30 s; 00:29:50 to 00:50:31 (20 min 41 s). Gate ok on every part (no refusals); settle polls 2.6-4.0 % after one poll each.

## Verdict: `accept M5` REFUSED (no hardware verdict): `dive-lit.json` missing, the gated lit dive CRASHED

`test-render --perf --scene dive --lit` panics right after "rig built" (before any frame): `wgpu error: Validation Error: In Queue::write_buffer: Buffer with 'globals' label is invalid` (wgpu 30.0.1), exit 101, twice (inside `perf m5`, and the one re-run `perf stream dive --store ... --lit --out perf/logs/dive-lit.json`). dive-fast (same script) and the other lit streaming scenes ran. Cause open (item under diagnosis). Other automated steps: `test fast` 32.3 s over 20 s (machine busy); `gpu_modes the_terrain_perf_workload_refuses_unready_machines_and_the_smoke_run_is_labelled` slow (> 60 s).

## Lit views (session valid; GPU verdicts per view; "low" = clock under the 1,400 MHz floor)

| View | terrain <=8 | atmosphere <=1 | water <=2 | p99 <=20 | GPU clock |
|---|---|---|---|---|---|
| ground-1m | 1.110 ok | 0.214 ok | 0.088 ok | 3.419 ok | valid |
| aerial-20km | 0.783 ok | 0.229 ok | 0.160 ok | 4.152 ok | valid |
| coast-2km | 1.039 ok (low) | 0.254 ok (low) | 2.021 inconclusive | 5.510 ok | P0 1312 low |
| coast-3m | 1.328 ok (low) | 0.234 ok (low) | 2.943 inconclusive | 7.466 ok | P0 1320 low |
| orbit | 0.051 ok | 0.646 ok | 0.160 ok | 2.264 ok | valid |

Run-to-run noise of atmosphere and water at most 0.027 (0.065 ms); none above the 10 % rule or the proposed ADR 0036 point 3 floor.

## Coastal flights (report only; water pass + scene copy, ms median)

| Flight | filter 1 water | clock | filter 0 water | clock | frame p99 f1 / f0 |
|---|---|---|---|---|---|
| glide | 6.252 | P5 937 low | 3.062 | P0 1327 low | 16.535 / 7.528 |
| descent | 5.262 | P8 277 low | 2.972 | valid (over, not gated) | 31.260 / 7.719 |

Filter-on flights again ran at collapsed clocks, so the on/off ratio stays confounded. Filter-off water is up from window 6 (glide 1.79 ms at a valid clock): attribution pending.

## Streaming scenes on the lit path (frame verdicts valid; GPU verdicts not valid: each started at P5/P8, 210-525 MHz)

| Scene | median | p99 | max | upload p99 / max | detail lag | result |
|---|---|---|---|---|---|---|
| dive | - | - | - | - | - | CRASH |
| dive-fast (report only) | 8.86 | 15.50 | 17.66 | 0.68 / 4.06 | descent mean 0.633 (<=1), worst 2.78 (<=3); ground mean 0.342 | ok |
| teleport-1m | 6.12 | 8.90 | 11.08 | 0.44 / 1.64 | lag 18.04 at frame 0; under 1 level after 2.37 s (tracked, over 2 s); complete 6.40 s (<=9 s) | ok |
| teleport-2km | 4.52 | 5.82 | 11.76 | 0.44 / 1.54 | lag 11.51 at frame 0; after 1.38 s (tracked, over 1 s); complete 3.97 s (<=4 s) | ok |
| look-1m | 3.79 | 6.82 | 8.30 | 0.00 / 0.01 | rotation 0 | ok |
| look-2km | 2.02 | 3.79 | 4.54 | 0.00 / 0.00 | rotation 0 | ok |
| dive-cold (report only) | 6.61 | 15.38 | 19.48 | 0.69 / 1.87 | descent mean 5.290, worst 11.28; ground mean 0.461 | ok |

No blank or uncovered nodes. RAM 0.52-1.60 GiB (<=8); slots 231,090 of 231,090; resident max 15,719 (dive-fast). The window-7 stalls are gone: frame max 17.66 ms (dive-fast), 19.48 ms (dive-cold); STRM-024 and the coarse summary (ADR 0058) confirmed on the full 2,400-frame descent of dive-fast.

## lit-formats

Session valid for frames; compact/vertex ratios not formed: vertex views below the floor (coast-2km P0 1290, orbit P0 1327) and compact coast-2km at P2 1102 MHz.

## Open

Dive crash; water cost against window 6; clock validity of the coast views and flights; `tile_misses`, `queries` and WaterBook counts not yet read from the frames files (`perf/logs`, untracked).
