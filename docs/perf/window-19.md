# Perf window 19 (edit latency: brush, regional, line, region; preview cost), 2026-10-20

Rasierklinge, main ab23639 (lattice normals ADR 0073 merged; STRM-031 settle 25 ms), release build, RTX 3080 Ti Laptop, 2560x1440, cells 32, tau 6 px. Coordinator opus-planner, run by sonnet-explorer. Store `target/accept/global16384` (wet-floor blocks; compact tiles generated on demand, no pre-fill).

## Verdict

- Preflight OK (single 46.3 to 46.7 ms, parallel median 85.8 to 99.3 ms at 18 threads); the readiness gate was ok; no `--machine-ready`.
- `edit-latency` at the editor default settle (25 ms), 21 cases, 3 repeats, medians. Pre-emption counters are 0 in every case.
- The old over-budget case (200 m altitude, 150 m stamp) is within budget: drawn exact 125.3 ms against 300 ms (window 18: 544 ms at settle 40).
- All 9 stamp cases of window 18 are within budget; drawn exact falls from 508 to 544 ms to 125 ms at 200 m / 150 m and from 953 to 996 ms to 239 ms at 1 m / 50 m.
- Preview 2.2 to 7.1 ms in every previewed case (budget 16 ms).
- Brush: 150 m at 200 m altitude draws exact in 383.8 ms against the 300 ms budget (over); 1 km at 200 m is 651.8 ms against 1,000 ms (ok). The harness prints `no result` for the four gated brush rows because a brush has no preview node and `verdict()` requires a preview median (`crates/app/src/editperf.rs`, `verdict`); the drawn-exact values are valid.
- Regional stamps, ridge (line) and region cases are recorded, not gated: 65.7 to 249.8 ms drawn exact.

## Drawn exact against budget, medians of 3

| Camera | Case | Preview ms | Drawn exact ms | Budget | Verdict |
|---|---|---|---|---|---|
| 200 m | stamp 150 m (old failing case) | 3.5 | 125.3 | 300 | ok |
| 1 m | stamp 50 m | 6.1 | 239.2 | 1,000 | ok |
| 1 m | stamp 150 m | 7.1 | 222.4 | 1,000 | ok |
| 1 m | stamp 500 m | 6.4 | 230.4 | 1,000 | ok |
| 200 m | stamp 50 m | 3.2 | 60.9 | 300 | ok |
| 200 m | stamp 500 m | 5.4 | 210.4 | 1,000 | ok |
| 2 km | stamp 50 m | 2.7 | 47.6 | 300 | ok |
| 2 km | stamp 150 m | 2.6 | 47.6 | 300 | ok |
| 2 km | stamp 500 m | 2.4 | 56.9 | 1,000 | ok |
| 200 m | brush 150 m | none | 383.8 | 300 | over (printed `no result`) |
| 200 m | brush 1 km | none | 651.8 | 1,000 | ok (printed `no result`) |
| 2 km | brush 150 m | none | 77.7 | 300 | ok (printed `no result`) |
| 2 km | brush 1 km | none | 317.8 | 1,000 | ok (printed `no result`) |

## Regional, line and region (recorded, no budget yet)

| Camera | Case | Preview ms | Drawn exact ms | All exact ms |
|---|---|---|---|---|
| 2 km | stamp 5 km | 4.8 | 209.6 | 450.9 |
| 2 km | stamp 40 km | 4.0 | 249.8 | 463.7 |
| 20 km | stamp 5 km | 2.2 | 65.7 | 308.0 |
| 20 km | stamp 40 km | 3.6 | 222.2 | 434.3 |
| 200 m | ridge 5 km | none | 237.4 | 539.5 |
| 2 km | ridge 5 km | none | 238.9 | 469.2 |
| 200 m | region 2 km | 5.6 | 223.2 | 472.0 |
| 2 km | region 2 km | 4.5 | 173.8 | 424.9 |

## Preview cost with 0, 8 and 32 active stamps (all flagged nodes)

| Camera | 0 stamps GPU / wall ms | 8 stamps | 32 stamps |
|---|---|---|---|
| 1 m | 1.35 / 2.67 | 1.56 / 3.27 | 2.41 / 4.07 |
| 200 m | 1.15 / 2.18 | 1.22 / 3.25 | 1.53 / 2.98 |
| 2 km | 0.83 / 1.68 | 0.87 / 2.41 | 1.28 / 2.38 |
| 20 km | 1.01 / 1.82 | 1.03 / 1.84 | 1.14 / 1.78 |

## Notes

- The harness could not write `edit-latency.md` (the `--out` directory did not exist); the complete stdout is `docs/perf/window-19/edit-latency.txt`.
- The brush rows finish the half-refresh at 510 ms (150 m) and 1,107 ms (1 km) at 200 m altitude: 1,816 and 3,857 tiles refresh, against 291 for a 150 m stamp. The brush stroke is the one case over its budget.
