## Why
- The agreed G0 contracts (owner, 2026-11-16) set the fixture's second layout as two regions cut along a tile line through the divide (feature 5) and the tile-crossing reach (feature 6), and only a tributary reach is cut at a tile (`tile_crossing`). The fixture of #1561 split at an H1 column boundary inside a tile and let the trunk carry feature 6.

## What changes
- TEST-139: the two-region layout is whole tiles, cut along face 0's tile line at longitude -15. The divide lies on that line: both heads are at 490 m, a quarter degree either side, falling at the trunk's slope. Tributary T1 now starts at (-22, 28) and crosses the line, which is feature 6. The expectation file and the recorded values follow.

## Impact
generators tests only (`tests/g0_fixture.rs`, `tests/support/g0_fixture.rs`, `tests/support/g0_expectations.md`).
