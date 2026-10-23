# SPIKE-38: process data

Data only; the recommendations are the planner's. Collected read-only on 2026-10-23 by sonnet-crusader (SPIKE-38, owner's development process review before M8). Time box about 1.5 h.

## Sources and method

| Item | Source | Notes |
|---|---|---|
| PRs | `gh pr list --state all` (574: 567 merged, 7 closed unmerged) and `pulls/N/files` per PR | PRs exist from 2026-10-10 (M0 to M3 were mostly pushed before), so "M4 (to 10-16)" holds the late M3 and M4 PRs |
| Periods | merge date UTC | M4 to 10-16, M5 10-17 to 10-18, M6 10-19 to 10-21, M7 10-22 on (M7 closed 10-23; the log has fragments dated 10-24) |
| PR kind | files changed | docs-only = every file under `docs/`, `openspec/`, `.claude/`, root `*.md`, `spikes/**/results/`; docs-only with a `spike` branch or a `SPIKE-n` title = spike; everything else = code |
| Session / lane | branch prefix and the `Co-Authored-By` model of the squash commit; lane names from the log | PR bodies and branches do not carry the session name. `change/` branches: 313 of 421 cannot be tied to a lane (the log names it for only 108). Model: Sonnet = coder sessions, Opus = planner/architect |
| CI runs | `gh run list` (3263 runs since 2026-10-10) and `actions/runs/N/jobs` for the 1714 `ci` runs (13 returned a server error and are missing) | `docs/perf/ci-history.tsv` covers only 2026-10-16 to 10-18, so job medians come from the API (job started to completed, success only) |
| Failure causes | failed step names; whether a new commit followed; `attempt` > 1 | GitHub does not say flake, real or infra. A class here is a proxy, stated per table |
| Lines | per-file additions + deletions of merged PRs | a move into `docs/archive/` counts twice (delete + add); binary goldens count 0 |
| Log, learnings, perf windows | `docs/log.md`, `docs/log.d/`, `docs/learnings.md`, archives, `docs/perf/window-*.md` first-commit dates | counts by keyword, so a lower bound |

Job names changed over the period: one `windows (WARP)` / `linux (lavapipe)` job until M5; `A+B`, `C` (and `C 1/2`, `C 2/2` on Windows) from M6.

# Tables

model Counter({'Sonnet': 401, 'Opus': 146, 'none': 12, 'both': 7, '?': 1})
## 1a merged PRs per day
| date UTC | merged | code | docs-only | spike | Sonnet | Opus | change/ | planner/ | docs/ |
|---|---|---|---|---|---|---|---|---|---|
| 2026-10-10 | 37 | 30 | 7 | 0 | 27 | 8 | 36 | 0 | 0 |
| 2026-10-11 | 21 | 12 | 9 | 0 | 12 | 8 | 21 | 0 | 0 |
| 2026-10-12 | 31 | 18 | 11 | 2 | 20 | 8 | 31 | 0 | 0 |
| 2026-10-13 | 7 | 2 | 5 | 0 | 2 | 5 | 7 | 0 | 0 |
| 2026-10-14 | 11 | 6 | 5 | 0 | 6 | 4 | 11 | 0 | 0 |
| 2026-10-15 | 47 | 31 | 16 | 0 | 34 | 12 | 47 | 0 | 0 |
| 2026-10-16 | 46 | 20 | 24 | 2 | 24 | 14 | 37 | 6 | 3 |
| 2026-10-17 | 61 | 31 | 28 | 2 | 40 | 20 | 31 | 20 | 9 |
| 2026-10-18 | 69 | 31 | 34 | 4 | 48 | 20 | 37 | 20 | 12 |
| 2026-10-19 | 83 | 51 | 29 | 3 | 67 | 15 | 59 | 15 | 9 |
| 2026-10-20 | 71 | 37 | 29 | 5 | 59 | 12 | 52 | 12 | 7 |
| 2026-10-21 | 41 | 22 | 18 | 1 | 29 | 12 | 26 | 12 | 3 |
| 2026-10-22 | 32 | 20 | 4 | 8 | 30 | 2 | 25 | 2 | 3 |
| 2026-10-23 | 10 | 2 | 5 | 3 | 3 | 6 | 1 | 6 | 1 |

