## The reads
- **Runtime surface at a vertex:** `B(node) + R(L)`, where `R` is `monotone_read` over the window's 4 x 4 stencil (the window's edge clamped), the same read K11 makes inside a face.
- **Bake reference:** `C1(z)`, the bake's clamped Catmull-Rom of its own window. The runtime never reads `z`, and the bake defines heights only at the cell centres. Reading `z` with the nonlinear monotone read would measure the read's nonlinearity rather than the runtime.
- **Centres (the G-SURF gate):** every read returns the stored value, so the gate's numbers do not move.

## Option B, measured (DEC crusader-5)
Candidate reads of `L`, against the bake's `C1(z)` at the 225 vertices of `bake_k7`'s fixture tiles. Raw numbers: `target/review/surf-runtime/option-b.txt`, not committed.

| Read | Smooth: max / p99 / over 0.25 m | Channel: max / p99 / over 0.25 m | Steps at cell-centre lines |
|---|---|---|---|
| `B + monotone(L)`, the runtime today | 0.510 / 0.457 m / 16 | 2.949 / 2.012 m / 40 | none |
| `monotone(B + L)`, composed | 0.456 / 0.352 m / 7 | 2.938 / 1.990 m / 28 | none |
| `B + clamped Catmull-Rom(L)`, retired | 0.159 / 0.125 m / 0 | 0.277 / 0.125 m / 1 | value jumps of about 0.31 m and 1.64 m (the clamp switches cells) |
| `B + unclamped Catmull-Rom(L)` | 0.159 / 0.115 m / 0 | 0.125 / 0.115 m / 0 | none; overshoots its four cells by up to 0.075 m and 0.361 m |

- The reference is a Catmull-Rom read, so the Catmull-Rom candidates agree with it more by construction.
- The monotone reads flatten the slope at a trench's floor to 0 by design (no overshoot). That is the 2.9 m.
- Composing `B + L` does not help.
- The unclamped Catmull-Rom has neither the retired read's cell-centre steps nor the monotone flattening, at the cost of a bounded overshoot.
- The owner chooses the look. Any golden goes through golden-review and /bless.
