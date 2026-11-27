# Perf window 9 (fourth M5 window), 2026-10-18

Rasierklinge, main 501ad15 (includes the water horizon + height cut #273, REND-029 diagnostics #266, the clock work #257/#258/#271); `perf lit` (run twice: the allowed re-run) and `perf lit-formats` (once), no streaming scenes (window 8 and its dive re-run keep theirs); store `sonnet-explorer/target/accept/global16384`; authorization opus-planner "GO: perf window 9"; run by the coordinator session, no `--machine-ready`. Quiet check: 3.1 CPU-s in 30 s of 600 after killing a stray `find /` (one core since 02:37). 04:13:00-04:35:11. Gate ok on every part.

## Verdict: `accept M5` REFUSED (no hardware verdict)

`lit` session INVALID for frame verdicts both times: run-to-run noise on `lit/orbit` above 0.1 after one retry (0.178 first run, 0.149 re-run); `lit-formats` INVALID too (`orbit/vertex` 0.131, `orbit/compact` 0.179). The noisy quantity is the orbit view's tiny passes (terrain 0.051 ms; frame about 2 ms), the case ADR 0036 point 3 (proposed) addresses; the other views' atmosphere and water noise is 0.006-0.045 (0.001-0.065 ms). Automated steps: check FAIL only because of an untracked scratch example file (`cargo fmt`; removed afterwards); spec-lint, test, test gpu, smokes pass; `test fast` 3.6 s of tests (25.5 s wall).

## Lit views (second run; the numbers do not count while the session is invalid)

| View | terrain <=8 | atmosphere <=1 | water <=2 | p99 <=20 | clock | water nodes |
|---|---|---|---|---|---|---|
| ground-1m | 1.136 | 0.219 | not in view | 3.479 | P0 1477 held | 0 of 2,417 |
| aerial-20km | 0.799 (low) | 0.243 (low) | 0.116 (low) | 2.923 | P0 1365 | 56 of 913 |
| coast-2km | 1.060 (low) | 0.259 (low) | 1.871 ok at low clock | 5.322 | P0 1245 | 1,082 of 1,410 |
| coast-3m | 1.357 (low) | 0.238 (low) | 2.173 inconclusive | 6.554 | P0 1290 | 1,181 of 2,588 |
| orbit | 0.051 | 0.654 | 0.160 | 2.115 | P0 1537 held | 48 of 52 |

"low" = below the 1,400 MHz floor but within budget (ADR 0057: a figure within budget below the floor is a pass). Window 8 had coast-2km water 2.021 and coast-3m 2.943 ms (before the cut and before the cool start): now 1.871 and 2.173 ms at lower clocks.

## Coastal flights (report only; water pass + scene copy, ms median)

| Flight | filter 1 | clock | filter 0 | clock | water nodes (max) | frame p99 f1 / f0 |
|---|---|---|---|---|---|---|
| glide | 5.505 | P5 712 | 2.281 | P2 1327 | 1,372 of 2,597 | 16.138 / 7.182 |
| descent | 5.898 | P8 247 | 2.480 | P0 1365 | 1,495 of 2,598 | 31.032 / 7.040 |

Filter-off was 3.062 / 2.972 ms in window 8: the cut recovers about 0.5-0.8 ms. Filter-on again ran at collapsed clocks (P5, P8): the shore-filter cost stays unseparated.

## Cool start and held clock (ADR 0057)

Cool-down worked in every view (target 60 C; 55-70 C start, 0-35 s). Held warm-up reached the floor on ground-1m (P0 1477 MHz), aerial-20km (P0 1410), orbit (P0 1537) in about 3 s, and did NOT on the coast views and flights in 30 s (P0 1245-1365 MHz, P2 1327, P5 712 on glide filter-on, P8 247 on descent filter-on): at P0 (the top state) the clock sits at 1.25-1.37 GHz, which points to the power cap, not to heat or idling.

## lit-formats

Invalid (orbit noise). Compact/vertex ratios not formed: ground-1m clock states differ (P0 1620 against 1522 MHz), aerial-20km compact below the floor (P0 1357), coast-2km vertex below the floor (P0 1290), orbit clock states differ (P0 1522 against 1608). Per-pass: ground-1m terrain 1.067 vertex / 1.102 compact; aerial-20km 0.608 / 0.801 (low); coast-2km 0.883 / 1.069 and water 1.581 / 1.888 (both low); orbit 0.051 / 0.051.

## Open

Orbit noise rule on tiny passes (ADR 0036 point 3); clock floor unreachable on the coast views (P0 power cap); flights' filter-on clocks; streaming-scene GPU rows (window 8) still not valid.
