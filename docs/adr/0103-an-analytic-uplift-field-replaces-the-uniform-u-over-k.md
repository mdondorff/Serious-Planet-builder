# 0103 An analytic uplift field replaces the uniform U/K
Status: accepted (owner, 2026-11-05: "The other positions sound meaningful, please include those", the D3 review's item 1); amended 2026-11-05 by the independent review: the carve runs 20 steps, far from steady state, so the field is an elevation offset of at most about 84 m (total uplift equals U in metres), not a relief driver; lower K means less incision in the transient
Date: 2026-11-05
Source: owner, 2026-11-05; planner inbox #70 item 1; ADR 0088 point 4; ADR 0102; GEN-123; GEN-125; change `uplift-field`

## Context
- The face carve lifts every cell by the same ratio `U / K` (20, tapered at the coast): a craton rises as fast as an orogen.
- Relief that follows tectonics needs active belts and arc coasts high, the back-arc medium and the cratons near 0.
- ADR 0102 makes K a field; the class carve factor scales the uplift and K together, so a K field alone would leave the settled slope `U / (K sqrt(A))` unchanged by rock.

## Decision
1. The uplift is an analytic field `U(d)` (the ratio `U / K` at K 1): CPU-canonical, never stored, keyed by `UPLIFT_VERSION`, from the province lattice of ADR 0102 and a low-frequency noise (GEN-123).
2. The face carve reads `U(d)` in place of the uniform `u_over_k`; the coastal taper keeps multiplying it.
3. The uplift is applied separately from K and from the class carve factor: it is not scaled by either. This amends the wording of ADR 0102 point 3 ("in place of the class carve factor"): K and the factor scale erodibility only.
4. The field has a planet preview before any bake that reads it (GEN-125); its values are start values from that round.

## Consequences
- Extends ADR 0088 point 4 (with ADRs 0099, 0101 and 0102).
- Relief changes where the carve reads the field (the explorer's plan PR 4, with K); the class amplitudes and the scorecard are re-tuned on the result.
- Hard rock settles steeper and soft rock flatter at the same uplift.
- Every face layer key of a bake that reads the field is new; no stored data is rewritten.
