## ADDED Requirements
### Requirement: GEN-166 The round surface is stored f32
When `CarveParams::round_f32` is set, the heights SHALL be narrowed to f32 (`f64::from(z as f32)`: round to nearest, ties to even) once at the end of every round and after the last round, and SHALL be read widened by the next round's routing, the K6 and K7 stages and every other reader. The round SHALL compute in f64; the breach belongs to the round and is not narrowed separately; a height that does not fit an f32 SHALL be refused. The narrowing SHALL be idempotent, and the whole-region carve, the round solver (any thread count) and the memoised solve SHALL give the same bits under the flag. The flag SHALL be part of every key that holds a round result (the layer key by a byte only when set, the catchment key by a bit only when set); with it off every result and key SHALL be the bits of the build before this requirement. The memoised solve SHALL refuse a run without the canonical flood, naming the reason, and pass 2 and K7 SHALL refuse the flag. The flood's epsilon SHALL exceed the f32 spacing at the largest height of the region (9.77e-4 m up to 16,384 m); above it carved epsilon chains collapse to equal heights.

Verify: A
Status: planned
Source: A3 section 9, architect 2026-11-10 (D24 row 8)

#### Scenario: Round solver and carve under the flag
- **WHEN** a region is solved by the whole-region carve and by the round solver on 1, 3 and 8 threads with the flag set
- **THEN** every height is f32-representable and the results are bit-identical

### Requirement: GEN-178 The plan stores the round surface f32 until the sediment pass
Under `round_f32` the resident plan SHALL hold the heights after the rounds as f32 (exact: the surface is f32-representable) and drop the f64 copy until the sediment pass, which SHALL read them widened. The flood, the weighted route, H1, the river pieces, the channel graph and the plan's drivers SHALL read heights through a trait with an f32 and an f64 form; every result and key SHALL be the bits of the plan on f64 heights. With f64 rounds the plan SHALL keep f64 heights. The saving SHALL be at least 200 MiB of the peak commit at k = 4, or the step is dropped.

Verify: A
Status: planned
Source: Architect 2026-11-10 (GEN-166 row, the 200 MiB bar), inbox #125

#### Scenario: Same bits
- **WHEN** the plan runs with `round_f32` on a small compact cube
- **THEN** its H1 and sediment results equal the steps run by hand on f64 heights, and the allocation marks show the heights stored f32 after the rounds and widened before the sediment pass
