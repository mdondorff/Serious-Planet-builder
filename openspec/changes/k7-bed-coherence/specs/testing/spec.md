## ADDED Requirements

### Requirement: TEST-134 The bed coherence tests
The burn levels and the committed beds SHALL be tested at tier A: (a) on a graph, the levels equal the running minimum down the tree of the H1 levels and the start surface at the rasterised line cells, do not rise and are idempotent; (b) on a committed region with a rugged base, no step of a stored line rises over 0.25 m in the stored ground read back through the commit, and the stored ground stands at most 0.75 m over a stored level on land; (c) on the same region, no tile's burn has a cutoff. (b) and (c) fail with the H1 levels alone.
- The fixture does not reproduce R3a's larger rises; the evidence on production geometry is the `k7_line_check` example on R3a, recorded in the change.

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E1, #125), GEN-185

#### Scenario: Cases
- **WHEN** the tests of `bake_channel_graph` and `k7_bed_coherence` run
- **THEN** each of (a) to (c) has an assertion that cites GEN-185 and TEST-134
