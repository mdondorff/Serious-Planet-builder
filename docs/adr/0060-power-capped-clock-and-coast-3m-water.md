# 0060 A power-capped P0 clock is valid; coast-3m water budget 2.2 ms
Status: accepted (owner, 2026-10-18, opus-planner session; M5 gate decisions 11 and 12). Amends ADR 0036 point 2 and ADR 0057 point 2.
Date: 2026-10-18
Source: perf window 9 (`docs/perf/window-9.md`), `docs/perf/water-attribution.md`, M5 report section 5; ADR 0036 point 1 (the software power cap is normal)

## Context
- Under the coast views' load the GPU holds P0 at 1,245-1,365 MHz with the software power cap (`0x4`) as its only throttle reason, after the cool start (ADR 0057). That is the laptop's sustained clock, below the 1,400 MHz floor.
- ADR 0036 point 1 calls the power cap normal; the floor still made those views inconclusive whenever a figure was over budget.
- Coast-3m water (pass plus scene copy) read 2.175 ms at P0 1,290 MHz after the horizon cut (ATMO-020); the exact dry-tile cut (#286) predicts about 0.17 ms less. Its frame p99 is 6.6 ms.

## Decision
1. A view whose clock samples all read P0 and whose only throttle reason is the software power cap is valid for GPU figures, whatever its clock. Below the floor in any other state, ADR 0057 point 2 applies unchanged.
2. The water budget (pass plus scene copy) of the held view coast 3 m is 2.2 ms. The other held views keep 2 ms.
3. Comparisons between views (ADR 0034 point 5) keep their clock rule.

## Consequences
- Coast views get real verdicts: PASS or FAIL instead of inconclusive. `accept M5` re-judges logged windows without a new run (verdicts derive from logged data, #279).
- The coast-3m budget is a worst-case view's own number; a later cut that brings it under 2 ms does not change the budget without a new decision.
- The numbers live in the testing spec (the `lit` scene and `accept M5` requirements).
- Tests (tier A): P0 below the floor with only the power cap is valid; P0 below the floor with a thermal reason is not; P2 below the floor is not; coast 3 m at 2.2 ms passes and at 2.21 ms fails; another view at 2.1 ms fails.

## Licences of adopted code or techniques
None.
