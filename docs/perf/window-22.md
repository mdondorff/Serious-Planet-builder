# Perf window 22 (the plan job under ADR 0074), 2026-10-21

Rasierklinge, RTX 3080 Ti Laptop, release builds. Coordinator opus-planner, run by sonnet-explorer. Three builds: A = f2382bd8 (+ #495 and the sweep patches), B = 772081f0 (+ the sweep columns), C = main at 3169473a (#505 capping, #506 sweep columns). Two rounds, order A, B, C per scene.

- Sweeps: `PLANET_SWEEP_ONLY="Platonmax,Hill 7,Ridge 3"`, altitudes 2000, 200, 30 m, 90 frames per stop, in the editor window on the owner's items copy (frame cap 250 ms).
- Stream scenes: `test-render --perf --scene dive` and `teleport-1m`, hardware adapter, 16384-cell store.
- Preflight: the first was refused (CPU calibration round 3 single 53,172 µs against 52,000); the second passed (`window-22/preflight1.md`, `preflight2.md`).
- Raw output: `window-22/`.

## Verdict

- **The plan job of C is over the +10 % bound** (ADR 0074): dive plan median 2.73 to 2.77 ms against 1.20 to 1.22 ms (+125 %), both rounds.
- No regression between A and B: the dive plan median is equal (1.21, 1.22 and 1.20, 1.22 ms); the sweep frame times agree within noise.
- Platonmax is capped by #505: sweep C refused no plan in either round; A and B refuse at 6 of 12 stops in both rounds.
- Cause: the plan job walks the position-only selection on every real field (`steep_possible` is true at h of about 4000 m); R adds no volumes afterwards.

## `dive` (2400 frames), plan job ms (median / p99 / max)

| Build | Round 1 | Round 2 |
|---|---|---|
| A f2382bd8 | 1.21 / 14.0 / 3824 | 1.22 / 43.7 / 2572 |
| B 772081f0 | 1.20 / 44.1 / 2552 | 1.22 / 47.7 / 2522 |
| C main + #505 | 2.77 / 51.5 / 2501 | 2.73 / 52.2 / 2504 |

Frame median / p99 are equal in every build (4.6 to 5.0 / 8.0 to 9.3 ms). The max is the cold start. `teleport-1m`: plan median 0.00 (too few plans), frame median 2.9 to 3.1 ms in every build.

## Sweep, 2 km stops (every build delivers a plan), round 2, plan p50 ms

| Item | A | B | C |
|---|---|---|---|
| Platonmax | 228 | 175 | 485 |
| Ridge 3 | 722 | 696 | 1517 |
| Hill 7 (first) | 103 | 95 | 401 |
| Hill 7 (second) | 467 | 655 | 1388 |

Round 1 agrees (C 510, 1568, 431, 969). R's job is the same size in all builds. The frame p50 and p95 of the sweep sit at the 250 ms cap during arrival and are not used to rank builds. At the 200 m and 30 m stops A and B refuse the plan, so only C has plan samples there (C: 255 to 1275 ms).

## Not measured

- aerial-20km and ground-1m as separate stream scenes (the dive covers the descent).
- Frame time after the cost fix (to follow in the re-measure).
