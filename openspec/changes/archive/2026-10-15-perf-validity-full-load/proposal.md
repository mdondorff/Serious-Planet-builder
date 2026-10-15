## Why

The first M4 perf window (2026-10-15) was INVALID by rule, not by the machine: every heavy view read throttle reason 0x4 (the laptop GPU's software power cap at 86-90 W, 52-55 C), and light views sat in P2 at 1,710 MHz and lost their GPU verdicts to the P0-only rule. The owner decided on 2026-10-15 (ADR 0036 points 1 and 2, accepted; point 3 stays proposed): the power cap is recorded, not invalidating, and a view is GPU-valid at a graphics clock of at least 1,400 MHz in every sample, whatever the P-state. Queue item 6e, before the perf scenes.

## What Changes

- perf `gate`: `is_throttling` no longer counts 0x4; thermal and hardware reasons and unparseable ones still invalidate. `has_power_cap` and `THROTTLE_SW_POWER_CAP` record it.
- perf `clock`: `GPU_CLOCK_FLOOR_MHZ = 1400.0` (ADR 0036). `gpu_validity` = every sample present and at or above the floor; the P-state is printed, not judged (`P0/P2` when samples disagree). `equal_clock_state` = clocks within 5 %; the report requires both views GPU-valid. The warm-up stops at the first reading at the floor (30 s cap unchanged). `LOADED_PSTATE` is removed.
- perf log: new per-run `power_cap`; `gpu_valid` and `gpu_clock_state` stay (the reason now names the lowest sample and the floor). Old logs are read through the same rule from their recorded samples (those without samples stay `no clock sample`).
- Console: `NOT VALID for GPU budgets (P2 1150 MHz, below the 1400 MHz floor)`, `[power cap 0x4 recorded, not invalidating]` after the clock state, the session summary line names the floor, the warm-up line names the floor.
- TEST-012 text and scenarios (delta): Power cap, Warm-up to the clock floor, View below the clock floor, View at the clock floor in any P-state, Equal clock state, Frame medians at any clock state.

## Not in this change

- ADR 0036 point 3 (a 0.1 ms noise floor) stays proposed and is not implemented.
- ADRs 0020 and 0034 are untouched. The P5-or-lower session warning (`session_warnings`) stays. Not run on the reference GPU.