## 1b by period
| period | merged | code | docs | spike | Sonnet | Opus | both |
|---|---|---|---|---|---|---|---|
| M4 (to 10-16) | 200 | 119 | 77 | 4 | 125 | 59 | 4 |
| M5 (10-17..18) | 130 | 62 | 62 | 6 | 88 | 40 | 1 |
| M6 (10-19..21) | 195 | 110 | 76 | 9 | 155 | 39 | 1 |
| M7 (10-22..) | 42 | 22 | 9 | 11 | 33 | 8 | 1 |
| all | 567 | 313 | 224 | 30 | 401 | 146 | 7 |

## 1c lead time h
| period | kind | n | median h | p90 h | max h |
|---|---|---|---|---|---|
| M4 (to 10-16) | code | 119 | 0.19 | 0.53 | 9.4 |
| M4 (to 10-16) | docs | 77 | 0.16 | 0.60 | 23.8 |
| M4 (to 10-16) | spike | 4 | 0.18 | 0.21 | 0.2 |
| M4 (to 10-16) | all | 200 | 0.18 | 0.57 | 23.8 |
| M5 (10-17..18) | code | 62 | 0.38 | 0.84 | 11.6 |
| M5 (10-17..18) | docs | 62 | 0.01 | 0.02 | 4.0 |
| M5 (10-17..18) | spike | 6 | 0.01 | 0.04 | 0.1 |
| M5 (10-17..18) | all | 130 | 0.07 | 0.67 | 11.6 |
| M6 (10-19..21) | code | 110 | 0.34 | 0.96 | 20.0 |
| M6 (10-19..21) | docs | 76 | 0.02 | 0.10 | 4.9 |
| M6 (10-19..21) | spike | 9 | 0.02 | 0.03 | 0.0 |
| M6 (10-19..21) | all | 195 | 0.24 | 0.71 | 20.0 |
| M7 (10-22..) | code | 22 | 0.33 | 0.83 | 2.5 |
| M7 (10-22..) | docs | 9 | 0.02 | 0.04 | 0.1 |
| M7 (10-22..) | spike | 11 | 0.03 | 0.32 | 3.7 |
| M7 (10-22..) | all | 42 | 0.25 | 0.77 | 3.7 |
| all | code | 313 | 0.31 | 0.84 | 20.0 |
| all | docs | 224 | 0.02 | 0.30 | 23.8 |
| all | spike | 30 | 0.02 | 0.22 | 3.7 |
| all | all | 567 | 0.20 | 0.68 | 23.8 |

## prefix x model
| prefix | model | n |
|---|---|---|
| change | ? | 1 |
| change | Opus | 53 |
| change | Sonnet | 349 |
| change | both | 6 |
| change | none | 12 |
| docs | Sonnet | 46 |
| docs | both | 1 |
| m0-foundations | Sonnet | 1 |
| perf | Sonnet | 1 |
| planner | Opus | 93 |
| spike | Sonnet | 4 |

