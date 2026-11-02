# 0090 Shadow march: order-independent atomics and a drop coefficient
Status: accepted (owner, 2026-11-01; drafted by opus-planner; amends ADR 0085 points 3 and 9)
Date: 2026-11-01
Source: shadow-clipmap-march PR 2a (#1014) and PR 3 (REND-049, REND-053); crusader's departure report, 2026-11-01; spec-guardian and numerics-reviewer on PR 3; CON-11, CON-14

## Context
- ADR 0085 point 9 forbids atomics in layer 1 so that results do not depend on dispatch order.
- The clipmap gather writes overlapping blocks from unordered dispatches. Without arbitration it raced on the software adapter: incremental and full rebuilds differed at random.
- ADR 0085 point 3 puts the sphere's drop into the gather (per-tile f64 coefficients) and keeps planet-radius terms out of the march.
- Rays that leave the clipmap continue on the coarse cube (point 4), which has no gathered tangent frame, so the drop must be applied in the march there.
- CON-11 allows presentation shading values of planet-radius magnitude with a tested bound; a coefficient 1/(2R) is not of that magnitude.

## Decision
1. **Owner pass atomic.** The gather's owner pass may use one integer `atomicMax` (the job id) to resolve overlapping blocks, followed by clear, claim and write passes. An integer max is order-independent, so point 9's goal holds. The march, upsample and temporal passes stay free of atomics (REND-053, tier A scan).
2. **Drop coefficient.** Beyond the clipmap (the cube phase) and for the horizon test, the march applies the drop as `d² · c` with `c = 1/(2R)` passed as an f32 uniform built on the CPU, and `d` camera-relative. No planet radius appears in WGSL. The f32 error bound at the march's 1,000 km cap is stated in REND-049 (values in the spec).

## Consequences
- ADR 0085 points 3 and 9 read with this amendment; their status line points here.
- Goldens stay per adapter (ADR 0061); no cross-machine exception is needed.
- Beyond 1,000 km the drop is first-order only; the spec caps the march there (numerics review of PR 3).

## Licences of adopted code or techniques
None.
