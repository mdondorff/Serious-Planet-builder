# Perf window 7 (second M5 window), 2026-10-17

Rasierklinge, main f0d68b0, `perf m5` once plus one re-run per part; store `sonnet-explorer/target/accept/global16384`; authorization opus-planner "GO: perf window 7". Logs: `sonnet-m4/perf/logs/` (untracked; json and frames not committed), console noise lines in `window-7-console.txt`, generated report `sonnet-m4/target/review/M5-report.generated.md` (not committed; the planner fills it). Run 20:32-20:50 (m5), re-runs to 21:07.

## Verdict: `accept M5` FAIL (hardware criteria; `test fast` 26.0 s over its 20 s budget with the machine busy)

Failing gated rows (dive-lit, valid session): frame max 61.19 ms at frame 95 (cap 40; record column 60.86 ms); descent detail lag mean 1.702 levels (budget 1), worst 8.17 (budget 3). Not judged (GPU clock below 1400 MHz in a valid session): coast-2km, coast-3m, orbit (terrain, atmosphere, water rows). All other gated rows pass. Software steps (check, spec-lint, test, test gpu, smokes) pass.

## Lit views (re-run of `perf lit`, session valid for frame and GPU eligibility)

| View | terrain <=8 | atmosphere <=1 | water <=2 | p99 <=20 | GPU clock |
|---|---|---|---|---|---|
| ground-1m | 1.110 PASS | 0.215 PASS | 0.088 PASS | 4.35 PASS | P0 1485 valid |
| aerial-20km | 0.785 PASS | 0.231 PASS | 0.162 PASS | 3.02 PASS | P0 1538 valid |
| coast-2km | 1.038 n/v | 0.254 n/v | 2.020 n/v | 5.35 PASS | P0 1282 invalid |
| coast-3m | 1.340 n/v | 0.235 n/v | 2.971 n/v | 7.37 PASS | P0 1335 invalid |
| orbit | 0.055 n/v | 0.741 n/v | 0.183 n/v | 2.28 PASS | P0 1342 invalid |

n/v: not valid for GPU budgets (clock under the 1400 MHz floor; warm-up capped at 30 s without reaching it). First `perf lit` (inside m5): session INVALID (aerial-20km noise 0.108 above 0.1 after its retry); its aerial row was P0 1162 MHz.

## Coastal flights (report only; water pass + scene copy, ms median; none has a valid GPU clock)

| Flight | filter 1 water | clock | filter 0 water | clock | p99 frame f1 / f0 |
|---|---|---|---|---|---|
| glide | 6.15 | P5 840 | 3.01 | P2 1290 | 16.19 / 7.76 |
| descent | 5.43 | P8 285 | 2.91 | P0 1350 | 30.74 / 7.89 |

Filter-on water is about 2x filter-off in the same run but each filter-on flight ran at a collapsed clock (P5/P8) and filter-off at ~1300 MHz, so the ratio is clock-confounded; neither value is a verdict. Filter-off was 1.79 ms in window 6 at a valid clock; here it is 2.9-3.0 at 1290-1350 MHz. Whether water stays <= 2 ms with the filter on is NOT answered.

## Streaming scenes on the lit path (frame verdicts valid; GPU not measured)

| Scene | median <=16.6 | p99 <=20 | max <=40 | upload p99 <=2 / max | detail lag | verdict |
|---|---|---|---|---|---|---|
| dive | 5.54 | 15.72 | 61.19 FAIL | 0.70 / 1.81 | descent mean 1.702 (<=1) FAIL, worst 8.17 (<=3); ground 2.75 s (tracked) | FAIL |
| dive-fast (report) | 8.05 | 15.22 | 55.78 over | 0.67 / 3.09 | descent mean 1.712, worst 7.74 | not gated |
| dive-cold (report) | 5.56 | 15.77 | 16.55 | 0.66 / 2.05 | descent mean 4.131, worst 10.96; ground 0.173 | not gated |
| teleport-1m | 5.96 | 8.05 | 9.38 PASS | 0.44 / 1.44 | lag 18.04 at frame 0; under 1 level after 2.50 s (2 s, tracked); complete 6.12 s (9 s) | PASS |
| teleport-2km | 4.13 | 5.25 | 6.52 PASS | 0.46 / 1.76 | lag 11.51 at frame 0; after 1.53 s (1 s, tracked); complete 3.60 s (4 s) | PASS |
| look-1m | 3.77 | 5.30 | 7.09 | 0.00 / 0.00 | rotation 0 | PASS |
| look-2km | 2.03 | 3.13 | 3.80 | 0.00 / 0.01 | rotation 0 | PASS |

