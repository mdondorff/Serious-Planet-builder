# 0111 Hydrology model: H0 and H1, channel graph against diffuse field, basin and segment identity
Status: proposed (spikes session for opus-planner, 2026-11-08, inbox #100); the owner accepts after the external review and the D2-D10 pass
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §5; SPIKE-105 §6, §8 F3, §12 F5-F6; ADR 0041, 0099, 0101

## Context
- Rivers and basins exist only implicitly: a drainage-area byte (GEN-110), a fill-depth channel (ADR 0101). R4a and R4b need them as persistent objects.
- The carve already uses two models: jittered D8 receivers and MFD area (ADR 0099 point 2); they must not be conflated in the data model.
- A planet-wide route across cube edges costs about 7 s at 3.26 km with heights resident (F6); the largest catchment is 5.9e6 km^2.
- Uniform cell areas overstate drainage area by 18 % on average and change 7-14 % of channel cells (F5).

## Decision
1. H0: a planet-wide route at the coarse grid across cube edges gives regions, trunk connectivity and the level-1 preview. H0 does not cut erosion domains.
2. H1 (regional grid): three separate quantities. **Diffuse area** (MFD, true cell area and runoff weighted, stored as a byte) drives the carve and initiation. **The channel graph** (one D8 receiver per channel cell) gives connectivity: a channel continues downstream after initiation to its outlet. **Channel discharge** is accumulated along the D8 tree (the local term plus all donors), so it is conserved and the sum at a confluence is exact; it is not derived from the MFD value.
3. A channel starts at a configurable runoff-weighted true-area threshold (default 50-100 km^2).
4. Basins are objects with floor, spill level, spill point, active outlet, area curve, type and water level. Types: artefact (breached), lake, endorheic, intentional; class rules are preset parameters. The water level solves the steady balance `A(h) E_net = Q_in` on the area curve; a basin that reaches its spill level overflows through its spill point.
5. Ids are pure functions of the current surface: a basin by its canonical lowest cell, a segment by its end cells. Continuity across an edit is a separate, non-authoritative map computed at commit.
6. Lakes are producers of the water field (ADR 0041 point 7); the fill depth stops being a stored term.

## Consequences
- Gate G-CON, G-BASIN. Amends ADR 0101 points 2 and 3; extends ADR 0041.
- Rivers and lakes (later authoring) become constraints on K6 inputs, not new stages.
