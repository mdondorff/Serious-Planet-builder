# 0111 Hydrology model: H0 and H1, channel graph against diffuse field, basin and segment identity
Status: accepted for the prototype (owner, 2026-11-11: "approve with the conditions mentioned", after the external review of 2026-11-11), with the conditions below; code still to follow point 7's cascade and open-water term. Production: G-CON, G-BASIN at the A/B gate
Date: 2026-11-08
Source: `docs/design/terrain-target.md` §5; SPIKE-105 §6, §8 F3, §12 F5-F6, §18, §19; ADR 0041, 0099, 0101

## Context
- Rivers and basins exist only implicitly: a drainage-area byte (GEN-110), a fill-depth channel (ADR 0101). R4a and R4b need them as persistent objects.
- The carve already uses two models: jittered D8 receivers and MFD area (ADR 0099 point 2); they must not be conflated in the data model.
- A planet-wide route across cube edges costs about 7 s at 3.26 km with heights resident (F6); the largest catchment is 5.9e6 km^2.
- Uniform cell areas overstate drainage area by 18 % on average and change 7-14 % of channel cells (F5).

## Decision
1. H0: a planet-wide route at the coarse grid across cube edges gives trunk connectivity and the level-1 preview. It constrains H1 in nothing: H1 determines drainage and channels from its own surface and regions from its own land mask (ADR 0112 point 1). H0 does not cut erosion domains.
2. H1 (regional grid), quantities kept apart (`a` true cell area; A3 §5.2): **geometric area** (sum of `a`, m^2); **effective area** `A_d` (MFD accumulation of `a w`, `w` the dimensionless runoff weight; m^2 of reference runoff), which drives the carve and initiation; **potential discharge** `Q` (m^3/s, the D8 accumulation of `a r` over every land cell, so hillslope runoff enters a channel at the first channel cell of its D8 path); **routed discharge** `Q_w` (`Q` after basin losses), which rivers report. `Q` and `Q_w` are discharges in m³/s; an area mode exists for tests only and every consumer refuses it.
3. `A_d` is an f64 of the round. The stored byte (a log code, A3 §5.2) is one-way, for materials; no simulation stage reads it.
4. **The channel graph** (one D8 receiver per channel cell) gives connectivity: a channel continues downstream after initiation to its outlet, also below an endorheic basin (with `Q_w` = 0). It starts at a calibrated preset threshold of effective area (default 50-100 km^2).
5. Basins are objects with floor, spill level, spill point (topographic), active outlet (a hydrological state), area curve, type and water level. Types: artefact (no object, no water), lake, endorheic, intentional; class rules are preset parameters, computed from the final round. The artefact limit is in cells and depth, so that it scales with the grid.
6. Basins are found on the filled surface of the round's routing; touching basins are one component (nested basins are not separated).
7. Balance: `Q_in = A(h) E_open + q_out(h)`, `q_out` zero below the spill level, `E_open = PET - (P - r)`. A basin that cannot balance its inflow overflows at its spill point. Basins resolve upstream to downstream, so `Q_in` reads `Q_w`; all cases are in A3 §5.5.
8. Ids are pure functions of the current surface: a basin by its canonical lowest cell, a segment by its end cells. Continuity across an edit is a separate, non-authoritative map computed at commit.
9. Lakes are producers of the water field (ADR 0041 point 7); the fill depth stops being a stored term.

## Consequences
- Gates G-CON, G-BASIN. Amends ADR 0101 points 2 and 3 and ADR 0105 points 2 and 3 (an endorheic basin by water balance, not by aridity; the fill-depth channel retired by point 9); extends ADR 0041.
- Rivers and lakes (later authoring) become constraints on K6 inputs, not new stages.
- Open: nested basins; basin losses in the carve's `A_d` (ADR 0105 point 2 is not in the round solver).

## Conditions (external review, 2026-11-11)
1. Three levels are kept apart: topographic drainage connectivity, potential channel topology, and active water-bearing segments. An endorheic basin ends active surface water, even where the potential graph continues (point 4).
2. Point 7's balance states the units and signs of `P`, `PET`, `r` and the outflow, and defines negative net evaporation, zero and high inflow, the spill threshold and nested basins (A3 §5.5).
3. Basin identity stays surface-derived and canonical (point 8); the continuity map stays non-authoritative.
