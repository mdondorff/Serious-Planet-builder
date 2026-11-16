## Why
- The river-graph plan's G0 (owner, 2026-11-16: approved) needs the minimum fixture of the G0 contract addendum section 2.7 before the graph contract's harness. Lead #154: terrain and expectations now; no graph types and no KS, KG, KV or KC code until the G0 contract is agreed.

## What changes
- TEST-139: a tier-A fixture on a small cube sphere (H1 814 m, L 101.75 m at a face's centre, 64 L cells a tile):
  - `B`, `z_b`, `z_s` and `S0`;
  - the seven features: trunk and two tributaries, an explicit confluence, an antecedent ridge crossing, a closed basin, a divide near a region boundary, a tile crossing and a cube-face crossing;
  - two partition layouts (one region; two regions split at the H1 column boundary just east of the divide);
  - an expectations file in plain tables.
- The test builds the fixture on 1, 2, 4 and 7 threads (same bits), checks the features with today's routing and flood, and compares the measured values with the recorded table.

## Impact
generators tests only (`tests/g0_fixture.rs`, `tests/support/g0_fixture.rs`, `tests/support/g0_expectations.md`); no product code.
