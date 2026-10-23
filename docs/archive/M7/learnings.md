# Learnings moved at the M7 close (2026-10-23)

## 2026-10-21 Plan refusals at brush stacks were real steepness, not loose bounds
Scope: editor::slope, frame::lod (LOD-008), generators::stamp
- SPIKE-26: 44 raise strokes of 5 m made a 73 degree spike; the bounds were exact to 1 mm, the ground was steeper than the plan can morph (`LodParams::drawable_slope`, 2.5).
- Measure a bound against sampled heights before tightening it; guard steepness where the ground is made (the brush) and in the plan (ADR 0074).
- Obsolete when: the plan draws ground of any slope and the guard is a warning (M7 A5).
- Moved: A5 (VIEW-047) and ADR 0074 (capped tiles) are built; the rule lives in LOD-008 and VIEW-047.

