# 0087 Perf verdicts under load, windows only for close calls
Status: proposed (drafted by the spike session for opus-planner, 2026-10-30; owner, 2026-10-30: Option B item B9)
Date: 2026-10-30
Source: SPIKE-58 phases 1 and 2 (`spikes/m9-perf-windows/results/results.md`); ADR 0020, ADR 0034, ADR 0036, ADR 0065; TEST-012, TEST-023, TEST-039; CON-20, CON-21

## Context
- M8 ran about 29 perf windows in 5 days, roughly 35-45 h of exclusive machine time; every other lane stopped meanwhile. No window was invalid for noise; the losses were exclusive time for verdicts with 2-30x margin, readings redefined after 8 windows, and fixed repeat counts.
- Measured on Rasierklinge (SPIKE-58 phase 2): no loaded or pinned reading was below the quiet minimum (30 readings). Quiet noise: 4.4 % (GPU timestamps), 2.7 % (CPU wall time).
- Full CPU load (a parallel workspace build) adds up to 18 % to GPU-timestamp readings (shared laptop power budget) and 3.6-5.6x to CPU wall time. With the load pinned to the E-cores at BelowNormal and the scene on the P-cores at High: +9-14 % GPU, +22-28 % CPU; the build takes 1.5x as long.
- The TEST-039 calibration ratio does not cancel load, and process CPU time is not load-immune (+29 % pinned, 2x loaded).
- The calibration gate refused 2 of 4 quiet preflights (single 55.5 ms over 52; parallel 129 ms over 112).

## Decision
1. **Loaded pass.** A time verdict measured without a window is a PASS when its reading is under budget by at least the loaded margin (5 %, the measured quiet noise; in the testing spec) and point 3 does not exclude it. The log marks it `loaded_pass` with the recorded load.
2. **Window queue.** A fail, a pass within the margin, or an excluded verdict goes to the window queue; a loaded fail is never a verdict.
3. **Exclusive verdicts.** A quiet window (ADR 0020 gate) stays for: frame time median and p99 with streaming on; cross-session regression comparisons of CPU wall time; the milestone acceptance run of criteria within the margin.
4. **Pinned runs for CPU scenes.** A CPU-timed scene runs loaded only with the competing builds and tests pinned to the E-cores at BelowNormal and the scene on the P-cores at High (an xtask mode); unpinned loaded CPU readings are not used.
5. **GPU budgets.** GPU-time verdicts use timestamp queries under the ADR 0036 clock floor, A/B in one process where a reference variant exists; they are loaded passes under point 1, not load-immune.
6. **Counts first.** A time requirement gets a tier B count companion (jobs, tiles, stand-in nodes on a fake clock) where the work can be counted.
7. **Batching, stop rules, wall time.** At most one planned window per day holds the queue; repeats stop early far from the budget (thresholds in the testing spec); the harness records start, end and active time per scene.

## Consequences
- Estimated cut of blocked time: 60-75 % on M8's pattern. Worth starting loaded: GPU verdicts with at least 20 % margin, CPU verdicts with at least 30 % pinned or 6x unpinned.
- ADR 0020's gate stays for point 3. Coder lanes may run perf scenes for loaded passes, which ADR 0020 reserved to the architect; this amends it in part. `--machine-ready` stays the owner's override; a loaded pass needs a mode that records the load instead of overriding the gate.
- Open: the calibration limits (ADR 0065) refuse a quiet machine about half the time; re-derive them or widen them before the next window. The CPU-timed scene after B6 (which removes `anet-chunk`) measures its noise once in its first window.
- A loaded pass can hide a regression that still fits the budget; the count companions and the window's acceptance run catch it.

## Licences of adopted code or techniques
None.
