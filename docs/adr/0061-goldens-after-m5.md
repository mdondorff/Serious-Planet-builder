# 0061 Goldens after M5: the M5 window closes, goldens are per-adapter regression checks
Status: accepted (owner, 2026-10-18, opus-planner session; M5 gate decisions 3 and 4). Supersedes ADR 0037 point 1 and ADR 0052; amends ADR 0017 (cross-adapter agreement).
Date: 2026-10-18
Source: M5 report section 5; ADR 0017, ADR 0037, ADR 0052; learnings 2026-10-17 (RTX against software rounding)

## Context
- ADR 0037 point 1 blessed first M5 goldens under policy when their debug views agreed across WARP, llvmpipe and the RTX; ADR 0052 widened that agreement at edges. Both end at the M5 gate.
- Cross-adapter agreement cost two ADRs, three tool changes and a reversal in M5; hardware and software rasterisers differ at edges and in rounding by design.
- Correctness of shading and geometry is checked by the bound tests against the f64 references, not by agreement between adapters.

## Decision
1. The ADR 0037 window closes: `ADR_0037_M5_CLASSES` is emptied. A first golden of any class escalates to the owner's `/bless` (ADR 0017). Look changes after the M5 look review stay AUTO-L (ADR 0037 point 2, TEST-036).
2. Goldens are per-adapter regression checks. Agreement between adapters is no longer a condition to bless or a reason to escalate; each adapter's candidate is judged against its own golden under ADR 0017's other rules (rounding noise, declared change, driver gate).
3. A bless still needs at least one participating adapter with a candidate, the bound tests green in the same run, and the change declared.

## Consequences
- Fewer escalations from rasteriser differences; a defect that shows on one adapter only is still caught by that adapter's own golden.
- `golden-review` drops the cross-adapter comparison and the ADR 0052 constant; tier-A tests updated, none weakened beyond the removed rule.
- The specs holding the golden policy state the rule (ADR 0055).

## Licences of adopted code or techniques
None.
