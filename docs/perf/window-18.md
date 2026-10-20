# Perf window 18 (edit latency after the pre-emption change), 2026-10-20

Rasierklinge, main fa8e3f8 (#450: STRM-031, TEST-041 modified, ADR 0072), release build. Coordinator opus-planner, run by sonnet-explorer. The 16384-cell bake was regenerated for the window (`generate --global-dir target/accept/global16384 --global-res 16384 --prefill-levels 0`: bake 362 s on 20 threads, ocean channel 10.2 s beyond the bake, summary 9.9 s). Raw: `target/perf18/` of the `sonnet-m4` worktree (not archived).

## Verdict

- Preflight OK; the readiness gate was ok in both runs; no `--machine-ready`.
- `edit-latency` at `--settle-ms 0` and `--settle-ms 40` (the editor's setting), 3 repeats per case, medians. The verdict reads the first frame whose draw plan is exact ("drawn exact", ADR 0072).
- 8 of 9 cases are within budget at both settings. The one case over is 200 m altitude with a 150 m stamp: 508 ms at settle 0, 544 ms at settle 40, against 300 ms.
- The settle window costs about its own length (the first job is dispatched 42 to 48 ms after the edit instead of 2 to 5 ms); drawn exact grows by 21 to 64 ms. It is not the cause of the failing case.
- The pre-emption counters (coalesced, cancelled, dropped) are 0 in every case: one edit per run. The pre-emption is proven by the tier A tests (20 quick edits start at most 2 jobs).

## Machine (preflight, `target/perf18/preflight/preflight.md`)

| | |
|---|---|
| AC power, plan | connected, High performance |
| top competing processes | System 0.6 %, svchost 0.3 %, MsMpEng 0.3 %, claude 0.2 % |
| calibration single / parallel median (us) | 47,384 / 97,671; 46,247 / 91,430; 46,414 / 94,714 (limits 52,000 / 112,000) |

## Drawn exact, preview and verdict

| Camera | Stamp | Preview ms | Drawn exact, settle 0 | Drawn exact, settle 40 | Difference | Budget | Verdict |
|---|---|---|---|---|---|---|---|
| 1 m | 50 m | 6.5 | 953 | 996 | +43 | 1,000 | ok |
| 1 m | 150 m | 6.7 | 913 | 977 | +64 | 1,000 | ok |
| 1 m | 500 m | 6.2 to 6.7 | 919 | 955 | +36 | 1,000 | ok |
| 200 m | 50 m | 3.8 to 5.0 | 178 | 213 | +34 | 300 | ok |
| 200 m | 150 m | 3.8 to 3.9 | 508 | 544 | +36 | 300 | OVER |
| 200 m | 500 m | 5.4 to 5.9 | 848 | 869 | +21 | 1,000 | ok |
| 2 km | 50 m | 2.8 to 3.1 | 83 | 135 | +52 | 300 | ok |
| 2 km | 150 m | 3.0 | 96 | 137 | +41 | 300 | ok |
| 2 km | 500 m | 2.7 to 3.1 | 144 | 183 | +39 | 1,000 | ok |

## All exact (recorded, not gated), settle 0

| Camera | 50 m | 150 m | 500 m |
|---|---|---|---|
| 1 m | 1,260 ms | 1,219 ms | 1,238 ms |
| 200 m | 481 ms | 834 ms | 1,158 ms |
| 2 km | 394 ms | 422 ms | 447 ms |

At settle 40 each is within about 50 ms of these.

## Raw files

- `docs/perf/window-18/edit-latency-settle0.txt` and `edit-latency-settle40.txt`: the complete output of the two runs (budget table, stages table with the settle and the three counters, refreshed set by level for the first run of each case); `docs/perf/window-18/preflight.md`.
- The refreshed set by level of each case (tiles named, how many the plan drew, spacing against the stamp's change) is the third table of each file: the 200 m / 150 m case names 291 tiles, 151 of them drawn.
- Window 16's edit-latency table (drawn exact against all exact, no per-case stages) is in `docs/perf/window-16-17.md`; its raw output was not archived.
- Decision after the window (opus-planner, 2026-10-20): 4 ms of margin at 1 m with a 50 m stamp is too thin, so the editor's settle default becomes 25 ms (above one 60 Hz drag interval, so a drag still coalesces); the 200 m / 150 m overrun goes to sonnet-spikes as a diagnosis of the refresh throughput.

## Findings

- With the settle on, `ManagedLoop::edit` returns the number of refresh requests queued now (0: they are held), so the "tiles refreshed / resident" column and the half and all refreshed instants read 0 and `-` in the settle-40 table. The verdict and the drawn exact figures do not depend on it. A follow-up makes the return count the held refreshes.
- `cargo xtask perf stream` whitelists its options and has no `--settle-ms`; the runs called the harness directly (`cargo run --release -p planet -- test-render --perf --scene edit-latency --adapter hardware --store DIR --global-res 16384 --settle-ms N --out DIR`), as window 16 did. A follow-up adds the option.
- 200 m with a 150 m stamp stays 208 to 244 ms over its budget with or without the settle: the cost is the refresh throughput of the drawn tiles (291 named, 151 drawn), not the hold. An open owner item (`docs/status.md`).
