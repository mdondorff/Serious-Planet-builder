# 00NN Perf verdicts under load, windows only for close calls
Status: proposed (drafted by the spike session for opus-planner, 2026-10-30; owner, 2026-10-30: Option B item B9)
Date: 2026-10-30
Source: SPIKE-58 (`spikes/m9-perf-windows/results/results.md`); ADR 0020, ADR 0034, ADR 0036; TEST-012, TEST-023, TEST-039; CON-20, CON-21

## Context
- M8 ran about 29 perf windows in 5 days, roughly 35-45 h of exclusive machine time; every other lane stopped meanwhile (SPIKE-58 inventory).
- No M8 window was invalid for noise. The losses were exclusive time for verdicts with 2-30x margin, readings redefined after 8 windows, and fixed repeat counts (window 27: 4.8 h wall, 0.3 h active).
- Competing load only adds time to the measured work. A time budget that passes under load passes on a quiet machine; a fail or a near miss under load proves nothing.
- GPU timestamps with the clock floor (ADR 0036) do not depend on CPU load. The harness already records load during a case (TEST-023) and a CPU calibration kernel (TEST-039).

## Decision
1. **Loaded pass.** A time verdict measured without a window counts as PASS when it is under its budget by at least the loaded margin (20 %, in the testing spec) and the scene is not excluded by point 3. The log marks it `loaded_pass` with the recorded load.
2. **Window queue.** A fail, a pass within the margin, or an excluded verdict goes to the window queue; a loaded fail is never a verdict.
3. **Exclusive verdicts.** These need a quiet window as today (ADR 0020 gate): frame time median and p99 with streaming on; cross-session regression comparisons of CPU wall time; the milestone acceptance run of criteria within the margin.
4. **GPU budgets.** GPU-time verdicts use timestamp queries under the ADR 0036 clock floor, A/B in one process where a reference variant exists. They need no CPU-quiet window, only no other GPU user.
5. **Counts first.** A time requirement gets a tier B count companion (jobs, tiles, chunks, stand-in nodes on a fake clock) where the work can be counted, so regressions show in CI before a window.
6. **Batching and stop rules.** At most one planned window per day holds the queue. Repeats stop early for readings far from their budget (the thresholds live in the testing spec).
7. **Wall time logged.** The harness records start, end and active time of every scene.

## Consequences
- Estimated cut of blocked time: 60-75 % on M8's pattern (SPIKE-58), to be confirmed with phase 2's noise numbers. The loaded margin is set from them.
- ADR 0020's gate and validity rules stay for the exclusive verdicts of point 3. Coder lanes may run perf scenes for loaded passes, which ADR 0020 reserved to the architect; this amends it in part.
- The testing spec gains the loaded margin, the stop-rule thresholds and the count companions; TEST-012's report shows `loaded_pass` beside `ok`.
- A loaded pass can hide a regression that still fits the budget; the count companions and the window's acceptance run catch it.

## Licences of adopted code or techniques
None.
