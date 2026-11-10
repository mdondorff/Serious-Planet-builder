## ADDED Requirements
### Requirement: GEN-163 The lake producer
The bake SHALL turn each kept basin of H1 into one lake record: the water level of the steady balance of A3 section 5.5 (between the floor and the spill level, the spill level when the basin overflows), the class, whether it overflows, `q_in`, `q_out`, and the surface area at the level read from the area curve. A basin with no inflow SHALL have the floor as its level and no water surface. The river segments SHALL be produced without the channel cells inside kept basins: a segment that crosses a basin ends at the last cell before it and continues at the spill cell with `Q_w` carrying `q_out`. The result SHALL be a pure function of the hydrology in a fixed order (CON-14).
- Lakes are one per kept basin by `basin_id`; an artefact or a flat's ramp gives none.
- The producer adds no balance of its own; the level is H1's.

Verify: A
Status: planned
Source: Architect D6 (inbox #109), item 13 finding 2, briefing item 15

#### Scenario: Bowl with a river
- **WHEN** a bowl with a known sill and a channel through it is produced
- **THEN** the lake is full to the sill, no channel cell of any piece lies in the bowl, and a piece starts at the spill cell
