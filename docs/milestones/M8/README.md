# M8 milestone folder: index

Owner-facing guide to this folder (opus-planner, 2026-10-27). Everything here is evidence, not goldens.

## Owner live test 2026-10-27
`owner-test-2026-10-27/README.md`: 16 findings in groups A to H and the plan before M8's acceptance (editor stalls, missing shadow bakes, chunk edges and tile cuts first).

## For the owner's look review (criterion 1)
Current set: `look-review-2b/` (ANET_VERSION 10, rendered after #757). Later changes up to 2026-10-27 do not change the picture (byte-identical horizon maps, performance only).

| Look at | Site | Why |
|---|---|---|
| `look-review-2b/hills-b/` | lat 8.972, lon -16.238 | green hill country with a river valley and a cut channel; the main judgement site |
| `look-review-2b/river-r1/` | lat -11.1657, lon 23.0493 | the large lowland river (review 1's site); channel and valley form |

Each site holds `default/` and `sample-driver-look/` with six stills: 300 m, 30 m and 1.8 m eye, each at midday and sunset.

Questions for the owner:
1. River and valley forms on plausible relief, with horizon shadows, against the look reference (realism of hill terrain is M9).
2. Default look or sample driver look (ground detail 0.6, soil and rock curves) as the M8 default.

Known and tracked, not to be judged as new: faint striations on shadowed walls beyond a rim (hillslope flat patches, Tracked in `docs/status.md`).

## Superseded (deleted at M8's housekeeping, 2026-10-30; in the git history)
| Folder | What it was |
|---|---|
| `look-review-1/`, `look-review-1-hills*/` | look review 1 and its three fix iterations (2026-10-25), before the relief rebuild |
| `horizon-shadows-hills/`, `horizon-shadows-lowland/` | first horizon-shadow inspection (2026-10-25) |
| `look-review-2/`, `look-review-2-hills/` | look review 2 on the rebuilt store; replaced by `look-review-2b/` |
| `look-review-2b/hills-r1/` | review 1's hills site; snow-covered at 3.8 km after the rebuild, looks indistinguishable |

## Acceptance
`acceptance-runbook.md`: what `cargo xtask accept M8` runs, per criterion the command, the latest number and what is missing, and the order of the runs for one window.

## Performance records (for the acceptance report, not for review)
| Files | Criterion |
|---|---|
| `criterion-6-edit-window.md` | criterion 6 summary |
| `edit-latency-w24-main819-5x.md`, `edit-latency-w25-drawn-again.md` | criterion 6, the latest windows; the earlier windows (`edit-latency-*`, `edit-resolve-w*-phases.log`) are in `docs/archive/M8/perf/` |
| `docs/archive/M8/perf/envelope-w*.json` | criterion 7 windows 8 to 14; the latest are `envelope-w12-steady-300.json` and `envelope-w12-steady-rest.json` |
| `docs/archive/M8/perf/anet-change-bound-48-chunks.log` | the chunk stage's measured height change (input to the plan bounds; the stage is removed, ADR 0086) |
