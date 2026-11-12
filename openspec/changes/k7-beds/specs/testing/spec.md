## ADDED Requirements
### Requirement: TEST-124 The K7 bed and fixed-channel tests
The K7 channel rules SHALL be tested at tier A: (a) a line crossing a low coastal plain (start -4 to +20 m) to its sea outlet has no land bed below the sea level and the profile holds (the test fails without the floor); (b) a burned trench in steep terrain keeps its depth through the whole pass (the test fails without the fixed cells); thermal and the deposition sweep leave fixed cells bit-unchanged, the volume thermal exports to the fixed cells equals what would have entered them in the free pass, the deposition ledger closes, and pit spreading leaves fixed cells alone and keeps the volume; (c) the final pass counts every path cell including a sub-sea one, the burn reports its own cutoff and an unbounded carry reports none, and K7 reports both; (d) the G-PROFILE tests of TEST-116 hold their properties (rise under 0.25 m along and across tile borders, no cutoff over 0.25 m, cutoff_binds 0 on the shallow fixture); their non-vacuity is restated (channel steps, tiles with a burned channel, steps across a tile border; the deep fixture holds holes farther upstream than the carry so a genuine cutoff rise exists and the final pass has work).

Verify: A
Status: planned
Source: GEN-176, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_k7_beds` and the G-PROFILE tests of `l_graph` run
- **THEN** each of (a) to (d) has an assertion that cites GEN-176
