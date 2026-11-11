# Estimates: planned against actual

Planned and actual cost of every briefed item (owner, 2026-10-28: calibrate estimates like tokens; a habit, not a one-off). The planner adds a row when it briefs an item and fills the actual when it closes. Wall clock runs from the brief to the merge (or the report for unmerged work). Quota is the owner's reading when known.

## Calibration (current)
| Kind of item | Wall clock, lane |
|---|---|
| In-memory measurement or spike on the real store | 0.5 to 3 h |
| Literature study (Opus, Sonnet helpers) | under 1 h (quotes not re-checked) |
| One code change with tests, verifier and CI (one CI round about 20 min) | 0.5 to 3 h |
| Docs-only PR | 5 to 15 min |
Planning unit: lane-hours, PR count and quota share; never "days" for lane work. Calendar days only for owner-bound steps (tests, reviews, /bless, Friday decisions).

## Ledger
| Date | Item | Lane | Planned | Actual | Note |
|---|---|---|---|---|---|
| 2026-10-28 | SPIKE-51 uphill and held-back baselines | spikes | 1 day | 0.75 h | |
| 2026-10-28 | M8-G no-degradation gate (#844) | explorer | 1 to 2 days | 2.7 h | |
| 2026-10-28 | D0 decision measurement | crusader | 1 to 2 days (box 1.5) | about 2.7 h | |
| 2026-10-28 | Gate with the chunk stage off (#847) | explorer | half a day | about 0.5 h (report), 0.8 h (merge) | |
| 2026-10-28 | SPIKE-52 literature study (#846) | spikes (Opus) | 1.5 days | under 0.5 h after restart | quotes not re-checked |
| 2026-10-28 | M8-F step 1, stage off by default (#851) | explorer | not stated | 0.6 h | |
| 2026-10-28 | accept M8 to amended criteria (#850, #852) | crusader | not stated | 0.5 h | |
| 2026-10-28 | M8-F step 2, horizon maps from the base terrain (#853) | explorer | about 1 day | about 2 h to the PR | merged |
| 2026-10-28 | SPIKE-53 LOD, mesh and ground-detail survey | spikes (Opus) | 1 to 2 h | 1.5 h | |
| 2026-10-28 | Perf window 26 (criteria 4 to 7) | explorer | 1 to 1.5 h | 2.5 h wall | edit-latency 1.5 h; GPU cooldown retry |
| 2026-10-28 | Tier B stage-off horizon test (#860) | explorer | small | 1.3 h | |
| 2026-10-28 | Parallel horizon bakes (#862) | explorer | not stated | 2.2 h | |
| 2026-10-28 | Horizon march speed-up (rejected) | explorer | box 2 h | 1.2 h | no gain; cache-bound |
| 2026-10-28 | Envelope stage default, log path, chunk_stage (#861, #863, #864, #868) | crusader | not stated | 1.2 h + 1.2 h | prepush 11 to 15 min, CI 25 min per round |
| 2026-10-28 | Main build fix and distance column (#865, #866) | explorer | not stated | 1.5 h | #861 and #862 clashed on main |
| 2026-10-29 | Perf window 27 (criteria 6, 7) | explorer | 1 to 1.5 h | 4.8 h wall, 0.3 h active | edit-latency 5 repeats dominates |
| 2026-10-29 | SPIKE-54 G1 octave cost (#867) | spikes | box 2 h | 1.1 h | plus 6 h waiting for a quiet slot |
