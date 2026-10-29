# M8 acceptance runbook

Readiness check (sonnet-explorer, 2026-10-28), amended to the owner's fallback (crusader, 2026-10-28; `docs/briefings/m8-plan.md` "Amendment 2026-10-28": chunk stage off by default, hydrology and erosion out of M8). Nothing was measured for this file; the numbers are the latest recorded ones, and rows measured with the chunk stage on are marked stale. Criteria: `docs/briefings/m8-plan.md`.

## What `cargo xtask accept M8` does
The generic steps below, then an M8 arm (TEST-060, `planet_perf::accept_m8`) that judges criteria 4 to 7 from the logs in `perf/logs/` (or `--logs DIR`) and prints a PASS, FAIL or MISSING row each; a failed row fails the command, a MISSING row makes it incomplete. It measures nothing. Names it reads: `coupled-make.json` (at most 600 s from the 16384 bake), `shadow-cost.json` (every pose at most 1.5 ms; runtime-off only and within 20 % is MISSING), `edit-latency.md` (copy it from the `--out` directory; no `OVER` or `over` row in the M6 table `verdict` and in the horizon table `verdict (10 s, horizon maps)`; the chunk table `verdict (10 s, drawn tiles)` is not judged; no horizon table is MISSING), `envelope.json` (the steady speed at 300 m at least 200 m/s; needs a `steady_curve`, so run with `--only-steady`). A smoke, other-scene, other-store or overridden log is MISSING. Criteria 1, 2, 3 and CI stay on the manual list printed by the command (criterion 3 is the real-store gate, `cargo xtask gate`; `gate --small` runs as an automated step).

| Step | Run by `accept M8` | Note |
|---|---|---|
| check (fmt, clippy, layering) | yes | |
| spec-lint | yes | |
| test (tiers A+B, software adapter) | yes | |
| test gpu (tier C, software adapter) | yes | On this machine the software adapter times out 9 to 14 cases (all in `gpu_window`, `gpu_stream_perf`, `gpu_water_coverage`, `gpu_envelope_perf`); CI (WARP, lavapipe) passes them. Read the CI result for the merge commit, not the local step. |
| test gpu --real | yes (M8 is not M4 or M5) | Never during a perf window (CLAUDE.md). Run it before or after the window. |
| gate --small (TEST-061 to TEST-063) | yes (M8) | The no-degradation counters on a generated 32-cell world on the software adapter. |
| run modes `editor --smoke`, `generate --smoke`, `test-render --smoke` | yes | The Tracked item says `editor --smoke` does not complete on the real store (17 frames); the step uses no store. |
| test fast (timing) | yes | Over its 20 s budget on a loaded machine (34 to 40 s seen); judged by the step. |

Copy the window's logs to `perf/logs/` under the names above (the scenes write `coupled-make.json`, `shadow-cost.json` and `envelope.json` there by default; keep a dated copy under `docs/milestones/M8/` afterwards).

## Criteria: command, latest number, what is missing
Machine for every perf row: Rasierklinge (RTX 3080 Ti Laptop), store `C:/DEV/SeriousPlanetData/global16384`, readiness gate ok. A perf run needs the planner's authorization and the heavy-run lock (`cargo xtask heavy` is built into `perf`).

