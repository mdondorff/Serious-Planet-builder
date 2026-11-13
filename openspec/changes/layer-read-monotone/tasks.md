## 1. GEN-188
- [x] 1.1 Tier A tests first: 20,000 random crossings of cell-centre lines with a jump of at most 1e-3 m; the read at a cell centre is the stored sample bit for bit (both failed on the old read: 1.93 m, 1.2e-13 m).
- [x] 1.2 The monotone C1 cubic per axis in `ErosionLayer::read_face`, the centre snap, `LAYER_READ_VERSION` 2.
- [x] 1.3 numerics-reviewer (2026-11-13: nothing blocking; its medium finding, a crease inside the cells from the harmonic-mean slope, fixed with a C1 limiter and a crease test; the production-geometry exactness case added).
- [ ] 1.4 verifier, prepush; golden-review for the lit goldens over a layer (declared change).
- [ ] 1.5 Archive with the spec delta synced; delete this file and design.md.
