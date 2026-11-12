## ADDED Requirements
### Requirement: TEST-120 The f32 round surface tests
The f32 round surface SHALL be tested at tier A: (a) the narrowing is the f32 value widened, idempotent, and refuses a height that does not fit; (b) on the near-degenerate fixtures of the P0 b list (flats, shallow pits, narrow channels, spill levels, low gradients) the whole-region carve and the round solver on 1, 3 and 8 threads are bit-identical under the flag, leave f32-representable heights, and every land cell drains; (c) the memoised solve under the flag is the fresh solve, and the same cache with the flag off gives the f64 solve; the catchment key changes with the flag; (d) the memoised solve refuses a run without the canonical flood; (e) the layer key changes with the flag; (f) pass 2 and K7 refuse the flag (their keys have no byte for it).

Verify: A
Status: planned
Source: GEN-166, architect 2026-11-10

#### Scenario: Cases
- **WHEN** the tests of `bake_round_f32`, `bake_rounds` and the layer store run
- **THEN** each of (a) to (e) has an assertion that cites GEN-166
