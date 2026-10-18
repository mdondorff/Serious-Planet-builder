# Perf window 10 (fifth M5 window), 2026-10-18

Rasierklinge, main a4479cb (ADR 0060 doc; includes the exact dry-tile cut #286; #288, tests only, merged during the window and is not in the measured tree); `perf lit` and `perf lit-formats` once each over the 16384 store, no `--machine-ready`. The streaming rows in the accept report are window 9's logs (not re-run).

## Verdict

Both sessions VALID (frame and GPU verdicts), 0 gated measures over budget. `accept M5` hardware criteria INCOMPLETE: one row, lit/coast-3m water (2.170 ms against 2 ms at P0 1350 MHz, below the floor). Under ADR 0060 (P0 power-capped clock valid, coast-3m budget 2.2 ms; the code is #291, not merged) that row passes by hand: 2.170 <= 2.2. The other steps of `accept` pass: fmt, clippy, layering, spec-lint, test (1115), test gpu (software), run modes.

## Lit views (water = pass + scene copy, ms median)

| View | terrain <=8 | atmosphere <=1 | water <=2 | p99 <=20 | clock | water nodes |
|---|---|---|---|---|---|---|
| ground-1m | 1.083 | 0.209 | not in view | 3.5 | P0 1568 | 0 of 2,417 |
| aerial-20km | 0.790 | 0.236 | 0.112 | 2.9 | P0 1350 | 56 of 913 |
| coast-2km | 1.060 | 0.258 | 1.869 | 5.7 | P0 1297 | 1,082 of 1,410 |
| coast-3m | 1.347 | 0.235 | 2.160-2.170 | 6.6 | P0 1350 | 1,181 of 2,588 |
| orbit | 0.052 | 0.679 | 0.168 | 2.9 | P0 1481 | 48 of 52 |

Coast rows by today's accept: coast-2km ok at a low clock, coast-3m NOT VALID (over 2 ms below the floor). By hand under ADR 0060: both pass (1.87 <= 2; 2.17 <= 2.2).

## Water nodes did not drop

The exact dry-tile cut (#286) is not visible in the lit scene: node counts equal window 9 (coast-2km 1,082; coast-3m 1,181) instead of the 844 / 840 of the CPU scratch run, and coast-3m water is 2.17 ms against 2.173 in window 9. The scene's plans appear to use the loose verdict: the book minimum is filled by the tile job, and the lit scene's meshes are apparently not filled that way (or not before the plan). Open.

## Coastal flights (report only)

| Flight | water f1 | clock | water f0 | clock | frame p99 f1 / f0 |
|---|---|---|---|---|---|
| glide | 4.650 | P5 802 | 2.302 | P0 1327 | 15.854 / 6.683 |
| descent | 4.260 | P8 262 | 2.450 | P0 1342 | 30.737 / 6.923 |

Filter-on again ran at collapsed clocks (P5, P8 262 MHz). Filter-off is unchanged from window 9 (2.281 / 2.480).

## lit-formats

Valid. Compact / vertex ratios: ground-1m terrain 0.991, atmosphere 1.014, span 0.994; orbit terrain 1.000, atmosphere 0.972, water 0.969, span 0.971. aerial-20km and coast-2km not compared (below the floor). Regression lines against the last valid log: orbit cpu_ms 0.21 -> 0.32 ms (compact) and 0.20 -> 0.31 ms (vertex), frame_ms 1.53 -> 1.84 ms (P0/P2 1406 against 1597 MHz). CPU side, under 0.4 ms, clock-confounded, not gated.

## Open

Exact cut not reaching the lit scene; flight filter-on clock collapse; orbit cpu_ms +50 % (0.2 -> 0.3 ms).
