# 0057 GPU figures below the clock floor: a cool start, and a pass is a pass
Status: accepted (opus-planner, delegation, 2026-10-17; the owner confirms at the M5 gate, since it amends owner-accepted ADR 0036 point 2)
Date: 2026-10-17
Source: perf window 7 (`lit.json`: clocks, power and temperature of every run), ADR 0036 point 2, ADR 0034; spec TEST-033, TEST-034

## Context
- In window 7 the held-clock warm-up of ADR 0036 reached the 1,400 MHz floor on ground-1m, aerial-20km and the first orbit warm-up (54 to 59 C, 1,455 to 1,567 MHz) and not on the coast views, the later orbit runs and the flights (66 to 77 C, 1,282 to 1,380 MHz).
- Every run of every view read throttle reason `0x4` (software power cap) at 83 to 105 W. The cap trades clock for leakage: the clock follows the GPU temperature, which rose from 54 to 77 C over the sequential window, not the weight of the scene. A 30 s warm-up under the real load did not lift it; a dummy load would add heat.
- A lower clock can only make a pass take longer. A figure that meets its budget at a clock below the floor meets it at the floor.

## Decision
1. **A cool start.** Before the warm-up of every view the harness waits, idle, until the GPU reads 60 C or less (polling every 5 s, at most 120 s), and logs the temperatures and the time, or `NOT cooled`.
2. **One-sided rule (amends ADR 0036 point 2).** A view whose clock samples were all read and some are below the floor is **below the floor**. A GPU figure of such a view that is within its budget is a pass; one over its budget is inconclusive (not a failure, not a pass). A view with a missing or unreadable sample stays invalid. At or above the floor nothing changes. Comparisons between views (ADR 0034 point 5) still need both views at or above the floor.
3. The floor, 1,400 MHz, is unchanged. The M4 terrain scene keeps its console wording; the rule applies to the lit rows of `cargo xtask perf lit` and to `cargo xtask accept M5`.
4. The acceptance report keeps the clock state, the temperature range, the power and the power-cap reading beside the GPU rows of every view.

## Consequences
- A window can end with PASS rows at a low clock where it ended INCOMPLETE; a row over its budget at a low clock stays INCOMPLETE.
- Window time grows by up to 2 minutes per view when the GPU is hot.
- No budget changes. `nvidia-smi -lgc` (clock locking, admin) is not needed and not proposed.
- Tests (tier A): the cool-down with a fake sensor (target, cap, unreadable), the verdict of a below-floor view within and over its budgets, the unchanged verdicts at a valid clock, the words of the log read back into a below-floor state.
