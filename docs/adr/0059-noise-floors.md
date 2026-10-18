# 0059 Noise floors: 0.1 ms per pass, 0.5 ms per frame total
Status: accepted (opus-planner, delegation, 2026-10-18; owner confirms at the M5 gate). Amends ADR 0036 point 3.
Date: 2026-10-18
Source: ADR 0020 (validity, 10 % noise), ADR 0036 point 3; perf window 9 (`perf lit` INVALID twice, `lit/orbit` noise 0.178 and 0.149; explorer's finding: the noise was the waited frame total, not a pass)

## Context
- ADR 0036 point 3 proposed an absolute floor of 0.1 ms beside the 10 % limit, for per-pass GPU figures.
- The window 9 orbit view has a frame total of about 1.5 ms. Its run medians were 1.708, 1.557 and 1.475 ms: (max - min) / median 0.149, an absolute spread of 0.23 ms. A floor of 0.1 ms would not have rescued it, and a 0.23 ms spread cannot move a verdict against a 20 ms p99 budget.

## Decision
1. The noise rule counts a spread as above the limit only when it exceeds both 10 % of the median and an absolute floor.
2. The floor is **0.5 ms for a waited frame total** (2.5 % of the 20 ms p99 budget) and **0.1 ms for a per-pass GPU figure** (point 3 as proposed). Both comparisons are strict; fewer than two runs or a non-positive median still counts as noisy.
3. The retry rule (one retry, then the session is invalid) is unchanged.

## Consequences
- Tiny frames and tiny passes with large relative noise stop invalidating sessions; a 10 ms frame with a 1.5 ms spread still does.
- The constants are `gate::NOISE_FLOOR_FRAME_MS` and `gate::NOISE_FLOOR_PASS_MS`; the spec's session-validity scenario states the numbers.
- Tests (tier A): the orbit case valid, a 10 ms frame with a 1.5 ms spread invalid, both boundaries, a tiny pass valid, a large pass invalid.

## Licences of adopted code or techniques
None.
