# Perf window 13 (edit latency and the window-12 repeat), 2026-10-19

Rasierklinge, main 0c637bc (the perf preflight #392, the edit-latency scene #386), release build of that tree. `cargo xtask perf preflight --out target/perf-w13`, then Part A `cargo xtask perf m5 --store <16384 store> --out target/perf-w13`, then Part B `cargo xtask perf stream edit-latency --store <16384 store> --out target/perf-w13`; no `--machine-ready`. Run by sonnet-explorer on opus-planner's go (owner: laptop on mains power and idle); the other sessions were paused. Plan and empty tables: `docs/perf/window-13-plan.md`. Logs in `target/perf-w13/` (not committed).

## Verdict

- **Preflight OK** (TEST-040, first use): AC connected, High performance plan, calibration within the ADR 0065 limits in all three rounds.
- **Part A, streaming scenes:** all seven scenes ran to the end, every scene `SESSION VALID for budgets: frame verdicts: yes`, calibration inside the limits before and after each. Two gated measures are over budget, both on the dive (below, "New finding").
- **Part A, lit views:** the lit session is **INVALID**: four of the lit runs are outside the ADR 0065 limits (coast-3m after, coast-glide before, coast-glide@filter1 before parallel median by 555 us, coast-descent after). No lit verdict counts; the figures are reported, not judged.
- **Part B (edit latency): every case is OVER its budget.** Preview 74 to 247 ms against 16 ms; exact 385 to 1283 ms, over the budget in 8 of 9 cases (only 2 km with a 500 m stamp is within it). The first run of Part B panicked in the scheduler (a refresh request left in the class its tile had left, `layer.rs:214`); fixed in #397 (STRM-026) and rerun after a second preflight (the first of the rerun refused on a 52,837 us single reading while the Windows search indexer ran; the second passed). The session is valid: gate ok.

## Machine (preflight, `target/perf-w13/preflight.md`)

| | |
|---|---|
| AC power, plan | connected, High performance (8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c) |
| top competing processes | claude 0.6 %, System 0.5 %, MsMpEng 0.3 %, svchost 0.3 %, dwm 0.2 % |
| calibration single / parallel median (us) | round 1: 48,965 / 69,389; round 2: 48,695 / 97,881; round 3: 48,405 / 83,583 (limits 52,000 / 112,000) |
| readiness gate | ok (before each part) |

## A. Lit views, window 11 -> 12 -> 13 (ms median; the session is invalid, nothing here counts)

| View | terrain | atmosphere | water | frame median / p99 | clock w11 -> w12 -> w13 (MHz) |
|---|---|---|---|---|---|
| ground-1m | 1.083 -> 1.129 -> 1.296 | 0.211 -> 0.223 -> 0.216 | not in view | 2.908 -> 2.982 -> 3.050 / 3.428 -> 4.881 -> 3.624 | 1590 -> 1395 -> 1474 |
| aerial-20km | 0.776 -> 0.863 -> 0.848 | 0.235 -> 0.267 -> 0.250 | 0.108 -> 0.047 -> 0.120 | 2.332 -> 2.266 -> 2.265 / 3.071 -> 2.911 -> 3.141 | 1424 -> 1264 -> 1357 |
| coast-2km | 1.024 -> 1.150 -> 1.165 | 0.253 -> 0.284 -> 0.267 | 1.647 -> 1.768 -> 1.786 | 4.523 -> 4.781 -> 4.684 / 4.921 -> 5.253 -> 5.247 | 1312 -> 1237 -> 1170 |
| coast-3m | 1.312 -> 1.418 -> 1.615 | 0.233 -> 0.249 -> 0.248 | 1.938 -> 2.024 -> 2.129 | 5.586 -> 5.785 -> 5.841 / 6.083 -> 6.682 -> 6.379 | 1327 -> 1252 -> 1237 |
| orbit | 0.053 -> 0.145 -> 0.141 | 0.684 -> 0.748 -> 0.692 | 0.169 -> 0.215 -> 0.274 | 1.485 -> 1.799 -> 1.783 / 2.078 -> 2.135 -> 2.306 | 1462 -> 1334 -> 1410 |

## A. Streaming scenes (`--lit`; frame verdicts count)

| Scene | median | p99 | max | descent lag mean / worst | calibration single before / after (us) |
|---|---|---|---|---|---|
| dive | 5.81 | 16.89 | 70.92 (frame 2) OVER | 0.254 (budget 1) / 1.48 (budget 3) | 49,632 / 50,935 |
| dive-fast | 10.57 | 16.35 | 21.32 | 0.302 / 1.48 | 46,278 / 50,049 |
| dive-cold (not gated) | 5.89 | 15.55 | 17.08 | 0.650 / 2.60 | 50,190 / 47,387 |
| teleport-1m | 6.31 | 8.36 | 9.83 | tracked: 2.33 s under 1 level (budget 2 s), draw set complete 6.03 s (budget 9 s) | 46,550 / 48,320 |
| teleport-2km | 4.15 | 5.09 | 8.32 | tracked: 1.37 s (budget 1 s), complete 3.52 s (budget 4 s) | 51,218 / 51,278 |
| look-1m | 3.78 | 5.09 | 5.46 | not read | 46,504 / 46,217 |
| look-2km | 1.98 | 2.97 | 3.59 | not read | 48,549 / 46,824 |

Window 12 for comparison (did not count): dive 7.70 / 16.24 / 85.41, dive-fast 8.90, dive-cold 6.43, teleport-1m 5.67, teleport-2km 4.56, look-1m 3.77, look-2km 2.00; descent lag worst frame 3.13 (over the budget 3). Now 1.48 on both dives. RAM at most 1.54 GiB (dive-cold), terrain slots 219,585 of 219,585, driver VRAM at most 1.52 GiB (dive-fast).

## Window 12's open items, settled

- **Orbit terrain 0.053 -> 0.145 ms and orbit frame +21 %:** reproduced (0.141 ms at 1410 MHz, above the floor), so it is no clock effect. Attribution: the render floor 3 (ADR 0062): the orbit view draws 181 level-3 tiles instead of 52 level-2 tiles. Expected, not a regression.
- **aerial-20km water 0.108 -> 0.047 -> 0.120:** window 12's value was the outlier; window 13 equals window 11.
- **Descent lag worst frame 3.13 over the budget:** 1.48 now (valid session), within the budget.
- **Dive max 85 ms:** still there (70.9 ms at frame 2), see below.

## New finding: the dive's start-up frame (two gated measures over budget, valid session)

- `dive: frame max 70.92 ms at frame 2 (cap 40)` and `dive: upload bytes at most 3.357 MiB per frame (cap 1 MiB)`; the dive-fast upload is 3.357 MiB too; dive-cold shows the same value (not gated).
- 3.357 MiB is the start-up load of the 384 root tiles of the render floor 3 (STRM-001: loaded before frame 0 and not one of the measured frames). The measured frames 1 to 3 carry that upload to the GPU, so the first frames of every dive pay for it: a 70 ms frame once at the start of the descent, with no effect on the other scenes' verdicts.
- Open: either the roots are uploaded before frame 0 (warm-up) or the metric excludes frames before the roots are resident; the spec (STRM-001, TEST-019) says the first, the harness does the second. Not attributed beyond the 3.357 MiB equality; needs a change of its own.

## Part B (edit latency)

Rerun after #397 (STRM-026), release build of main 0c139df, `cargo xtask perf stream edit-latency --store <16384 store> --out target/perf-w13b`; the readiness gate was ok; preflight of the rerun: single 46,826 / 46,272 / 50,968 us, parallel median 102,231 / 98,970 / 79,674 us (a first preflight 30 min before refused on round 1, single 52,837 us, with the Windows search indexer running; this one passed with the search indexer out of the top five). The first run (main 0c637bc) panicked in the 200 m / 2 km part of the sequence (scheduler, fixed) and wrote no table. Median of three per row, 2560x1440:

| altitude | stamp radius | preview ms (budget 16) | exact ms (budget) | tiles refreshed / resident | previewed nodes (max) | repeats | verdict |
|---|---|---|---|---|---|---|---|
| 1 m | 50 m | 211.2 | 1116.1 (1000) | 542 / 11886 | 321 | 3 | OVER |
| 1 m | 150 m | 224.7 | 1069.5 (1000) | 519 / 12333 | 309 | 3 | OVER |
| 1 m | 500 m | 246.8 | 1283.4 (1000) | 514 / 12532 | 309 | 3 | OVER |
| 200 m | 50 m | 85.6 | 501.5 (300) | 115 / 9120 | 30 | 3 | OVER |
| 200 m | 150 m | 118.6 | 851.2 (300) | 291 / 9326 | 146 | 3 | OVER |
| 200 m | 500 m | 187.6 | 1211.7 (1000) | 469 / 9592 | 279 | 3 | OVER |
| 2 km | 50 m | 74.0 | 407.1 (300) | 58 / 6786 | 6 | 3 | OVER |
| 2 km | 150 m | 78.5 | 385.5 (300) | 66 / 6786 | 9 | 3 | OVER |
| 2 km | 500 m | 76.5 | 418.0 (1000) | 96 / 6786 | 26 | 3 | OVER (preview only) |

Settle times (cold first row of each rig, then the same rig): 1 m 17.3 / 3.8 / 7.2 s, 200 m 15.7 / 0.5 / 2.0 s, 2 km 11.9 / 0.0 / 0.0 s.

### Reading

- **Preview 5 to 15 times over its 16 ms budget** (74 to 247 ms), and it grows with the resident count and the tiles refreshed (6,786 resident: 74 to 79 ms; 9,100 to 9,600: 86 to 188 ms; 11,900 to 12,500: 211 to 247 ms). SPIKE-13 measured the vertex displacement itself at 4 to 14 ms, so the time before the first previewed frame is not the displacement. Not measured here which stage it is; the likely one is the plan: a node is flagged only when the plan names a mesh key that differs from the resident tile's, so the first previewed frame waits for the plan worker's re-selection after the definition revision (the pipelined loop delivers a plan per request; its time grows with the resident count). Needs a per-stage timestamp in the scene (`set_definition` return, `edit` return, plan delivered, first flagged frame) before a fix is chosen.
- **Exact against its budget:** 1 m: 1.07 to 1.28 s against 1 s; 200 m: 0.50 s (50 m, budget 0.3 s), 0.85 s (150 m, 0.3 s), 1.21 s (500 m, 1 s); 2 km: 0.41 s and 0.39 s (budget 0.3 s) and 0.42 s (500 m, within 1 s). The exact time follows the tile count refreshed (58 tiles 0.41 s; 514 to 542 tiles 1.1 to 1.3 s), i.e. about 2 to 3 ms per refreshed tile plus a base of about 0.3 s at 2 km, which includes the same plan latency as the preview.
- **What this says for the M6 exit criterion** (edits visible within 1 s): the preview, which is what makes an edit visible before the exact tiles, is at 74 to 247 ms and not at 16 ms; the exact result is within 1 s at 200 m (50 m and 150 m stamps) and at 2 km (all three), and over it at 1 m (1.07 to 1.28 s) and at 200 m with the 500 m stamp (1.21 s).
- Next measurements: the per-stage split above; the same table with the plan worker's time taken out; whether the base of 0.3 s is the retained `edit` re-plan or the first job start.
