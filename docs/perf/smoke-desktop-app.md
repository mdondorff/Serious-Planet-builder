# Perf smoke: coder session in the Claude desktop app, 2026-10-18

Question (owner, via opus-planner): does a coder session in the Claude desktop app measure like the VS Code panel did (window 11), with no repeat of the planner terminal's 1.4x per-thread slowdown?

Rasierklinge, code a32dafb (window 11's commit; `target/release` rebuilt from it, `sonnet-m4` worktree), session `sonnet-m4` running in the Claude desktop app. 16384 store of window 11. No `--machine-ready`; the readiness gate said ok on both runs. Not a gate: a comparison. Runs: `perf lit --views ground-1m,coast-2km` (a partial log, not a baseline) and `perf stream dive --lit`, each with the harness's cool-down. Logs: `target/smoke/` (not committed; numbers below).

## Verdict

The GPU numbers are the same within noise: +3 to +4 % on every pass of both lit views, with the clock 3 to 5 % lower than window 11 (ground-1m P0/P2 1511 MHz against P0 1590; coast-2km P0 1267 against 1312). Single-thread calibration before the run is +1.5 % against window 11. There is no 1.4x slowdown. Two things are not equal and stay open: the parallel calibration before the dive is +9 %, and the dive's frame p99 is +14 % (median +8 %), above the usual run-to-run noise of that scene.

## Readiness gate calibration (dive-lit; us, 18 threads)

The lit scene logs no calibration; the dive scene does.

| Value | window 11 | now | change | margin to the limit now (limits 50,000 / 100,000 as given by opus-planner; constants not found in the code) |
|---|---|---|---|---|
| before, single | 46,296 | 46,984 | +1.5 % | 6 % under |
| before, parallel median | 96,434 | 105,071 | +9.0 % | 5 % over |
| after, single | 46,902 | 59,335 | +26.5 % | 19 % over |
| after, parallel median | 95,388 | 91,121 | -4.5 % | 9 % under |

The gate said ok although "before, parallel median" is above 100,000, so the 100,000 limit is not the gate's actual judging value, or the gate judges something else. Window 11's other scenes ranged 88,331 to 96,434 (before, parallel) and 46,296 to 48,301 (before, single). "After, single" is measured at the end of a 74 s dive over 20 threads, with the post-pass still in the same process; window 11's value had no such rise. It is one sample.

## Lit views (GPU ms median; frame median / p99)

| View | pass | window 11 | now | change |
|---|---|---|---|---|
| ground-1m | terrain | 1.083 | 1.117 | +3.1 % |
| ground-1m | atmosphere | 0.211 | 0.218 | +3.3 % |
| ground-1m | frame | 2.908 / 3.428 | 2.868 / 3.432 | -1.4 % / +0.1 % |
| coast-2km | terrain | 1.024 | 1.061 | +3.6 % |
| coast-2km | atmosphere | 0.253 | 0.262 | +3.6 % |
| coast-2km | water | 1.647 | 1.707 | +3.6 % |
| coast-2km | frame | 4.523 / 4.921 | 4.490 / 5.254 | -0.7 % / +6.8 % |

Clock state: ground-1m P0/P2 1511 MHz (56-57 C, up to 85 W), coast-2km P0 1267 MHz (66-67 C, up to 92 W); coast-2km warm-up did not reach the 1400 MHz floor after 30 s, as in window 11 (ADR 0057, ADR 0060). Per-pass run-to-run noise: atmosphere 1.3 and 2.6 %, water 3.1 % (below ADR 0059's floors). Host load: system 4.3 % and 3.8 %, harness 2.3 % and 1.4 %.

Other passes now (ground-1m / coast-2km, ms median): sky-view 0.036 / 0.039, aerial-perspective 0.064 / 0.071, sky-composite 0.118 / 0.152, exposure-histogram 0.070 / 0.076, exposure-adapt 0.016 / 0.017, tonemap 0.038 / 0.043, scene-copy - / 0.076.

## Dive (lit, streaming; ms)

| | window 11 | now | change |
|---|---|---|---|
| frame median | 5.33 | 5.75 | +7.9 % |
| frame p99 | 13.57 | 15.41 | +13.6 % |
| frame max | 15.73 | 19.21 | +22 % |

Columns now (median / p99 / max): plan 0.68 / 15.70 / 1501.37 (off the frame's path), scheduler 0.20 / 0.99 / 2.69, upload 0.16 / 0.67 / 2.78, record 1.72 / 5.32 / 7.25. Clock state P8 210 MHz at the sample (15 samples; the scene takes no GPU time). All budgets ok; terrain slots 219,585 of 219,585; RAM peak 0.93 GiB; VRAM used 1.61 GiB. Window 10 to 11 was 5.41 -> 5.33 median and 13.04 -> 13.57 p99, so 15.41 is above both earlier windows. One run: not attributed.

## Open

The dive's p99 (+14 %) and the parallel calibration (+9 %, "after, single" +26 %) are single samples. If opus-planner wants them separated from noise, repeat the dive twice more; the lit GPU result does not need it.