Blank and uncovered nodes: none in any scene. RAM 1.2-4.1 GiB (<= 8), slots 231090 of 231090.

Validity: load_calib before single / parallel median (us; <=50,000 / <=100,000): dive 46,531 / 93,407; dive-fast 46,572 / 94,189; dive-cold 46,672 / 90,665; teleport-1m 46,394 / 95,122; teleport-2km 48,075 / 96,607; look-1m 49,913 / 85,042 (after values within limits too). All valid; no streaming re-run for calibration.

## Lit-formats (re-run, session INVALID both times; orbit/vertex noise 0.131 after retry)

No compact/vertex ratio could be formed (aerial and coast vertex views at 1282-1380 MHz). Water at coast-2km (invalid clock, ~1285 MHz both): vertex 1.710, compact 2.098 ms; terrain 0.888 vs 1.078. ground-1m (valid clocks, 1657 vs 1567 MHz, ratio refused): terrain 1.057 vs 1.103, water 0.082 vs 0.088.

## Watch list

- Frame 0 / early frames: dive 2.64 ms (frame 5 0.79), dive-fast 1.34 (frame 5 0.97), dive-cold 1.09 (frame 5 1.06), teleport-1m 0.42, teleport-2km 0.47. No multi-second frame 0 in any scene (window 6's 8 s frames gone). The slow item is the asynchronous plan: largest plan column 4,313 ms (dive, frame 234), 3,854 (dive-fast, 236), 2,185 (dive-cold, 176), 108.9 (teleport-1m, frame 7), 94.9 (teleport-2km, frame 6); off the frame path.
- Remaining stalls: dive frame 95 record 60.9 ms and dive-fast frames 65/83 (55.8 / 53.4 ms, record 53.0 at 83): single frames inside the descent, cause not investigated.
- WaterBook entries after the case (`water of N tiles`): dive 12,822; dive-fast 17,471; dive-cold 12,247; teleport-1m 11,929; teleport-2km 11,826; look-1m 11,550; look-2km 6,419 (resident slots at most 12,802 / 17,415 / 12,247 / 11,893 / 11,790 / 11,550 / 6,419). The book tracks about the resident-tile peak, not unbounded growth; dive-fast holds 17.5k against 231k slots. Coast flights (litperf) log no count.
- `tile_misses`: not in any log or console line of this build (only a unit test reads it); not measurable from this window.
- Tile load (18 workers, store loads in the tile job): load jobs / run median / wait median: dive 12,816 / 33.0 / 26.2 ms; dive-fast 17,438 / 31.4 / 23.9; dive-cold 12,241 / 34.5 / 28.2; teleport-1m 5,484 / 30.2 / 23.2; teleport-2km 5,380 / 30.8 / 23.6. Jobs per case second (case incl. post-pass): dive ~196, dive-fast ~269, dive-cold ~166, teleports ~215 (over 25 s incl. 12 s settle). Run median 31-35 ms is the F11 18-worker figure (27-28 ms); no sign of an oversubscription slowdown beyond it.
- Host load: dive 74 %, dive-fast 94 %, dive-cold 84 % system mean (harness 71-90 %; competing 3.3-3.4 %); lit/look views 2-4 %. Readiness gate refusals (CPU 18.4, 18.5, 15.5 % over limit 15) hit dive-fast, teleport-1m and teleport-2km inside the m5 run, 1 try each; re-run alone: dive-fast ran at try 1, teleport-1m try 1, teleport-2km after 2 more refusals (17.7, 16.5 %) at try 3. The harness's own settle polls report 8-14 %, so the gate and the settle disagree after a long child.
- A stray `cargo xtask perf` (no scene, the terrain scene) ran once at 20:29 before the window by mistake (wrote terrain logs, moved aside, not in the results).

## Parts and durations

m5: lit ~6.5 min (invalid), lit-formats ~4 min (invalid), dive 65 s case, dive-fast refused, teleports refused, look-1m 34 s, look-2km 26 s, dive-cold 74 s; total 18 min. Re-runs: dive-fast, teleport-1m, teleport-2km (65 / 26 / 25 s cases), lit (once), lit-formats (once). `accept M5` ~5 min.