closed unmerged [(559, 'change/atmo-top-continuity'), (46, 'change/m3-plan-cache'), (45, 'change/m3-cache-adrs'), (44, 'change/m3-gen-context'), (36, 'change/m2-steep-terrain'), (3, 'change/m1-generator-first-tile'), (2, 'change/m1-coordinates')]
## 2 CI
## 2a job medians s (success only), main push / PR
### main push
| job | M4 (to 10-16) | M5 (10-17..18) | M6 (10-19..21) | M7 (10-22..) |
|---|---|---|---|---|
| determinism (Windows vs Linux) | 17 / p90 25 (n=162) | 24 / p90 27 (n=62) | 24 / p90 29 (n=108) | 24 / p90 27 (n=22) |
| linux (lavapipe) | 214 / p90 405 (n=181) | 626 / p90 754 (n=59) | - | - |
| linux (lavapipe) A+B | - | 431 / p90 433 (n=3) | 440 / p90 548 (n=106) | 440 / p90 478 (n=20) |
| linux (lavapipe) C | - | 473 / p90 480 (n=3) | 496 / p90 588 (n=107) | 496 / p90 568 (n=22) |
| windows (WARP) | 406 / p90 671 (n=180) | 1100 / p90 1306 (n=55) | - | - |
| windows (WARP) A+B | - | 637 / p90 681 (n=3) | 741 / p90 833 (n=102) | 766 / p90 839 (n=20) |
| windows (WARP) C | - | 880 / p90 929 (n=3) | 922 / p90 1086 (n=28) | - |
| windows (WARP) C 1/2 | - | - | 725 / p90 804 (n=76) | 792 / p90 908 (n=22) |
| windows (WARP) C 2/2 | - | - | 672 / p90 726 (n=78) | 764 / p90 823 (n=22) |

### PR runs
| job | M4 (to 10-16) | M5 (10-17..18) | M6 (10-19..21) | M7 (10-22..) |
|---|---|---|---|---|
| determinism (Windows vs Linux) | 17 / p90 24 (n=269) | 24 / p90 27 (n=109) | 24 / p90 30 (n=180) | 26 / p90 30 (n=38) |
| linux (lavapipe) | 204 / p90 393 (n=272) | 643 / p90 764 (n=84) | - | - |
| linux (lavapipe) A+B | - | 400 / p90 436 (n=4) | 450 / p90 548 (n=166) | 444 / p90 558 (n=32) |
| linux (lavapipe) C | - | 487 / p90 502 (n=4) | 495 / p90 588 (n=170) | 506 / p90 587 (n=38) |
| windows (WARP) | 376 / p90 676 (n=278) | 1160 / p90 1307 (n=81) | - | - |
| windows (WARP) A+B | - | 719 / p90 736 (n=4) | 740 / p90 841 (n=169) | 760 / p90 792 (n=32) |
| windows (WARP) C | - | 814 / p90 914 (n=4) | 913 / p90 1055 (n=51) | - |
| windows (WARP) C 1/2 | - | - | 715 / p90 790 (n=113) | 794 / p90 920 (n=38) |
| windows (WARP) C 2/2 | - | - | 678 / p90 778 (n=122) | 752 / p90 832 (n=37) |

## 2c failed jobs by step

