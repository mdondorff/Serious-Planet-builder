## ADDED Requirements
### Requirement: GEN-163 The lake producer
The bake SHALL turn each kept basin of H1 into one lake record: the water level of the steady balance of A3 section 5.5 (between the floor and the spill level), the class, whether it overflows, `q_in`, `q_out`, and the surface area at the level read from the area curve. A basin with no inflow SHALL have the floor as its level and no water. The river segments SHALL be produced without the channel cells under water: the water of a lake is the cells of its basin and the cells draining into them with `z` under the level. The labelled cells of the basin downstream of the water carry no river. A segment that goes under water ends at the last cell before it and continues at the first cell out of it, with `Q_w` carrying `q_out` at an overflowing lake's spill cell. The result SHALL be a pure function of the hydrology and the surface in a fixed order (CON-14).
- Lakes are one per kept basin by `basin_id`; an artefact or a flat's ramp gives none.
- The producer adds no balance of its own; the level is H1's.

Verify: A
Status: planned
Source: Architect 2026-11-10, item 13 finding 2, briefing item 15

#### Scenario: Bowl with a river
- **WHEN** a bowl with a known sill and a channel through it is produced
- **THEN** the lake is full to the sill, no channel cell of any piece lies in the bowl, and a piece or a whole segment starts at the spill cell

#### Scenario: Gentle shore and endorheic lake
- **WHEN** a lake has a shelf 0.2 m under its level that H1 did not label, or an endorheic lake stands far below its sill
- **THEN** the river is not drawn over the shelf, and in the endorheic basin it is drawn down to the water's edge
