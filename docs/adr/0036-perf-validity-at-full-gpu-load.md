# 0036 Perf validity at full GPU load: power cap and clock band
Status: accepted in part (points 1 and 2: owner, 2026-10-15, opus-planner session; point 3 accepted and amended by 0059; point 2 amended by 0057 and 0060). Amends ADR 0020 (throttle rule) and ADR 0034 (P0 rule) in part.
Date: 2026-10-15
Source: first M4 perf window, 2026-10-15 (`docs/log.md`; main at d6f4564; `perf/logs/terrain.json`); ADR 0020; ADR 0034; constitution CON-21

## Context
Since the tight frustum cut, indirect multi-draw and frame pipelining, the terrain scene keeps the GPU busy (ground-1m frame median about 2.7 ms against 10.5 ms on 2026-10-13). The first M4 window ran twice (compact, then vertex for an A/B) and both sessions were INVALID by rule, not by the machine:
- every heavy view reported throttle reason `0x4` (software power cap) at 86-90 W and 52-55 °C: the laptop GPU's normal power governor at full load. ADR 0020 invalidates a session on any throttle reason;
- ADR 0034 makes a view GPU-valid only in P0, but the GPU read P2 at 1,710 MHz (faster than P0 readings of 1,365-1,470 MHz) on light views, so they lost GPU verdicts for no measurement reason (ADR 0034 point 3 foresaw this);
- aerial-20km's run-to-run noise was 0.186 at a 1.2-1.4 ms frame median, where a 0.25 ms spread is already 18 %.

## Decision
1. **Power cap is normal (owner, 2026-10-15).** Throttle reason `0x4` (SW power cap) is recorded per run and printed but does not invalidate the session. Thermal and hardware reasons (HW slowdown, HW thermal, SW thermal, HW power brake) still invalidate it; an unknown reason still invalidates it.
2. **Clock band instead of P0 (owner, 2026-10-15).** A view is GPU-valid when every clock sample of its final batch reads a graphics clock at or above a floor, whatever the P-state; the floor is **1,400 MHz** for the reference GPU, set from this window (loaded clocks 1,470-1,710 MHz; the coolest loaded start 1,365 MHz), recorded in the harness as a named constant with this ADR as its source. Equal clock state for comparisons (ADR 0034 point 5) becomes: both views GPU-valid and graphics clocks within 5 % of the larger; the P-state is printed, not compared. The warm-up loop (ADR 0034 point 3) runs until the floor is read, with the same 30 s cap.
3. **Proposed: an absolute floor for the noise rule.** A view's run-to-run noise counts as above the limit only when it exceeds both 10 % of the median and 0.1 ms. Decided with the next window's data (which views it would have rescued, and whether their verdicts were then stable).

## Consequences
- Sessions at full GPU load can be valid; the M4 exit-criteria verdicts (p99 frame time) and the GPU budget get verdicts again.
- The floor is specific to Rasierklinge's GPU; another reference machine needs its own measured floor (CON-21 names one reference machine).
- Tests (tier A): the throttle classification (0x4 recorded, thermal reasons invalid, unknown invalid), the clock band (P2 above the floor is valid, P0 below it is not), the comparison rule; the recorded fixtures of ADR 0034 are re-labelled, no assertion weakened.

## Licences of adopted code or techniques
None.