"where": `pull_request` = PR runs, `push` = pushes to a non-main branch, `main` = pushes to main.
| job | failed step | where | n |
|---|---|---|---|
| linux (lavapipe) | test gpu (tier C, software adapter) | push | 35 |
| windows (WARP) | check (fmt, clippy, layering) | push | 32 |
| linux (lavapipe) | check (fmt, clippy, layering) | push | 31 |
| windows (WARP) | test gpu (tier C, software adapter) | push | 27 |
| linux (lavapipe) | test gpu (tier C, software adapter) | pull_request | 25 |
| linux (lavapipe) A+B | check (fmt, clippy, layering) | push | 21 |
| windows (WARP) A+B | check (fmt, clippy, layering) | push | 21 |
| windows (WARP) | test gpu (tier C, software adapter) | pull_request | 19 |
| windows (WARP) | check (fmt, clippy, layering) | pull_request | 18 |
| linux (lavapipe) | check (fmt, clippy, layering) | pull_request | 18 |
| windows (WARP) C 1/2 | test gpu (tier C, software adapter) | push | 15 |
| windows (WARP) C 1/2 | test gpu (tier C, software adapter) | pull_request | 12 |
| linux (lavapipe) C | test gpu (tier C, software adapter) | pull_request | 10 |
| linux (lavapipe) C | test gpu (tier C, software adapter) | push | 10 |
| windows (WARP) | test (tiers A+B) | push | 8 |
| linux (lavapipe) A+B | spec-lint | pull_request | 7 |
| windows (WARP) A+B | spec-lint | pull_request | 7 |
| windows (WARP) A+B | check (fmt, clippy, layering) | pull_request | 7 |
| linux (lavapipe) A+B | check (fmt, clippy, layering) | pull_request | 7 |
| linux (lavapipe) A+B | spec-lint | push | 6 |
| windows (WARP) A+B | spec-lint | push | 6 |
| windows (WARP) C 2/2 | test gpu (tier C, software adapter) | push | 6 |
| linux (lavapipe) A+B | test (tiers A+B) | pull_request | 6 |
| linux (lavapipe) A+B | test (tiers A+B) | push | 6 |
| windows (WARP) C | test gpu (tier C, software adapter) | push | 6 |
| windows (WARP) A+B | test (tiers A+B) | push | 5 |
| windows (WARP) C 2/2 | test gpu (tier C, software adapter) | pull_request | 4 |
| windows (WARP) C 1/2 | test gpu (tier C, software adapter) | main | 4 |
| windows (WARP) A+B | test (tiers A+B) | main | 4 |
| windows (WARP) C | test gpu (tier C, software adapter) | pull_request | 4 |
| windows (WARP) | test (tiers A+B) | main | 4 |
| windows (WARP) A+B | spec-lint | main | 3 |
| linux (lavapipe) A+B | spec-lint | main | 3 |
| windows (WARP) A+B | test (tiers A+B) | pull_request | 3 |
| windows (WARP) | test (tiers A+B) | pull_request | 3 |
| linux (lavapipe) A+B | check (fmt, clippy, layering) | main | 1 |
| windows (WARP) C 2/2 | test gpu (tier C, software adapter) | main | 1 |
| windows (WARP) A+B | check (fmt, clippy, layering) | main | 1 |
| linux (lavapipe) C | test gpu (tier C, software adapter) | main | 1 |
| windows (WARP) C 2/2 | Set up job | main | 1 |
| linux (lavapipe) | determinism manifest (GEN-003) | push | 1 |
| determinism (Windows vs Linux) | diff manifests | push | 1 |
| linux (lavapipe) | test (tiers A+B) | push | 1 |
| windows (WARP) | test gpu (tier C, software adapter) | main | 1 |

## 2d failed ci runs by class (a run counts once)
| where | class | failed runs |
|---|---|---|
| main | spec-lint | 3 |
| main | tests A+B | 8 |
| main | tier C goldens | 5 |
| pull_request | check (fmt/clippy/layering) | 23 |
| pull_request | spec-lint | 7 |
| pull_request | tests A+B | 6 |
| pull_request | tier C goldens | 45 |
| push | check (fmt/clippy/layering) | 49 |
| push | spec-lint | 6 |
| push | tests A+B | 10 |
| push | tier C goldens | 61 |

## failed PR-event runs per period
[('2026-10-10', 1), ('2026-10-11', 12), ('2026-10-12', 1), ('2026-10-15', 9), ('2026-10-16', 1), ('2026-10-17', 3), ('2026-10-18', 21), ('2026-10-19', 12), ('2026-10-20', 12), ('2026-10-21', 2), ('2026-10-22', 7)]
## CI job-hours in PR-event runs: all, then failed (job-hours, not wall time)
273.8425 16.06

