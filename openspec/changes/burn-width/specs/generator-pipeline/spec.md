## ADDED Requirements
### Requirement: GEN-168 The burnt channel matches the width law
`burn_channels_wide` SHALL lower every cell of a channel's 8-connected line to the channel's level there, SHALL raise no cell, and SHALL burn a trench whose mean width across the line (burnt cells per cell of arc length, the line's ends left out) is within 0.5 cell of max(1, w) on a line off the cell rows and columns, within 0.71 cell on a diagonal or curved line, and within 1.05 cells on a line on a cell row or column; at every column across an axis-parallel line at least floor(max(1, w)) cells SHALL be burnt.

Verify: A
Status: planned
Source: architect D41/D43

#### Scenario: Lattice limit
- **WHEN** a symmetric burn is centred on a cell row
- **THEN** its cell count is odd, so an integer w can differ by one cell from w; the 1.05 adds 0.05 for the line's ends
- **AND** the carve that follows widens the valley around the burnt channel; that is not a burn defect
