## ADDED Requirements
### Requirement: GEN-183 Plateau uplift rises from the rim to the interior
When the plateau rim rule is on (`PlateauRim::w_km` above 0), the base elevation `e0` of a land point SHALL gain `U x (smoothstep(-W, W, x) - plateau_field)` metres, with `U` the plateau uplift of GEN-096, `W` the rule's width in km, `x` the signed distance in km from the point to the 0.5 contour of the plateau field (positive inside; a lattice of its own with the cap `W`) and `plateau_field` the field's own value, so that the plateau's uplift is `U x smoothstep(-W, W, x)`: 0.5 `U` on the contour, `U` from `W` inside, 0 from `W` outside, and never falling toward the interior. The change SHALL be faded in by `smoothstep(0, 0.2, land depth)` so that the shore has no step, and added after the class weights are decided, so that no class of the class source moves. With the rule off every `e0` and every key SHALL be the bits of the build before this requirement; with it on the inputs identity SHALL change with `W`.

Verify: A
Status: planned
Source: Architect, 2026-11-12 (the start-surface fix, plateau)

#### Scenario: A plateau interior
- **WHEN** the rule is on and a plateau's field stays at or above 0.5 inside its rim
- **THEN** the uplift at that point is the one of its distance to the rim, not lower than the uplift nearer the rim
- **AND** a dip of the field below 0.5 inside a plateau has a contour of its own and is a hole of the rule, not a bowl it removes; a feature narrower than one cell of the contour lattice is not resolved

#### Scenario: Details
- **WHEN** the rule is read
- **THEN** the uplift share is `smoothstep(-W, W, x)`, 0 when the rule is off
- **AND** the rule is a process-wide switch like the coast tilt (GEN-180): a run sets it once at its start