## 3a revert / redo / fix-titled PRs
23 fix-ish titled 2 revert/redo
423 code rollback-reported: a rollback that fails is named; the editor refuses commands until the items are reloaded (V
470 code editor-test-gaps: set_line_params refusals, the part budget, hover and redo after a group delete, a brush over
## 3b fix within 24h touching same non-doc files as an earlier merged PR
12
#65 morph-height: geomorph the height, fix the relief grid precision band <- #61 m3-cache-keys: 128-bit persistent cache keys (ADR 0018, STRM (7.5 h, 1 shared files)
#165 upload-batching: one queue call per run of adjacent slots (REND-018; M <- #164 first-frame-warmup: no one-time initialisation inside a meas (0.6 h, 4 shared files)
#168 upload-time-variance: ranking sets, not writes, made upload_ms vary (M <- #167 dispatch-throughput: in flight follows the generation pool ( (1.4 h, 6 shared files)
#169 early-frame-gpu-wait: representative warm-up frame (REND-017; M4 fix F <- #168 upload-time-variance: ranking sets, not writes, made upload_ (0.4 h, 2 shared files)
#306 streaming smoke: run the dive until the streamer is alive (TEST-035 de <- #259 tier-b-perf-smoke: streamer and pool configuration-cap net ( (11.4 h, 2 shared files)
#415 path-replay: requirement is VIEW-026 (fixes the VIEW-025 duplicate on  <- #408 path-replay: cargo xtask repro frame-N-path.txt replays the  (0.6 h, 6 shared files)
#470 editor-test-gaps: set_line_params refusals, the part budget, hover and <- #465 layer-rows: the layers list has one row for a split line or  (1.2 h, 1 shared files)
#493 editor-ux-p2: value fields step, the wheel sets the brush, a click aga <- #492 region-cut-label: the editor says a mode-2 mask removes the  (0.1 h, 3 shared files)
#496 editing-light: H switches a sun fixed in camera space over the viewer' <- #493 editor-ux-p2: value fields step, the wheel sets the brush, a (0.5 h, 2 shared files)
#535 edit-health: the Problems tab lists steep, stacked and capped edits wi <- #531 ground-gloss: GGX highlight on the detailed ground, F6 cycle (4.6 h, 2 shared files)
#546 adr-chains: stale pointers fixed, known limits added, four readings AD <- #542 spec-style-lint: long or first-person requirement statements (0.8 h, 1 shared files)
#555 editor-smoke-flake: the smoke script's drag is one step (VIEW-022) <- #553 edit-frame-scale: the editor shows at most 400 edits; the ed (1.4 h, 1 shared files)
## 3b2 code PRs merged within 24 h of an earlier code PR sharing a non-doc file: count, then code PRs (coupling is high: shared lib.rs and spec files)
296 313
## 3c closed unmerged PRs
559 change/atmo-top-continuity 2026-10-22 atmo-top-continuity: no colour step across the top of the atmosphere (ATMO-023, 
46 change/m3-plan-cache 2026-10-11 m3-plan-cache: warm per-tile cache makes planning fit a frame (stacked on #43)
45 change/m3-cache-adrs 2026-10-11 ADR 0017 and 0018: persistent cache keys and tile storage (proposed, docs only)
44 change/m3-gen-context 2026-10-11 m3-gen-context: WorldDefinition, read-recording GenContext, keys from the read s
36 change/m2-steep-terrain 2026-10-10 m2-steep-terrain: steep stress field (TEST-015); finding: camera below steep ter
3 change/m1-generator-first-tile 2026-10-10 m1-generator-first-tile: CPU height generator, dumps, metamorphic tests
2 change/m1-coordinates 2026-10-10 m1-coordinates: frames, reference surface, cube-sphere tiles, hashing
## 1d lane via log, per period (code PRs on change/)
| period | sonnet-explorer | sonnet-crusader | sonnet-spikes | sonnet-m4 | sonnet-coder | unattributed | other |
|---|---|---|---|---|---|---|---|
| M4 (to 10-16) | 7 | 3 | 0 | 0 | 0 | 180 | 0 |
| M5 (10-17..18) | 6 | 24 | 0 | 16 | 0 | 22 | 0 |
| M6 (10-19..21) | 14 | 31 | 2 | 0 | 0 | 90 | 0 |
| M7 (10-22..) | 1 | 4 | 0 | 0 | 0 | 21 | 0 |

## 5a lines changed by period (merged PRs; additions+deletions, binary excluded)
| period | docs lines | code lines (src, shaders, xtask) | test-dir lines | docs : (code+tests) |
|---|---|---|---|---|
| M4 (to 10-16) | 21853 | 56926 | 29562 | 0.25 |
| M5 (10-17..18) | 37326 | 21667 | 11977 | 1.11 |
| M6 (10-19..21) | 44492 | 32408 | 23418 | 0.80 |
| M7 (10-22..) | 15200 | 5135 | 1852 | 2.18 |
| all | 118871 | 116136 | 66809 | 0.65 |

## 5b share of docs-only PRs
| period | PRs | docs-only (incl spike) | share |
|---|---|---|---|
| M4 (to 10-16) | 200 | 81 | 40% |
| M5 (10-17..18) | 130 | 68 | 52% |
| M6 (10-19..21) | 195 | 85 | 44% |
| M7 (10-22..) | 42 | 20 | 48% |
| all | 567 | 254 | 45% |

inside code PRs: {'docs': 35091, 'code': 116136, 'tests': 66809, 'golden': 0}
most touched docs files [('docs/status.md', 250), ('docs/log.md', 133), ('docs/learnings.md', 100), ('openspec/specs/testing/spec.md', 84), ('openspec/specs/streaming-cache/spec.md', 56), ('docs/adr/README.md', 53), ('openspec/specs/renderer/spec.md', 48), ('openspec/specs/generator-pipeline/spec.md', 48), ('openspec/specs/viewer/spec.md', 45), ('docs/spikes.md', 41)]
## 6c merge gaps (all merged PRs, UTC)
0-1 h: 513
1-3 h: 35
3-6 h: 12
6-12 h: 5
12-1000000000.0 h: 1
gaps >=6 h:
24.0 h 10-13 20:26 -> 10-14 20:25
9.3 h 10-12 19:54 -> 10-13 05:09
8.2 h 10-13 10:44 -> 10-13 18:58
7.2 h 10-20 21:27 -> 10-21 04:36
7.0 h 10-23 00:00 -> 10-23 07:03
7.0 h 10-11 21:04 -> 10-12 04:04
6.0 h 10-10 23:10 -> 10-11 05:09
5.0 h 10-11 05:19 -> 10-11 10:17
4.9 h 10-13 05:49 -> 10-13 10:44
4.7 h 10-10 11:23 -> 10-10 16:05
4.6 h 10-22 15:51 -> 10-22 20:30
4.5 h 10-18 23:44 -> 10-19 04:13
4.5 h 10-16 10:50 -> 10-16 15:18
4.3 h 10-12 12:20 -> 10-12 16:37
## gaps per prefix lane (change/ vs planner/): count of gaps >=3h
change 421 median gap h 0.37 p90 1.33 >=3h 19 >=6h 6
planner 93 median gap h 0.68 p90 4.65 >=3h 15 >=6h 7
## per-lane gaps (log-attributed change/ PRs)
sonnet-explorer 28 median gap h 1.33 p90 7.12 >=3h 7 >=6h 3
sonnet-crusader 62 median gap h 0.82 p90 7.96 >=3h 12 >=6h 8
## 4a review steps named in PR bodies (code PRs)
| period | code PRs | verifier | spec-guardian | numerics-reviewer | golden-review / bless | prepush | perf window / harness |
|---|---|---|---|---|---|---|---|
| M4 (to 10-16) | 119 | 78 (66%) | 65 (55%) | 35 (29%) | 16 (13%) | 0 (0%) | 18 (15%) |
| M5 (10-17..18) | 62 | 29 (47%) | 17 (27%) | 9 (15%) | 13 (21%) | 0 (0%) | 14 (23%) |
| M6 (10-19..21) | 110 | 18 (16%) | 26 (24%) | 33 (30%) | 5 (5%) | 64 (58%) | 23 (21%) |
| M7 (10-22..) | 22 | 0 (0%) | 1 (5%) | 2 (9%) | 1 (5%) | 1 (5%) | 1 (5%) |
| all | 313 | 125 (40%) | 109 (35%) | 79 (25%) | 35 (11%) | 65 (21%) | 56 (18%) |

## 4b PRs touching goldens
29 PRs touch golden files; by period Counter({'M4 (to 10-16)': 17, 'M5 (10-17..18)': 8, 'M6 (10-19..21)': 3, 'M7 (10-22..)': 1})
lead median h goldens 0.58 p90 7.31 | other code 0.28 0.72
golden/bless in title 28 [(566, "ground-look-2: land takes its materials' own albedo; lit gro"), (326, 'bless-apply: cargo xtask bless lists and promotes the owner-'), (323, 'golden-review-zoom: a tier-A test proves the PNG and report '), (322, 'golden-review-zoom: enlarged triptychs and crops of the diff'), (321, 'golden-review: summary per escalated group for /bless (owner'), (300, 'golden-review: enforce the declared bound tests for AUTO-E a'), (298, 'golden-review after M5: per-adapter verdicts, first goldens '), (297, 'ADR 0061 (owner): goldens after M5'), (287, 'docs: complete golden-review packet for #275 (21 candidates)'), (284, 'xtask: test gpu runs every test (--no-fail-fast): every cand'), (283, 'docs: golden-review of #275 in the M5 report and gate agenda'), (262, 'golden-review: look changes after the M5 review, AUTO-L (ADR')]
## 4c CI wall time to green per code PR (first ci PR run created -> last successful run updated)
| period | n | green run minutes median | p90 | first run -> last green min median | p90 | mean ci PR runs |
|---|---|---|---|---|---|---|
| M4 (to 10-16) | 118 | 7 | 12 | 9 | 24 | 1.64 |
| M5 (10-17..18) | 62 | 20 | 22 | 21 | 43 | 1.53 |
| M6 (10-19..21) | 110 | 14 | 21 | 18 | 52 | 1.80 |
| M7 (10-22..) | 22 | 14 | 21 | 16 | 44 | 1.68 |
| all | 312 | 13 | 21 | 15 | 43 | 1.68 |

## 4d perf windows (window-N.md first commit)
2026-10-17|perf window 7: results summary, console noise lines, log fragment (#251)
2026-10-18|docs: perf window 8 results (dive CRASH, descent-lag fix confirmed on dive-fast) (#264)
2026-10-18|docs: perf window 9 results (lit sessions invalid on orbit noise; accept M5 refused) (#277)
2026-10-18|docs: perf window 10 (#292)
2026-10-18|docs: perf window 11 (final M5 code regression check) (#317)
2026-10-19|Perf window 12: removal of the vertex render path (no verdict: CPU calibration) (#344)
2026-10-19|window13-scenes: the edit-latency perf scene, ManagedLoop::set_preview, the window 13 plan (#386)
2026-10-19|docs: perf window 13 report (preflight, part A, part B) (#399)
2026-10-20|docs: perf report for windows 16 and 17 (edit latency, the cover quadrants A/B) (#429)
2026-10-20|docs: perf window 18 (edit latency after ADR 0072) and the SPIKE-21 results (the wet floor of ADR 0
2026-10-20|perf window 19: edit latency (brush, regional, line, region) and the preview cost (#473)
2026-10-20|perf window 20: edit latency after the brush footprint (#475)
2026-10-20|atmosphere-camera-march: geometry above 8 km is hazed by a march from the camera (SPIKE-20) (#479)
2026-10-21|perf window 22: the plan job under ADR 0074 is over the +10 % bound; no #498/#499 regression (#507)
2026-10-21|perf window 23: plan job after #513 (dive +13 %, Ridge 3 tracked) (#514)
2026-10-22|perf-window-24: the ground detail is 2.4 to 2.9 ms over its 1.0 ms budget; ground views for perf li
2026-10-22|perf-window-25: the ground detail is +0.70/+0.90 ms after G2c, inside its 1.0 ms budget; no plan-jo
## 6a Spikes since M4 and what they changed (register text; the planner judges)

Closed spikes in the register since M4: SPIKE-08 to 34 (27). By the register's own outcome text:

| Outcome | Spikes |
|---|---|
| Diagnosis, cause named, fix landed as a change | 16, 17, 18, 19, 20, 22, 23, 25 (not reproduced), 26 |
| Recommendation that set or changed a decision or plan | 12 (retire the vertex format, owner decides), 21 (option C of ADR 0071), 24 (evidence of ADR 0073), 27 (procedural ground detail first), 28 (GPU culling path), 29 (edit scale: plan job is the bottleneck; led to `plan-cache-keep`), 30 (ADR 0076 open points), 34 (block size for ADR 0077) |
| Measurement or plumbing, no plan change recorded | 8, 9, 11, 13, 14, 15 |

## 6b Stop reports and gates

| Item | Count | Where |
|---|---|---|
| "stop report" in the log | 2 reports (M2, 2026-10-11, gate go not recorded; M4, 2026-10-15, waiting for a perf window), 3 mentions | `docs/log.md`, `docs/archive/M4/log.md` |
| Perf windows | 17 windows with a report, numbered 7 to 25 (14 and 15 have none; 16 and 17 share a file; dates 10-17 to 10-22); window 9 invalid (orbit noise), window 12 no verdict (CPU calibration), window 8 dive crash | `docs/perf/` |
| Learnings entries | 15 current, 1 (M7 archive), 71 (M6 archive), 19 (M4 archive) | `docs/learnings.md`, `docs/archive/` |

## 3d Corrected figures and withdrawn readings (log and learnings, by keyword)

| Date | Entry |
|---|---|
| 2026-10-15 | M4 queue item 0 text corrected (goldens and terrain paths used `cpu-ref-height-v2`) |
| 2026-10-17 | opus-planner withdrew its 32-px delta-1 reading of ADR 0052 (`adr-0052-as-written`); goldens RTX against WARP and llvmpipe differed in 36 to 3,448 pixels at delta 1 |
| 2026-10-19 | M5 process review: the `test fast` 24.1 s was a whole-workspace run, not the changed-crate run the 20 s budget is for |
| 2026-10-22 | learning: drafting `compact-slot-v2` and `render-floor-3` found five numbers that disagreed with the specs or the arithmetic |
| 2026-10-22 | `adr-chains`: stale ADR pointers and statuses fixed (also in this PR series: `housekeeping-mechanical`, 2026-10-16, and the M7 housekeeping pass, #571) |
| learnings | "assumption was wrong" entries: perf window 1 first frame (pipelines assumed lazy), review findings on m3-cell-index, m3-cache-keys, m3-cache-verify |

## Limits of this data

- Lane (explorer, crusader, spikes) is known for 108 of 421 `change/` PRs; the rest are only attributable by model.
- "Failed run" causes are proxies; no run failed and then passed on the same commit without a human rerun, so pure infra flakes show only as the 14 manual reruns (10 ended green, 4 red again).
- Review-step shares (4a) count mentions in PR bodies; the body template changed around M6, so the fall of "verifier" after M6 may be wording.
- The fix-within-24h list (3b) matches the word list in its heading and shared non-doc files, so it has false positives (a title that says "fix" in its content) and misses unlabelled fixes.
- PRs of M0 to M3 are mostly not in GitHub PR form; the M4 row includes the late M3 PRs.
