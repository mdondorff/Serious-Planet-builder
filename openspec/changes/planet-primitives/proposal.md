## Why
- Owner O2 (2026-11-23): planet primitives the port needs land on main during G3. They are additive and output-neutral, so they are built once and not again on the spike (G3 gate, lead #193).
- The G3 port order (seam and cube-face reconciliation, 2026-11-22, §5) starts with C1 (cells across cube faces) and S1 (the spill engine over seam pairs). Their output-neutral parts come first.

## What changes
- PR 1, COORD-011: `GlobalCell(u64)` (face, i, j at `n` cells per face edge; bits 60-62, 30-59, 0-29; bit 63 reserved). `fold_cell` becomes public. Neighbours cross cube edges by axis steps only (GEN-139). The id order (face, j, i) is the seam engine's tie key.
- PR 2, GEN-191: `bake::spill` over a caller-supplied list of tile pairs. Each pair carries its seam type (intra-face: 3 neighbours a border cell; cube edge: the axis neighbour only) and an opaque u64 tie key. It is bit-identical to today's spill levels on today's planar pairs.
- Not here: the quadrature metric's area change (it changes stored output; spike only), `CubePatch`, the breach bound (D1-3).

## Expected golden changes:
- none: additive code with no caller; hashes, goldens and the determinism manifest stay bit-identical.

## Impact
- Spec deltas: COORD-011 (PR 1), GEN-191 (PR 2).
- Crates: core (`global_cell.rs`; `tile.rs`'s `fold_cell` public), generators (`bake/spill.rs`, PR 2).
- Tests (tier A): `crates/core/tests/global_cell.rs` (PR 1).
- A new planet contract: numerics-reviewer on both PRs, and the lead's review before merge (routing §4).
