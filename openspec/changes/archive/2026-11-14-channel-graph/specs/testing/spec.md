## ADDED Requirements
### Requirement: TEST-114 The channel graph's tests
The channel graph SHALL be tested at tier A: the codes (monotone, round trip, clamps, the stated error), the arrays against the routing's chains with the bed running minimum and the byte count, the cube-edge resolver on every face, the blocks through a store (every vertex in its block, the margin, the unit flag, refusals, the same bytes twice), and the bake that reads its channels back through the commit.

Verify: A
Status: planned
Source: GEN-170, STRM-084, architect 2026-11-11

#### Scenario: Cases
- **WHEN** the tests of `bake_channel_graph`, `graph_store` and `l_graph` run
- **THEN** each case above has a test that cites GEN-170 or STRM-084
