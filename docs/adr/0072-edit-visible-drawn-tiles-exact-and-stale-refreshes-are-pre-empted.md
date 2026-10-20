# 0072 Edit visible: the drawn tiles are exact, drawn tiles refresh first, stale refreshes are pre-empted
Status: proposed (sonnet-explorer, 2026-10-20; the substance is the owner's decision of 2026-10-20 relayed by opus-planner, who accepts it). Reads the M6 exit criterion "edit visible" (SPIKE-13, SPIKE-14); extends STRM-026 (refresh of resident tiles).
Date: 2026-10-20
Source: owner (2026-10-20, `docs/status.md`); windows 13 to 16 (`docs/perf/`, `docs/perf/window-16-17.md`); STRM-026; TEST-041; ADR 0066 point 7

## Context
- The M6 criterion asks for the exact result within 300 ms (stamps up to 150 m) or 1 s (up to 500 m, or the camera at 1 m). Window 16 measured the time until every resident tile touched is refreshed ("all exact") at 371 to 1,290 ms, and the time until every tile the plan draws is refreshed ("drawn exact") at 93 to 956 ms: within budget in 8 of 9 cases (200 m altitude, 150 m stamp: 525 ms against 300).
- The streamer already refreshes a tile in the draw class first (rank 0 in its class), tells a running job to stop and drops its result, keeps one queued entry per tile, and re-requests a stale tile when nothing runs for it (STRM-026).
- It starts a new job as soon as the cancelled one has ended, so a quick sequence of edits (a drag) can start one job per edit, each stopped by the next.

## Decision
1. **Reading.** "Exact" means every tile the plan draws is exact: its resident version was made from the definition as it is now. The edit-latency scene's `drawn exact` column is the measure and the gated figure; `all exact` stays recorded. Tiles the plan does not draw refresh in the background and may take seconds. The budgets are unchanged.
2. **Drawn first.** A refresh of a tile in the draw class is requested and dispatched before every refresh of the residency and prefetch classes (the existing class order; a refresh keeps rank 0 inside its class). Nothing waits for the other classes to finish.
3. **Pre-emption.** Refresh work made stale by a newer edit is not computed:
   - a refresh still waiting (queued, or held by point 4) when a later edit re-keys its tile stays one entry and is counted as coalesced;
   - a job running for a tile that a later edit re-keys is told to stop (it stops at its next poll, at most 64 samples later) and is counted as cancelled;
   - a result that arrives for a re-keyed tile is dropped before it is made resident or uploaded, and counted as dropped.
4. **Settle.** A re-keyed tile's next refresh job is held until `refresh_settle_ms` of the injected clock have passed since the latest edit that re-keyed it, so a drag or a quick sequence starts one job after the last edit (plus the one that was already running at the first). The editor sets 40 ms; the default is 0, which holds nothing and leaves every other user of the streamer as it was. The preview (REND-033) shows the edit meanwhile.
5. **Reported.** The stream statistics and the edit-latency table carry the three counts per case.

## Consequences
- A single edit's exact result arrives up to 40 ms later (13 % of the 300 ms budget); a drag's does not start until the pointer rests for 40 ms.
- The 200 m, 150 m case (525 ms) is not made faster by this decision; it is re-measured in window 18 and decided on that number.
- Specs: STRM-030 (pre-emption, settle, counters, drawn first) and TEST-042 (the reading of the edit-latency criterion). Tests: tier A on the streamer with a controlled spawner (20 edits on one tile start at most 2 jobs; drawn tiles dispatch first; the counters), a managed-loop test (the editor setting reaches the streamer), the edit-latency table columns.
- Easy to reverse: the settle value; the counters.

## Licences of adopted code or techniques
None.
