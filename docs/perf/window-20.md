# Perf window 20 (edit latency after the brush footprint), 2026-10-20

Rasierklinge, main 80e1418 (#474: the footprint and the refresh find of a brush follow its strokes), release build, RTX 3080 Ti Laptop, 2560x1440, cells 32, tau 6 px. Coordinator opus-planner, run by sonnet-explorer. Store `target/accept/global16384`.

## Verdict

- Preflight OK on the first run (single 47.0 to 50.7 ms, limit 52 ms; parallel median 83.7 to 92.5 ms at 18 threads, limit 112 ms); the readiness gate was ok; no `--machine-ready`.
- `edit-latency` at the editor default settle (25 ms), all 21 cases, 3 repeats, medians. Pre-emption counters are 0 in every case.
- The 150 m brush at 200 m altitude is within budget: drawn exact 286.1 ms against 300 ms (window 19: 383.8 ms); tiles refreshed 601 (window 19: 1,816).
- The 1 km brush at 200 m: 402.8 ms against 1,000 ms (651.8 ms); tiles refreshed 930 (3,857).
- Every gated row has a verdict (`ok`); none reads `no result`. The brush rows have a preview (8.5 and 10.3 ms at 200 m altitude, against 16 ms).
- The control, the 150 m stamp at 200 m, is unchanged: 127.8 ms (window 19: 125.3 ms), 291 tiles.

## Brush rows against window 19

| Camera | Brush | Preview ms | Drawn exact ms (w19) | Budget | Tiles refreshed (w19) | Verdict |
|---|---|---|---|---|---|---|
| 200 m | 150 m | 8.5 | 286.1 (383.8) | 300 | 601 (1,816) | ok |
| 200 m | 1 km | 10.3 | 402.8 (651.8) | 1,000 | 930 (3,857) | ok |
| 2 km | 150 m | 3.5 | 58.7 (77.7) | 300 | 81 (143) | ok |
| 2 km | 1 km | 9.8 | 195.9 (317.8) | 1,000 | 371 (1,291) | ok |

## Control and other cases (medians, ms)

| Camera | Case | Drawn exact (w19) | Tiles refreshed |
|---|---|---|---|
| 200 m | stamp 150 m | 127.8 (125.3) | 291 |
| 200 m | stamp 50 m | 60.6 (60.9) | 115 |
| 1 m | stamp 150 m | 239.2 (222.4) | 519 |
| 200 m | ridge 5 km | 259.0 (237.4) | 485 |
| 200 m | region 2 km | 230.8 (223.2) | 520 |
| 2 km | stamp 5 km / 40 km | 209.6 (209.6) / 236.6 (249.8) | 452 / 487 |
| 20 km | stamp 5 km | 64.2 (65.7) | 97 |

## Notes

- The brush still refreshes about twice the tiles of a stamp of the same radius at 200 m (601 against 291): the stroke is 712 m long, so its capsule is larger than a 150 m disc.
- The 150 m brush has 14 ms of headroom against its budget (286.1 against 300); the 1 km brush has 600 ms.
- Raw output: `docs/perf/window-20/edit-latency.txt`; the harness wrote `edit-latency.md` into the new `--out` directory (target only); `docs/perf/window-20/preflight.md`.
