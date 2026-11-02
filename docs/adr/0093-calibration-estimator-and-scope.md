# 0093 The CPU calibration: a robust estimator, read before the frames, voiding frame verdicts only
Status: accepted (opus-planner, 2026-11-02, under the owner's delegation: a process decision, inbox #72; drafted by the spikes lane); amends ADR 0065
Date: 2026-11-02
Source: SPIKE-95 (`spikes/perf-window-process/results/results.md`); SPIKE-90 windows 1 and 2; ADR 0065, ADR 0087; opus-planner, inbox #72

## Context
- In the three windows after ADR 0087, 53-75 % of perf attempts gave no verdict, all of them from the ADR 0065 calibration limits; on unloaded configurations 36 % (SPIKE-90 window 1).
- Each reading is one ~50 ms timing on an unpinned thread; the limits are the quiet maximum plus about 5 %, so one interrupt or one hot start voids a scene.
- The streaming scenes read "before" ahead of the settle and the GPU cool-down, on a CPU still hot from the previous run.
- A failed calibration voids GPU-timestamp verdicts too, which no CPU kernel's speed enters; the GPU's own guards are the clock floor and the noise rule (ADR 0036, ADR 0057).
- The reserve test's loads (BUDG-012) run during its calibration by design: 5 of 6 quiet reserve sessions got no verdict for the load under test.

## Decision
1. **Estimator.** The single reading is the smallest of several timings; the parallel reading is the repetition with the smallest median among several (counts in TEST-089). Interference only adds time to fixed work, so the minimum is the machine's state without transients, while a persistent slow state (ADR 0065's slow terminal) shows in every timing.
2. **Placement.** The reading before a case is taken after its settle, cool-down and warm-up, right before the first measured frame.
3. **Scope.** A calibration outside its limits voids the frame verdicts (frame, CPU and plan times, counts, RAM, slots, lag), not the GPU verdicts.
4. **Declared load.** A scene that starts its own load before its calibration declares it in the log; its readings are recorded, not a session cause, as in loaded mode (ADR 0087).
5. **Limits unchanged** until the first window measures the new estimator on the quiet machine and in the slow state; the values live in TEST-039.

## Consequences
- TEST-039 changes and TEST-089 is added (change `perf-calibration-robust`); `accept` judges old logs by the same scope, and a milestone acceptance still needs frame validity.
- A calibration takes a few tenths of a second longer.
- ADR 0087's open point (re-derive the 0065 limits for quiet sessions) is partly addressed by the estimator; the limits stay open.
- To be measured in the planner's next quiet slot: 20 quiet preflights with the old and new estimators, and the planner terminal's slow state still refused.

## Licences of adopted code or techniques
None.
