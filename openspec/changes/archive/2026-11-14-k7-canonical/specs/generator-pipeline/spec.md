## ADDED Requirements
### Requirement: GEN-160 K7 floods with the canonical flood by a parameter
K7 SHALL, with `canonical_flood` on, flood with the canonical flood (cells visited by level, then cell id; A3 section 6.4) in its pit statistics, its incising carve and its deposition sweep, and with it off SHALL give the bits of the plain-flood K7.
- The default pass 2 (`deposit_on_own_route`, `carve` with its default rules) SHALL be bit for bit as before.
- K7 SHALL give the same bits on any thread count with either setting.
- The change of the stored `L` against the plain flood SHALL be reported (cells changed, maximum, share over 0.25 m).

Verify: A
Status: planned
Source: A3 section 6.4, inbox #114

#### Scenario: a tile with a chain beside two outlets
- **WHEN** the pit statistics and the deposition's surface are read on a flat chain between two sea cells, and K7 runs on a small tile with the plain and with the canonical flood
- **THEN** the plain statistics count the chain's lifted cells and the canonical ones none, the plain K7 equals the digest taken before this change, and the canonical K7 is the same bits on 1 and 2 threads
