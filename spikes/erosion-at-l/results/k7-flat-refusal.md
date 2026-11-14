# K7's canonical flood on the L-eroded surface: why it refuses, and the smallest fix

Order: lead, 2026-11-14 (P3 E2 groundwork, read only). Diagnostic: branch spike/k7-flat (local, never merged).

## Finding
- The refusal comes from GEN-173's grade, not from the flat test. The flat test (`lifted_flats`, bound 127, border) is correct.
- The grade lifts flat cells above their chessboard level, which breaks GEN-173's own rule ("No cell SHALL stand above its chessboard count").
- A non-lifted cell beside the flat, which drained into it, then has no lower neighbour, and `receiver_of` asserts (route.rs:236).

## Evidence
Spike bake of SPIKE-115 with the canonical flood in K7; K7 tile window 896 x 896; epsilon 1e-3 m.

| Cell | z (m) | chessboard (m) | graded (m) | graded - chessboard |
|---|---|---|---|---|
| 406261 (373, 453), not lifted | 0.030971 | 0.030971 | 0.030971 | 0 |
| neighbour 1, flat #950 | 0.001000 | 0.028 | 0.032676389 | +4.68 mm |
| neighbour 2, flat #950 | 0.001496 | 0.028 | 0.032676388 | +4.68 mm |
| neighbour 4, flat #950 | 0.001496 | 0.029 | 0.032676387 | +3.68 mm |
| other 5 neighbours, not lifted | 0.036 to 1.22 | same | same | 0 |

- Flat #950:
  - 570 cells;
  - Chebyshev diameter 95, under the bound, so it is graded;
  - base 0.001 m, the GEN-176 sea floor of the burnt beds;
  - deepest lift 0.040 m.
- Cells without a lower neighbour: 0 before the grade, 1 after it.
- The three graded levels differ by exactly `epsilon * FLAT_STEP` (1e-9 m). That is the signature of `regrade_one`'s `below + FLAT_STEP` chain (flood.rs:280).

## Mechanism
1. `regrade_one` sets `rank = max(d / sqrt 2, rank(parent) + FLAT_STEP, own)`. Its term `own = (z - base) / epsilon + FLAT_STEP` keeps a cell above its own ground.
2. On an eroded plain, a cell inside the flat can be lifted by much less than its count. `own` then binds, and the cell's rank sits close to its own height (here about 31.7 epsilon).
3. Its Euclidean descendants inherit that rank plus `FLAT_STEP`. These are the cells the Dijkstra reaches after it, farther from the rim.
4. The Euclidean order is not the chessboard order, so a descendant can be nearer the pour by count (27 epsilon) yet take the higher rank.
5. Nothing caps a rank at the chessboard level. It rises above the level that a neighbour outside the flat was routed against.
6. The fixtures with flat beds never let `own` bind inside a flat: SPIKE-108 and `bake_flats`. The low, nearly flat, sea-floored plains of the L-eroded surface do.

## Smallest fix (code, after ADR 0123 is accepted)
- The change is one line in `regrade_one` (flood.rs:299): each cell gets `min(base + epsilon * rank, chessboard level)`.
- Every cell keeps a strictly lower neighbour:
  - **cap not binding:** the cell's Euclidean parent stands lower, because its capped level is at most its own rank, which is below the cell's;
  - **cap binding:** the cell's chessboard parent stands at most at its own chessboard level, one epsilon below the cell's.
- No cell outside the flat sees a neighbour rise. Every routing made against the chessboard surface keeps its lower neighbour.
- The grade still depends only on the flat and its rim, because its input is the chessboard surface. The tiling rule holds.
- Each graded cell stays strictly above its ground, because both terms of the minimum are above `z`.
- Checked on the spike branch:
  - the generators' flood, flat and grade tests pass, 39 of 39;
  - they include `bake_flats` (the bound and the tiling) and `bake_k7` (thread independence);
  - the spike bake with the canonical flood in K7 bakes all 52 tiles of R3a (commit 434a18061744d173, 426 s), with no cell left without a lower neighbour.

## Spec delta (GEN-173)
The requirement already holds the cap. The delta makes the drain explicit and adds a scenario that falsifies the current code.

```
## MODIFIED Requirements
### Requirement: GEN-173 Flats are graded by distance to the pour points
(unchanged text up to "every cell kept above the neighbour it drains to.") A graded level SHALL be at most the cell's chessboard level; where that bound binds, the cell drains to its chessboard parent. (rest unchanged)

#### Scenario: A flat with a hump
- **WHEN** a flat holds a cell the fill lifted by less than its chessboard count, beside cells nearer the pour by count but farther from it by Euclidean distance, and a cell outside the flat drains into them
- **THEN** no graded cell stands above its chessboard level, and every cell of the window that is not an outlet has a lower neighbour
```

- **Test (tier A, in `bake_flats`):** a fixture built as in the scenario: a low plain at the sea floor, a hump cell lifted by less than its count, and a non-lifted rim cell draining in. It fails on main with the route.rs assert and passes with the cap.
- **Bit changes:** only grids where `own` binds inside a graded flat change bits. Existing fixtures and goldens are expected unchanged, and the implementation PR proves it with `cargo xtask test gpu`.
- **Workaround:** the spike bake's plain flood in K7 is no longer needed after the fix.
