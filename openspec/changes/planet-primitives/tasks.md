## 1. GlobalCell and the fold (COORD-011, PR 1)
- [x] 1.1 `GlobalCell`, the public `fold_cell`, neighbours by axis steps across edges
- [x] 1.2 Tier A: 24 rotations, symmetry, corners, no cross-edge diagonal, the tie-key order, agreement with `TileId::neighbors8`
- [ ] 1.3 numerics-reviewer, verifier, PR (lead review)

## 2. The seam-pair spill engine (GEN-191, PR 2)
- [ ] 2.1 `bake::spill` over caller-supplied pairs with seam type and tie key
- [ ] 2.2 Tier A: today's planar pairs bit-identical; a cube-edge pair joins two faces' basins
- [ ] 2.3 numerics-reviewer, verifier, archive, PR (lead review)
