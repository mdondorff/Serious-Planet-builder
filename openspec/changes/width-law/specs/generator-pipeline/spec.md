## ADDED Requirements
### Requirement: GEN-165 One river width law
The width of a river SHALL be `w = a * Q_w^b` metres for a routed discharge `Q_w` in m3/s, one law for the burn-in and for the drawing, with `a` and `b` bake parameters in the key (standard `a = 7.2`, `b = 0.5`). K7's burn-in SHALL cut a trench of `max(1 cell, w)`: every cell whose centre lies within `w / 2` of the line, lowered to the line's level and never raised; a channel without discharges or narrower than a cell SHALL burn as the one-cell line, bit for bit.
- The width distribution of the channel cells of a region (p50, p95, maximum, the share wider than one cell) SHALL be reported.

Verify: A
Status: planned
Source: Architect D6 point 2, D19/D20 (inbox #117), item 15

#### Scenario: Trench widths
- **WHEN** a straight channel is burnt with `Q_w` of 100 m3/s into 10 m cells
- **THEN** the cells within 3.6 cells of the line are at the bed level, none farther is touched, and a smaller discharge burns a subset
