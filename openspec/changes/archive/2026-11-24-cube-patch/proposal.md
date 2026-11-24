## Why
- Owner O2 (2026-11-23): planet primitives the port needs land on main during G3, additive and output-neutral (lead #195).
- The G3 port order (seam and cube-face reconciliation, 2026-11-22, §5) row 3 is C2: `CubePatch` as the only tile-plus-halo grid of the L stages (reconciliation row 7). Its rows 1 and 2 put TEST-139's partition layouts in data with a halo that GEN-182 allows.

## What changes
- COORD-012: `planet_core::CubePatch`, a square core of one face plus `halo` cells a side. Halo cells across a cube edge are the neighbour face's cells (axis-only, GEN-139); cells past a cube corner are stitched from the corner's other two faces; every patch cell is a `GlobalCell`. `fold_cell`'s body moves to a crate-private `fold_cell_axis` that `fold_cell` calls unchanged.
- TEST-151: TEST-139's layouts as a data file (one, two and four regions, a shifted seam, a seam on a face edge; tiles of 64 L cells, halo 24).
- Halo 24, not 16: GEN-182's lower bound is `DISC_RADIUS_MAX` 16 plus `PROFILE_CARRY_MIN` 8 and its upper bound `n / 8` = 24 at 192 L cells a face edge (lead, 2026-11-24).
- Not here: any stage reading `CubePatch` (the port), the quadrature metric, the propagation module, canonical jitter slots.

## Expected golden changes:
- none: additive code with no caller; hashes, goldens and the determinism manifest stay bit-identical.

## Impact
- Spec deltas: COORD-012 (coordinates), TEST-151 (testing).
- Crates: core (`cube_patch.rs`, `tile.rs`), generators tests (`g0_layouts.rs`, `support/g0_layouts.txt`, the fixture's layout parser).
- A new planet contract: numerics-reviewer, and the lead's review before merge.
