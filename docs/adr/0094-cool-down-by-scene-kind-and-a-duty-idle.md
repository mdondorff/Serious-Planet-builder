# 0094 Cool-down by scene kind, and a duty idle between runs
Status: proposed (drafted by the spikes lane, 2026-11-02; a process decision for opus-planner under the owner's delegation); amends ADR 0057 point 1 and, for streaming scenes, ADR 0093 point 2
Date: 2026-11-02
Source: SPIKE-95 (`spikes/perf-window-process/results/results.md`); SPIKE-90 window 2; ADR 0057, ADR 0093; TEST-033, TEST-037; opus-planner, inbox #72

## Context
- ADR 0057 point 1 waits before every view, and TEST-037 before every streaming scene, until the GPU reads 60 C, for at most 120 s.
- SPIKE-90 window 2 lost 26 of 110 minutes to cool-downs that hit the cap on every run, and the runs after them still missed the CPU calibration: the GPU sensor did not follow the CPU's heat.
- Streaming scenes take no GPU rows; their verdicts are CPU wall time, guarded by the CPU calibration (ADR 0065, ADR 0093).
- Lit views' GPU figures are guarded by the clock floor's one-sided rule (ADR 0057 point 2, ADR 0060); a hot start shows there as a lower clock.
- Window 2's block 3 kept 45 s of idle before each run: 52-58 C, no cool-down waits.

## Decision
1. **Streaming scenes take no cool start.** Their GPU temperature at the start stays recorded in the log's clock sample; their calibration reading before the case follows the settle only (amends ADR 0093 point 2 for them).
2. **Lit and terrain views keep the cool start with a short cap** (value in TEST-033); a view that starts hot is judged by the clock floor.
3. **A duty idle between runs.** The A/B helper, the reserve test and `perf stream all` (and the planned window runner) idle for a share of the previous run's wall time, with a cap, before the next run (values in BUILD-035); not before the first run, and not between a refused try and its retry.
4. **Stop rule.** A streaming run that starts above the gate's temperature limit is named in the console (TEST-037); a device loss or a throttle reason other than the power cap in such a run brings back a capped cool start for streaming scenes, by a new ADR.

## Consequences
- TEST-033 and TEST-037 change and BUILD-035 is added (change `perf-duty-cycle`); TEST-039 and STRM-022 drop "the cool-down" for streaming scenes; the window runner (planned, BUILD-034) uses the same duty idle.
- On acceptance, ADR 0057's status line and its README row gain "point 1 amended by 0094" (ADR 0078).
- Window 8's device losses were the reason for the streaming cool start; `docs/status.md` attributes the device losses since 2026-10-18 most likely to a locked desktop. That is an attribution, not a measurement: the removal is unproven and is reversed in one place (`start_steps`); point 4 catches a return.
- To be measured in the planner's next quiet slot: 10 back-to-back streaming runs with the duty idle against 10 with the old cool-down, interleaved: valid share, tile-ready and frame p99 within the quiet noise.

## Licences of adopted code or techniques
None.
