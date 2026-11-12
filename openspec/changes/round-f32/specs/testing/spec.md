## ADDED Requirements
### Requirement: TEST-120 The f32 round surface tests
The f32 round surface SHALL be tested at tier A: (a) the narrowing is the f32 value widened, idempotent, and refuses a height that does not fit; (b) on the near-degenerate fixtures of the P0 b list (flats, shallow pits, narrow channels, spill levels, low gradients) the whole-region carve and the round solver on 1, 3 and 8 threads are bit-identical under the flag, leave f32-representable heights, and every land cell drains; (c) the memoised solve under the flag is the fresh solve, and the same cache with the flag off gives the f64 solve; the catchment key changes with the flag; (d) the memoised solve refuses a run without the canonical flood; (e) the layer key changes with the flag; (f) pass 2 and K7 refuse the flag (their keys have no byte for it).

Verify: A
Status: planned
Source: GEN-166, architect 2026-11-10

#### Scenario: Cases
- **WHEN** the tests of `bake_round_f32`, `bake_rounds` and the layer store run
- **THEN** each of (a) to (e) has an assertion that cites GEN-166

### Requirement: TEST-126 The f32 height storage tests
The f32 storage SHALL be tested at tier A: (a) the f32 and the f64 form of the height trait read the same bits, on slices, vectors and arrays; (b) the flood (plain and canonical), the weighted route, H1, the river pieces and the channel graph give the same bits on f32 and f64 heights; (c) the plan stores the heights f32 between the rounds and the sediment pass under `round_f32` (the allocation marks in order) and not with f64 rounds; (d) the plan equals the steps run by hand on f64 heights.

Verify: A
Status: planned
Source: GEN-178, architect 2026-11-10

#### Scenario: Cases
- **WHEN** the tests of `bake_heights` and the phase plan test of `bake_compact` run
- **THEN** each of (a) to (d) has an assertion that cites GEN-166 and GEN-178
