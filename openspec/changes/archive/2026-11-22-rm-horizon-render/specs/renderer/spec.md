## MODIFIED Requirements

### Requirement: REND-044 The shadows and the material map cost at most 1.5 ms of the terrain pass
At 1440p at each pose of the pose stills (TEST-048), the material map (LOOK-008, LOOK-009) SHALL cost at most 1.5 ms of terrain-pass GPU time, measured by `cargo xtask perf stream shadow-cost` as the median over three round-robin rounds of the median of 41 waited frames of the full draw minus the same draw with the material map compiled out of the shader (`MeasureVariant::NoMaterial`, a measurement build; the product source is unchanged). The runtime-off draw is logged beside it; a pose within 20 % of the budget by it SHALL NOT be called met before the compiled-out draw is measured. The shadows have their own limit (REND-052).

Verify: A, C
Status: active
Source: owner (2026-10-23, D4), opus-planner (2026-10-27), M8 exit criterion 5, ADR 0085

#### Scenario: The verdict
- **WHEN** a pose's full draw takes 5 ms and the draw without the material map 3 ms
- **THEN** the pose fails the budget, and a pose at 1.3 ms is flagged as within 20 % of it

## REMOVED Requirements

### Requirement: REND-038 The horizon shadow term
**Reason**: ADR 0085 point 7: the horizon maps are retired; the shadow comes from the march (REND-049) and is applied in `sky-composite` (REND-051).
**Migration**: `rm-horizon-render` (PR 6 of `shadow-clipmap-march`); the shadow golden coverage is REND-051's.

### Requirement: REND-041 The frame's horizon data
**Reason**: ADR 0085 point 7; the frame's shadow data is the clipmap plan (REND-046).
**Migration**: `rm-horizon-render`; `frame::horizonview` is deleted.

### Requirement: REND-043 The coarse horizon fades out with distance
**Reason**: ADR 0085 point 4; the coarse cube (REND-048) holds max heights and needs no fade.
**Migration**: `rm-horizon-render`.
