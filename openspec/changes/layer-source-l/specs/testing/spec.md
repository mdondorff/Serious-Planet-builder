## ADDED Requirements
### Requirement: TEST-111 The commit layer's tests
The commit layer SHALL be tested at tier A on a small baked face: the source returns the stored core with no fill and `B + L` equals K7's surface within half a quantisation step, a tile outside the commit is no land and a named tile the store lacks is an error, a tile is decoded once and the cache is bounded, and the drawn commit changes only when the tiles and tables of the new one are ready while a frame in flight keeps the previous one.

Verify: A
Status: planned
Source: STRM-083, architect 2026-11-10

#### Scenario: Cases
- **WHEN** the tests of `l_layer` run
- **THEN** each case above has a test that cites STRM-083
