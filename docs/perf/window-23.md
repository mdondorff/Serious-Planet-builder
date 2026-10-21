# Perf window 23 (the plan job after the nested-bounds proof, #513), 2026-10-21

Rasierklinge, RTX 3080 Ti Laptop, release builds. Coordinator opus-planner, run by sonnet-explorer. Builds: A = f2382bd8, B = 772081f0 (both as in window 22), C = main at 48292e52 (#505 capping, #513 the proof of LOD-012). One round; order sweeps C, A, B, then `dive` C, A, B. Method as in window 22 (`window-22.md`). Preflight OK on the first run (`window-23/preflight.md`: single 46.2 to 50.1 ms against 52 ms, parallel median 91.0 to 99.0 ms against 112 ms). Raw output: `window-23/`.

## Verdict

- **`dive`: plan job median 1.38 ms against 1.22 ms (+13 %)**, down from +125 % (2.73 ms) in window 22. Just over the +10 % bound; the cost is the proof itself (`none_steep`, about 0.16 ms per plan job).
- **Editor chain, 2 km item stops: three of four rows at or below A**; Ridge 3 is 3x.
- No wrapper of the editor chain blocks the proof (the chain `build_field`, stamp stages, `WaterBounded`, `CancelGuarded` forwards `bounds_monotone`; the eroded base says yes). Ridge 3 fails the proof because tiles near the ridge are really too steep to morph (the tiles ADR 0074 caps); the full path then runs, with the volumes (stamp-stage bounds) of the whole position-only tree computed cold.
- Tracked: `docs/status.md` (Ridge 3: the first plan at a new place is cold-costly). The probe after the window (a test of the selection, not kept; a 12 km steep ridge item, the stamp stage over the reference field, a camera 2 km up, a fresh `SelectionCache`) gave 211 ms cold against 8 ms over flat ground and 1.3 ms warm (flat 0.9 ms); at 200 m 48 ms cold. The cold time is the stamp stage's height-bounds queries, about 60 µs each: 85 ms for the position-only walk's tiles and 125 ms for judging them (the volumes of their neighbours). The sweep holds the camera still, so each of its plans is a cold one.

## `dive` (2400 frames), plan job ms (median / p99 / max)

| Build | Window 22 (mean of 2 rounds) | Window 23 |
|---|---|---|
| A f2382bd8 | 1.22 / 28.8 / 3198 | 1.22 / 47.9 / 2464 |
| B 772081f0 | 1.21 / 45.9 / 2537 | 1.22 / 45.0 / 2487 |
| C main | 2.75 / 51.8 / 2502 (#505) | 1.38 / 46.8 / 2478 (#505 + #513) |

## Sweep, 2 km stops (every build delivers a plan), plan p50 ms

| Item | A | B | C |
|---|---|---|---|
| Platonmax | 233 | 166 | 161 |
| Hill 7 (first) | 128 | 133 | 95 |
| Hill 7 (second) | 610 | 609 | 501 |
| Ridge 3 | 672 | 704 | 1998 |

Ridge 3 in window 22 (C with #505): 1568 and 1517. A and B refuse the plan at 6 of 12 stops (every 200 m and 30 m stop of Platonmax and Ridge 3, and the first Hill 7); C refuses none. At those stops only C has plan samples: 264 to 1511 ms at 200 m and 30 m. The frame p50 and p95 sit at the 250 ms cap during arrival and are not used to rank builds.

## Selection time without the editor (`plan_ablation`, warm, ms, on the eroded 16384 bake)

| View | #505 | #513 |
|---|---|---|
| ground-1m | 10.6 | 1.8 |
| ground-200m | 5.5 | 1.1 |
| low-2km | 3.8 | 0.9 |
| aerial-20km | 2.5 | 0.5 |
| cube-corner-1km | 7.5 | 5.4 (proof fails; full path) |

## Not measured

- Frame time after the fix (the sweep frames are cap-bound).
- aerial-20km and ground-1m as separate stream scenes.
