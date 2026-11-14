## Why
- GEN-173 is active and says "No cell SHALL stand above its chessboard count". Main breaks it: `regrade_one` writes `base + epsilon * rank` with no cap.
- The cause (spikes/erosion-at-l/results/k7-flat-refusal.md, 2026-11-14):
  - a flat cell lifted by less than its count has its rank set by its own height (`own`);
  - its Euclidean descendants inherit that rank plus `FLAT_STEP`, although they are nearer the pour by count.
- Consequences:
  - E2's trace measured up to 9.8e-10 m of excess on R3a (1,782 cells, "cause not derived");
  - on the L-eroded surface the excess reaches 4.7 epsilon, and a cell beside the flat loses its lower neighbour, so K7's routing asserts (route.rs:236).
- Lead, 2026-11-14: a bug fix of an active rule; it does not wait for ADR 0123.

## What changes
- GEN-173: a graded level is at most the cell's chessboard level. Where that bound binds, the cell keeps its chessboard parent as a lower neighbour.
- New scenario: a flat with a hump.
- TEST-119 (f): a seeded rough low plain with a planted cell; it fails on main (18 cells up to 3.5 epsilon above their count) and passes with the cap.
- Kernel versions: `BAKE_KERNEL_VERSION` 7 to 8 and `PASS2_KERNEL_VERSION` 9 to 10 (the canonical flood is in both passes). Stores of the old versions are other layers.

## Closes
- Parking list: "GEN-173 bound: cells stand up to 9.8e-10 m above the ungraded flood (R3a round 3)". The excess is now 0 by construction (the lead removes the line).

## Expected golden changes
None expected: no golden scene's flats have `own` binding inside a graded flat. `cargo xtask test gpu` decides; any candidate goes through golden-review.

## Impact
generators (bake/flood.rs one line, bake/mod.rs versions), the bake_flats test, two spec deltas. Bits change only where a graded cell stood above its chessboard level.
