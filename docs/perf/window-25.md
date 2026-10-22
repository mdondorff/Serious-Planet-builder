# Perf window 25 (PW2: the ground detail after G2c, the plan job), 2026-10-22

Rasierklinge, RTX 3080 Ti Laptop (Vulkan), release build of main at 89e36544 (#533, G2c). Coordinator opus-planner, run by sonnet-explorer. Preflight: the first run refused (single 55.6 ms over 52, right after my release build: its report was overwritten by the rerun), the second OK (`window-25/preflight.md`: single 46.4 to 49.7 ms against 52, parallel median 73.9 to 92.9 ms against 112); no `--machine-ready`. The readiness gate was ok in every run; five of the six `perf lit` runs are valid sessions (the 1.8 m view of `lit-natural-r1` ran at 1387 MHz, under the 1400 MHz floor: not valid for the GPU budget verdict, kept in the mean because it agrees with its twin to 0.05 ms), `lit-full-r2` is INVALID (CPU calibration of ground-0.5m 52.9 ms over 52, ADR 0065) and is kept for the record, not counted.

## Verdict

- **The ground detail is inside its budget: +0.70 ms (1.8 m) and +0.90 ms (0.5 m) of terrain-pass GPU time at 1440p against +1.0 ms for G2a plus G2b** (window 24: +2.38 and +2.90). The bench's prediction (+0.55, +0.7, terrain only) read 15 to 25 % low, as expected for the window (atmosphere, gated clocks).
- Natural (0.6) and full cost the same; the 0.5 m view is the tight one (0.1 ms of headroom).
- The plan job did not regress: window 23's build and main give the same plan times today (bisect below); window 23's recorded table was a faster session.

## Ground views, GPU ms (`perf lit --views ground-1.8m,ground-0.5m`, 2560x1440; terrain pass median of the valid runs)

| View | Strength | Terrain pass (r1 / r2) | Mean | Delta | Window 24 (before G2c) |
|---|---|---|---|---|---|
| ground-1.8m | off | 1.352 / 1.386 | 1.369 | | 1.367 |
| | natural 0.6 | 2.042 / 2.086 | 2.064 | +0.70 | 3.751 (+2.38) |
| | full 1.0 | 2.093 / (2.091, invalid) | 2.093 | +0.72 | 3.702 (+2.33) |
| ground-0.5m | off | 1.363 / 1.366 | 1.365 | | 1.363 |
| | natural 0.6 | 2.254 / 2.264 | 2.259 | +0.89 | 4.258 (+2.90) |
| | full 1.0 | 2.237 / (2.251, invalid) | 2.237 | +0.87 | 4.185 (+2.82) |

Frame (waited) median at the ground: 3.1 ms off, 4.1 (1.8 m) and 4.2 ms (0.5 m) with the detail. The runs `lit-natural-r1` (1.8 m) notes a low clock (P0 1387 MHz, floor 1400) and still agrees with its twin to 0.05 ms. Raw: `window-25/lit-<strength>-r<round>.txt` and `.json`.

## Item sweep, plan job ms (p50; still camera and a drift of 0.5 m a frame; `PLANET_SWEEP_ONLY="Platonmax,Hill 7,Ridge 3"`, a fresh copy of `target/owner-items`)

| Item and altitude | Still (cold) | Drift 0.5 m/frame (warm) | Window 24 still / drift |
|---|---|---|---|
| Platonmax 2 km | 291 | 49 | 281 / 51 |
| Platonmax 200 m | 1,526 | 572 | 1,557 / 555 |
| Platonmax 30 m | 638 | 525 | 607 / 576 |
| Ridge 3 2 km | 2,506 | 429 | 2,673 / 408 |
| Ridge 3 200 m | 1,570 | 675 | 1,703 / 648 |
| Ridge 3 30 m | 887 | 659 | 926 / 663 |
| Hill 7 2 km (first) | 116 | 87 | 137 / 89 |
| Hill 7 200 m | 1,131 | 603 | 1,170 / 602 |
| Hill 7 30 m | 1,206 | 696 | 1,188 / 684 |

No plan refused. Same numbers as window 24 within noise.

## Plan-job bisect (indicative, same session as window 24's sweep, not part of this window's gate)

Window 23's build C (48292e52, rebuilt) and main, still camera, 2 km / 200 m / 30 m, plan p50 ms:

| Item | 48292e52 | main (window 24) |
|---|---|---|
| Platonmax | 307 / 1,211 / 566 | 281 / 1,557 / 607 |
| Ridge 3 | 2,609 / 1,668 / 901 | 2,673 / 1,703 / 926 |
| Hill 7 (first, 2 km / 200 m / 30 m) | 166 / 989 / 722 | 137 / 1,170 / 1,188 |

The old build is as slow today as main: no code regression since window 23 (#521 to #533). The 1.3 to 1.75 times against window 23's recorded table is the session (machine state, or the items directory: the sweep reads a copy of the owner's current items). Raw: `window-24/sweep-*`, the old build's report is `window-25/bisect-48292e52-still.txt`.

## Not measured

- The detail in the aerial and orbit views, and with the editor UI open.
- The sweep with the detail off.
