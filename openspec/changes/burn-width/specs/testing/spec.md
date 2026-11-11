## ADDED Requirements
### Requirement: TEST-110 The burn width tests
The burn SHALL be tested at tier A on a straight line (off and on a cell row), a diagonal and a curved channel at w = 0.5, 1.5, 2 and 3 cells: no gap on the line, no cell raised, the mean burnt width within the tolerance of GEN-168, and at least floor(max(1, w)) burnt cells in every column across a straight line.

Verify: A
Status: planned
Source: GEN-168, architect 2026-11-11

#### Scenario: Cases
- **WHEN** the tests of `bake_burn_width` run
- **THEN** each shape and width has an assertion that cites GEN-168

### Requirement: TEST-112 The monotone burn tests
The monotone burn SHALL be tested at tier A on a straight, a diagonal and a curved channel over a surface with holes under the profile: no cell of the line above its upstream neighbour after the burn, no cell raised, and the lowest hole carried down the channel.

Verify: A
Status: planned
Source: GEN-169, architect, 2026-11-11

#### Scenario: Cases
- **WHEN** the tests of `bake_burn_width` run
- **THEN** the monotone case fails on the burn without the running minimum and cites GEN-169

### Requirement: TEST-113 The partition and order tests of the profile
The burn and the final pass SHALL be tested at tier A for partition independence: the same channel tree through two overlapping windows (the same halo) gives bit-identical core cells, and the unbounded carry does not (the test has teeth); and for order: channels listed in any order are burnt in the tree order. The whole K7 under two tilings is reported (core cells differing, largest difference), not asserted: its general halo difference is A3's open tile-continuity item.

Verify: A
Status: planned
Source: GEN-169, architect, 2026-11-11

#### Scenario: Cases
- **WHEN** the tests of `bake_burn_width` run
- **THEN** the partition and order cases cite GEN-169