| # | Criterion | Tier | Command | Latest recorded | Missing |
|---|---|---|---|---|---|
| 1 | Owner's look review at the fixed poses on plausible relief with horizon shadows; river and valley forms are not judged in M8 | D | Owner reads `docs/milestones/M8/look-review-2b/` (hills-b, river-r1; `default/` and `sample-driver-look/`). Live: editor with `--look docs/milestones/M8/look-review-2b/sample-driver-look.look.toml`, F8 switches the looks (#836). | Stills of 2026-10-26 (ANET_VERSION 10) were rendered with the chunk stage on; B6 coarse-horizon fix (#835) is newer | Re-render the stills with the chunk stage off (explorer's switch); owner's verdict and the default-look decision (README question 2). Owner test finding 6 at a low sun. |
| 2 | Chunk borders identical; CPU bit-exact across threads and OS | A | `cargo xtask test` (tiers A+B); CI job `determinism manifest (GEN-003)` and the Windows-vs-Linux comparison | Green on #837 (all CI jobs, Windows and Linux) | Nothing. Take the CI result of the merge commit. |
| 3 | No frame waits (CON-22): the gate (TEST-061 to TEST-063) on the real store with the chunk stage off: no frame over 100 ms, 0 held-back tiles without edits, no step between levels, no panic | A, C | A: `cargo xtask test` and `cargo xtask gate --small` (an automated step of `accept M8`). Real store: `cargo xtask gate` (stage at its default, off; never inside a perf window; no perf run) | The D0 measurement (`docs/milestones/M8/d0/README.md`) failed its stop rule, so the chunk stage is off and its figures are no longer the criterion: with the stage on, the entry step against the base was p99 14 to 110 m and the held-back count 50 to 120 tiles at two poses. The earlier 0.712 m (15/14, chunk 0x0b000000002cba11) is the step between pyramid levels 15 and 14 of the chunk's own pyramid, not the production boundary; D0 measured 0.390 m for the same quantity on chunk 0x0b000000002bd53a | The stage-off run of the real-store gate (explorer's switch and the horizon move first). |
| 4 | Global coupled run at most 10 min | perf | `cargo xtask perf coupled --store C:/DEV/SeriousPlanetData/global16384 --remake-into <OUT>` | 19.7 s from the 16384 bake (window 21, 2026-10-27; reduction 17.0 s, 100 steps 2.7 s; budget 600 s; `perf/logs/coupled-make.json`). Earlier synthetic: 545 s (`perf/logs/coupled.json`) | Repeat once on the final world version; 20 s plus store load. |
| 5 | Shadows and material map together at most 1.5 ms of terrain-pass GPU at 1440p | perf | `cargo xtask perf stream shadow-cost --store C:/DEV/SeriousPlanetData/global16384 --global-res 16384 --poses docs/milestones/M8/look-review-2b/river-r1/default/poses.toml --out-dir target/perf-wNN` | river-r1, compiled-out, 3 round-robin rounds (window 25): 300 m sunset 1.42 ms, 300 m midday 1.37, 30 m 1.00 / 1.03, 1.8 m 0.96 / 0.97 (`perf/logs/shadow-cost-river-r1-w25.json`); hills-b (window 22, runtime-off) 0.71 to 1.15 | Re-measure after #835: the coarse horizon now reads three texels a fragment instead of one, the margin at 300 m is 5 to 9 %. Also hills-b with compiled-out draws (only the runtime-off draw is recorded). |
| 6 | An edit shows within the M6 budgets; its horizon shadows update within 10 s on screen for every map the view wants | A, perf | `cargo xtask perf stream edit-latency --store C:/DEV/SeriousPlanetData/global16384 --repeats 5 --out target/perf-wNN` (the chunk-worker options no longer matter with the stage off) | Window 24: M6 column 13 of 13 gated rows ok (64 to 761 ms), preview 13.1 ms (budget 16). The chunk rows (eroded result within 10 s; 18 of 19 ok in window 25) are stale and not judged | The scene must log a horizon table `verdict (10 s, horizon maps)` (explorer's horizon move): until then `accept M8` reads criterion 6 as MISSING. A stage-off window. |
| 7 | Speed envelope; floor 200 m/s at 300 m for steady flight, curve recorded; the stand-in share is measured on the tiles and horizon maps the view wants (no chunk change) | perf | `cargo xtask perf stream envelope --store C:/DEV/SeriousPlanetData/global16384 --only-steady --altitudes-m 300 --steady-window-s 60 --out target/perf-wNN` (all four altitudes: omit `--altitudes-m`) | Steady curve with the chunk stage on (windows 12 and 14, 2026-10-27): 300 m 480 m/s, 30 m 320, 100 m 340 / 360, 1000 m 460 / 480; stale for the amended method | Repeat with the stage off (the chunk wait that limited the cold curve is gone; tiles and horizon maps only). `envelope.json` records `chunk_stage`, `horizon` and `source`; `accept M8` reads criterion 7 as MISSING unless the stage was off, the horizon jobs ran and the source is the production store. Pass `--out DIR` (a directory) or a `.json` path. |

## Order of the runs for one window
No perf run starts without the planner's go; the first four are not perf and can run before the window opens. Times are estimates from the recorded runs (the harness logs no wall time for edit-latency; its figure is a guess and is marked).

| Order | Run | Needs window | Time |
|---|---|---|---|
| 0 | Planner clears the machine: crusader and spikes send "free"; the owner's quiet flag is up (`cargo xtask quiet on`) | | |
| 1 | `cargo xtask accept M8` (steps in the table above); read the local tier C timeouts against the CI result of the merge commit | no (run before; includes `test gpu --real`, which never runs inside the window) | about 25 to 35 min with the timeouts |
| 2 | `cargo xtask gate` on the real store, stage off (criterion 3) | no (an editor window run, not a measurement) | minutes |
| 3 | Look stills after #835 (`cargo xtask capture stills --poses ... --store ...`) for criterion 1 | no (GPU, but not a measurement) | 5 to 10 min |
| 4 | `perf coupled --remake-into` (criterion 4) | yes | about 1 min |
| 5 | `perf stream shadow-cost`, river-r1 poses, then hills-b poses (criterion 5) | yes | about 5 min each |
| 6 | `perf stream edit-latency --chunk-workers 6 --base-threads 2 --repeats 5` (criterion 6) | yes | about 30 min (guess, 19 cases times 5 repeats) |
| 7 | `perf stream envelope --only-steady --altitudes-m 300` (criterion 7 floor) | yes | 11 trials of 72 s: about 14 min |
| 8 | `perf stream envelope --only-steady` at 100 m and 1000 m, two places (flight lead) | yes | 4 curves of 11 trials: about 55 min |
| 9 | Write `docs/milestones/M8-report.md`: criteria table, the logs copied to `docs/milestones/M8/`, owner items | no | planner |

Total inside the window: about 1 h 45 min for steps 4 to 8; steps 7 and 8 can be cut to the 300 m run (14 min) if the flight lead is judged later.

## Owner items at the gate
- Criterion 1 look verdict and the default look (`default/` or `sample-driver-look/`), with F8 live at hills-b.
- Owner live test 2026-10-27: finding 6 at a low sun, K without a panic, plateau shadows visible in the editor, group E and F/G scope.
- The M8 go/no-go (CLAUDE.md: a milestone gate is a stop point after M2 and M5 only; M8 reports and goes on unless the planner says otherwise).
