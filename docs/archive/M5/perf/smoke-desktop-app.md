# Perf smoke: coder session in the Claude desktop app, 2026-10-18

Question (owner, via opus-planner): does a coder session in the Claude desktop app measure like the VS Code panel did (window 11), with no repeat of the planner terminal's 1.4x per-thread slowdown?

Rasierklinge, code a32dafb (window 11's commit; `target/release` rebuilt from it, `sonnet-m4` worktree), session `sonnet-m4` running in the Claude desktop app. 16384 store of window 11. No `--machine-ready`; the readiness gate said ok on both runs. Not a gate: a comparison. Runs: `perf lit --views ground-1m,coast-2km` (a partial log, not a baseline) and `perf stream dive --lit`, each with the harness's cool-down. Logs: `target/smoke/` (not committed; numbers below).

## Verdict

The GPU numbers are the same within noise: +3 to +4 % on every pass of both lit views, with the clock 3 to 5 % lower than window 11 (ground-1m P0/P2 1511 MHz against P0 1590; coast-2km P0 1267 against 1312). Single-thread calibration before the run is +1.5 % against window 11. There is no 1.4x slowdown. The dive, run three times: frame median of 3 is +4.7 % and p99 of 3 is +12.4 % against window 11, but the p99 of the three runs spans 13.09 to 15.41 ms, as wide as the difference, and none of the runs has fully clean calibration (see "Verdict on the dive"). Not shown to be a desktop-app effect.

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

## Dive (lit, streaming; ms), three runs

Same store, code and cool start for each run. Run 1 is the first measurement; runs 2 and 3 are the repeat opus-planner asked for. Window 11: median 5.33, p99 13.57, max 15.73. Window 10 to 11 was 5.41 -> 5.33 median and 13.04 -> 13.57 p99.

| | run 1 | run 2 | run 3 | median of 3 | window 11 | median of 3 against window 11 |
|---|---|---|---|---|---|---|
| frame median | 5.75 | 5.38 | 5.58 | 5.58 | 5.33 | +4.7 % |
| frame p99 | 15.41 | 13.09 | 15.25 | 15.25 | 13.57 | +12.4 % |
| frame max | 19.21 | 17.75 | 18.35 | 18.35 | 15.73 | +16.6 % |
| calib before, single (us) | 46,984 | 46,251 | 46,501 | 46,501 | 46,296 | +0.4 % |
| calib before, parallel median | 105,071 | 101,285 | 95,713 | 101,285 | 96,434 | +5.0 % |
| calib after, single | 59,335 | 46,918 | 53,343 | 53,343 | 46,902 | +13.7 % |
| calib after, parallel median | 91,121 | 99,689 | 92,236 | 92,236 | 95,388 | -3.3 % |
| competing processes, mean / max | 7.2 / 20.4 % | 4.4 / 7.3 % | 5.2 / 7.1 % | | | |

Competing processes (the harness list, TEST-023, 8 samples over the run): run 1 git 2.2 % mean (max 16.9 %), System, svchost, Taskmgr, MsMpEng; runs 2 and 3 System, svchost, Taskmgr, MsMpEng, claude 0.2 and 0.3 %. System load 69 to 76 % (harness 65 to 71 %). CPU clock before / after: 2873 / 2873, 3046 / 3262, 3046 / 2894 MHz.

Other columns (median / p99 / max), run 1 / run 2 / run 3: scheduler 0.20/0.99/2.69, 0.19/0.97/3.71, 0.17/1.11/2.76; upload 0.16/0.67/2.78, 0.16/0.61/2.64, 0.16/0.64/2.56; record 1.72/5.32/7.25, 1.69/4.57/8.05, 1.74/5.52/7.88; plan max 1.50, 1.41, 1.50 s (off the frame's path). All budgets ok in all three runs; terrain slots 219,585 of 219,585; RAM peak 0.93 GiB and VRAM used 1.61 GiB (run 1).

## Verdict on the dive

By opus-planner's rule (calibration at most 50,000 single and 100,000 parallel, before and after), none of the three runs is fully clean: run 1 (before parallel 105,071, after single 59,335, `git` at up to 16.9 % during the run), run 2 (before parallel 101,285, 1 % over; otherwise clean) and run 3 (after single 53,343). The p99 spread across the three runs alone is 13.09 to 15.41 ms (18 %), larger than the median-of-3 difference to window 11 (+12 %). Run 2, the cleanest, is 3.5 % below window 11's p99 and 1 % over its median. The frame median is within 10 % of window 11 in every run (+4.7 % for the median of 3). So the dive's p99 is noise-bound at this sample size and not a demonstrated desktop-app effect; three runs with imperfect calibration cannot exclude a small real effect (at most about +12 %).

## Open

A repeat with calibration within the rule on every sample (nothing like `git` running) would settle the dive p99. The lit GPU result stands as measured.
