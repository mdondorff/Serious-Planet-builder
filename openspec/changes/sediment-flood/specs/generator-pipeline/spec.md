## ADDED Requirements
### Requirement: GEN-159 The sediment pass floods with the canonical flood
The sediment pass SHALL fill with the canonical flood (cells visited by level, then cell id; A3 section 6.4) in its pit statistics, its route and its deposition sweep, so that its depressions are those H1 labels.
- A flat chain of cells that drains through one outlet SHALL NOT lift a cell beside a lower outlet to the end of its epsilon ramp.
- The pit count of the sediment pass SHALL equal the count of cells deeper than the pit limit on the canonical filled surface.

Verify: A
Status: planned
Source: A3 section 6.4, inbox #111, #113

#### Scenario: a chain beside two outlets
- **WHEN** a chain of seven flat land cells lies between two sea cells and the cell next to the second sea cell is flooded
- **THEN** that cell stands one epsilon above the sea cell and the sediment pass reports no pit
