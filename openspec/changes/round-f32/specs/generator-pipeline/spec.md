## ADDED Requirements
### Requirement: GEN-166 The round surface is stored f32
When `CarveParams::round_f32` is set, the heights SHALL be narrowed to f32 (`f64::from(z as f32)`: round to nearest, ties to even) once at the end of every round and after the last round, and SHALL be read widened by the next round's routing, the K6 and K7 stages and every other reader. The round SHALL compute in f64; the breach belongs to the round and is not narrowed separately; a height that does not fit an f32 SHALL be refused. The narrowing SHALL be idempotent, and the whole-region carve, the round solver (any thread count) and the memoised solve SHALL give the same bits under the flag. The flag SHALL be part of every key that holds a round result (the layer key by a byte only when set, the catchment key by a bit only when set); with it off every result and key SHALL be the bits of the build before this requirement. The memoised solve SHALL refuse a run without the canonical flood, naming the reason.

Verify: A
Status: planned
Source: A3 section 9, architect 2026-11-10 (D24 row 8)

#### Scenario: Round solver and carve under the flag
- **WHEN** a region is solved by the whole-region carve and by the round solver on 1, 3 and 8 threads with the flag set
- **THEN** every height is f32-representable and the results are bit-identical
