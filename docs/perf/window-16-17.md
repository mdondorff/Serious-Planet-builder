# Perf windows 16 and 17 (edit latency; the cover quadrants A/B), 2026-10-20

Rasierklinge, release builds, coordinator opus-planner, runs by sonnet-explorer. Raw outputs: `target/perf-w16/`, `target/perf-w16-B/`, `target/perf17/` of the `sonnet-m4` worktree (not archived).

## Verdict

- Window 16 (main with the edit-find work, before #422): the preview passes in all nine cases (2.8 to 10.7 ms against 16 ms); "drawn exact" is within budget in 8 of 9 cases, "all exact" in 1 of 9 (table below).
- Window 17 (main df1562e against d56f973, the commit before #422): the cover quadrants with the gated cut pipelines cost nothing measurable on the RTX; descent detail lag improved (mean 0.149 against 0.285 / 0.314 levels).
- Every gated measure of both windows is within budget except the exact-edit criterion, which is an owner decision (`docs/status.md`).

## Window 16: edit latency

Preflight OK. Edit latency (`perf stream edit-latency`), main after edit-find, new columns "find call" and "drawn exact" (first frame whose drawn tiles are all exact):

| Camera | Stamp | Drawn exact | All exact | Budget | Drawn exact within budget |
|---|---|---|---|---|---|
| 1 m | 50 / 150 / 500 m | 890 / 948 / 956 ms | 1,144 / 1,286 / 1,290 ms | 1,000 | yes |
| 200 m | 50 m | 188 ms | 509 ms | 300 | yes |
| 200 m | 150 m | 525 ms | 824 ms | 300 | no |
| 200 m | 500 m | 886 ms | 1,210 ms | 1,000 | yes |
| 2 km | 50 / 150 / 500 m | 93 / 93 / 159 ms | 398 / 371 / 440 ms | 300 / 300 / 1,000 | yes |

- Under "drawn exact" one case fails (200 m, 150 m stamp); under "all exact" eight of nine fail against the budget column.
- Exact time is refresh throughput; the refreshed set by level (tiles named, tiles drawn, tiles whose sample spacing exceeds the stamp's change) is in the case output (`levels_table`). Redefining "exact" or skipping refreshes below the spacing is the owner's decision.
- Stream scenes of the same window (main before #422, `target/perf-w16-B/stream-all.txt`), wall median / p99 / max ms: dive 6.02 / 16.89 / 27.87, dive-cold 6.03 / 16.06 / 17.07, teleport-1m 6.91 / 8.88 / 10.88, teleport-2km 4.35 / 5.84 / 7.34. All within budget.

## Window 17: the cover quadrants A/B and the gated re-measure

Old = d56f973 (main before #422); new = df1562e (#422 cover quadrants with `CUT_QUADRANTS` pipelines only for masked nodes, line-stage, region-items). Preflight OK (single 46.4 to 47.5 ms against 52; parallel median 84 to 105 ms against 112), no `--machine-ready`. Order: old dive, new dive, old teleport-1m, new teleport-1m, old dive again (drift). Lit, compact tiles, 2560x1440, store `global16384`.

| Scene | Build | Wall median / p99 / max ms | Record median / p99 / max ms | Descent lag mean / worst (levels) |
|---|---|---|---|---|
| dive | old | 5.46 / 13.90 / 15.56 | 1.65 / 4.68 / 6.22 | 0.285 / 2.05 |
| dive | new | 5.86 / 16.16 / 21.39 | 1.66 / 5.08 / 7.58 | 0.149 / 1.49 |
| dive | old, repeat | 5.62 / 15.82 / 17.87 | 1.63 / 5.21 / 16.68 | 0.314 / 1.49 |
| teleport-1m | old | 6.20 / 9.15 / 10.59 | 3.65 / 5.58 / 6.37 | not applicable |
| teleport-1m | new | 6.53 / 8.43 / 11.19 | 3.78 / 5.03 / 6.56 | not applicable |

- The old build drifted by 0.16 ms (median) and 1.9 ms (p99) between its two dive runs; the new-against-old differences (+0.4 ms median, +2.3 ms p99 against the first old run) are of that size.
- The dive's new max (21.4 ms) is a single spike, under the 40 ms cap.
- teleport-1m: +0.33 ms median, p99 lower; the CPU clock differed between the runs (2,614 against 3,024 MHz).
- Conclusion: no measurable cost of the gated cut pipelines on the RTX. The earlier A/B with the discard in every pipeline cost 5 to 8 % of the frame in the heavy scenes (`docs/learnings.md`, SPIKE-18); that run is not archived.
- Not measured: the hoisting of the per-sample region test (GEN-029), which needs its own window.
