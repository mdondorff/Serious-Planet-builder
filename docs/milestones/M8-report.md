# M8 Natural landscape: report (2026-10-30)

Drafted by opus-planner from `cargo xtask accept M8` (main 3ecfaee1, 2026-10-30 14:52 to 15:18, desktop unlocked), perf windows 21 to 29, the owner's live tests (2026-10-27, 2026-10-30) and the log (`cargo xtask log`). Criteria: `docs/briefings/m8-plan.md`, "Exit criteria", as amended 2026-10-28 (owner's fallback: hydrology and erosion out of M8) and 2026-10-29 (criterion 6 judged on the drawn tiles).

## Verdict: accepted (opus-planner, 2026-10-30)
- Criteria 1 to 7 pass (section 1). Not a go/no-go gate (CLAUDE.md: after M2 and M5 only).
- The one failing automated step is the local software tier C: 2 of 177 tests aborted with an access violation inside WARP's `d3d10warp.dll` (learning 2026-10-16); both passed alone twice; CI's WARP jobs are green on the latest code PRs (#912, #923), which the runbook names as the judge of that step.
- M8 shipped less than planned: the chunk stage (fine erosion and rivers per 5 km chunk) failed the D0 stop rule and is off by default since 2026-10-28; it is removed from the code after M8 (owner, 2026-10-29). Realism of hills and rivers moves to M9.

## 1. Exit criteria
| # | Criterion (as amended) | Result | Evidence |
|---|---|---|---|
| 1 | Owner's look review on plausible relief with horizon shadows; river and valley forms not judged | PASS (tier D) | owner, 2026-10-30: "plausible enough for M8"; the default look stays; the driver look is revisited with M9's driver maps |
| 2 | Identical values at chunk borders; CPU bit-exact across threads and operating systems | PASS | CI `determinism (Windows vs Linux)` and tiers A and B green on #912 and #923 |
| 3 | No frame waits: the gate on the real store with the chunk stage off: no frame over 100 ms, 0 held-back tiles without edits, no step between levels, no panic | PASS | `cargo xtask gate`, 12 sites: 0 panics, 0 held-back tiles, levels PASS; one frame of exactly 100 ms at site 41356 (KNOWN, Tracked); main-thread block decodes within VIEW-061's ceiling |
| 4 | Global coupled run at most 10 min | PASS | 19.7 s from the 16384 bake (window 21, `perf/logs/coupled-make.json`) |
| 5 | Shadows and material map at most 1.5 ms of terrain-pass GPU at 1440p | PASS | worst pose 300 m sunset 1.42 ms (window 25, `shadow-cost-river-r1-w25.json`) |
| 6 | An edit shows within the M6 budgets (drawn tiles); horizon table reported | PASS | window 28: 21 rows, none over; horizon maps 131 of 269 over 10 s on large edits, reported only (owner, 2026-10-29) |
| 7 | Steady speed at 300 m at least 200 m/s | PASS | 920 m/s (window 29, `target/perf-w29`, horizon on, chunk stage off) |
| Automated | check, spec-lint, tiers A and B, `gate --small`, run-mode smokes, `test fast`, `test gpu --real` | PASS | `test gpu --real` 239 of 239 with all goldens (10 RTX goldens blessed by the owner on driver 621.01, #915) |
| Automated | `test gpu` (software adapter, local) | judged by CI | 175 of 177; 2 WARP access violations, pass alone (see verdict) |

### Verifier (2026-10-30, from the recorded evidence; heavy tiers not re-run)
| Step | Result |
|---|---|
| Criteria 1 to 7 | PASS (criterion 3: one frame of exactly 100 ms at the limit, Tracked) |
| check, spec-lint, tiers A and B, `gate --small`, `test gpu --real` 239 of 239, smokes | pass |
| Software tier C (local) | FAIL, judged by CI (green on #912 and #923) |
| Remarks | the WARP learning says "never under nextest", which these aborts contradict (learning updated at housekeeping); the verifier read a status line from before the bless (#915 is merged: the run on 3ecfaee1 includes the blessed goldens) |

## 2. Also delivered
| Item | Ids or PRs |
|---|---|
| Global coupled run at 2048 from the 16384 bake, stored beside the bake | GEN-040, GEN-056, STRM-051 |
| Horizon-map shadows with a coarse far field (to be replaced, ADR 0085) | GEN-044, GEN-045, REND-038, STRM-048, STRM-060, STRM-064 |
| No-degradation gate on the real store and in CI | TEST-061 to TEST-063 |
| Drag preview fix: a stale tile takes the preview delta only at its own state | REND-045, #900 |
| Repro bundles replay from a stored bake | TEST-071, #912 |
| Level-pop detector for a moving descent | TEST-072, #923 |
| Retired terms fail `spec-lint`; glossary with term status | BUILD-028, #914, #919 |
| Compute budget shares as a spec | BUDG-001 to BUDG-007, #908 |
| Spikes: shadow architecture (ADR 0085, accepted), perf windows, H. Schott's publications, river channel A1/A2, declutter audit | SPIKE-56 to SPIKE-60 |

## 3. Carried into the next milestone
- The chunk stage's removal (B6, `openspec/changes/remove-chunk-stage/`); edits under erosion (D3) go to M9's deferred list.
- Horizon maps until ADR 0085's build; their edit latency is not worked on.
- Real-GPU window checks need an unlocked desktop (owner, 2026-10-30): NOT RUN when locked (TEST-073, in review).
- Tracked: drag residuals, editor `--site`, window resize, K view stall at orbit, CI WARP jobs over the 16 min budget.

## 4. Risks
- M9 realism (R7) is open-ended: SPIKE-55 measured hills and mountains 4 to 9 times too gentle; SPIKE-60 showed the stored river network does not follow the ground.
- Perf windows blocked the lanes for about a third of M8's calendar (SPIKE-58); B9 changes the process.
- The local WARP crash now also shows under nextest, after the Windows update of 2026-10-30.

## 5. Learnings
- A milestone criterion that depends on a new subsystem (chunks) needs a stop rule before the build: D0's stop rule ended the chunk stage after 6 days of integration work.
- A still-frame detector must assert content: two TEST-069 cases passed on black frames.
- Window presentation on the RTX stalls or loses the device while the session is locked; three days of device-loss reports had that cause, not load or the driver.
- A CPU latched at 17 % (firmware) inflated every timing of 2026-10-29; check `PercentProcessorPerformance` before a window.

## 6. Proposed ADRs awaiting the owner
- ADR 0076 (driver maps) and ADR 0084 (coarse valley carve): status reviewed at housekeeping.
- SPIKE-58's draft (perf windows that block less): to be filed after phase 2.

## 7. Recommendation
Option B for the away week (owner, 2026-10-30), then ADR 0085's build, the terrain scorecard and M9's bake (`docs/briefings/week-2026-11-02.md`).

## Process review
opus-planner, 2026-10-30, from recorded data; M7 is the baseline.

| Measure | M7 (2026-10-21 to 23) | M8 (2026-10-23 to 30) | Source |
|---|---|---|---|
| PRs merged | 77 | 339 | `gh pr list --search merged:` |
| PR lead time, median / p90 / max | 13 / not recorded / 223 min | 16 / 41 / 411 min | PR timestamps |
| CI slowest job median | `windows (WARP)` C 1/2 13.2 min | `windows (WARP)` C 1/2 18.8 min (budget 16) | `cargo xtask ci-stats` |
| Perf windows | 2 | about 34 (windows 1 to 29 and 4 to 5 more), about 35 to 45 h of exclusive machine time | SPIKE-58 |
| Rework | two | the chunk stage (6 days, removed); 8 criterion-6 windows re-read after ADR 0072 | log, SPIKE-58 |
| Coordination stalls | idle inboxes | merge races under the up-to-date rule (#912 behind three times); a merge refused by a session's permission check (#915) | this session |

Improvements, by gain against cost:
| # | Improvement | Session | Where |
|---|---|---|---|
| 1 | Merge slots handed out by the planner while the up-to-date rule is on | opus-planner | practice (2026-10-30) |
| 2 | B9: loaded passes with margin, GPU timestamps, at most one batched window a day | spikes, then coders | SPIKE-58 |
| 3 | Window checks NOT RUN on a locked desktop | crusader | TEST-073 |
| 4 | A stop rule in every criterion that depends on a new subsystem | opus-planner | M9 planning |
