## 1. Height geomorph

- [x] 1.1 `MeshVertex::coarse_height`, `MESH_VERSION` 4, layout and known answers (REND-009, GEN-010)
- [x] 1.2 Shader mixes the height with the position's morph factor; `morph-height` debug view (REND-014)
- [x] 1.3 Tier A: coarse heights equal the parent grid's, morphed border height equals the coarser neighbour's (REND-009)
- [x] 1.4 Tier B: oracle for the shader's height morph, with a mutation check that it fails on an unmorphed height (REND-014)

## 2. Relief grid precision

- [x] 2.1 Per-decade f64 fractional phases, small line coordinates, far decades fade out (REND-006)
- [x] 2.2 Tier A: grid phase maths and ground-pixel resolution of every decade (REND-006)
- [x] 2.3 Tier C: thin lines only at the ground cameras; the detector flags the committed pre-fix image (REND-006)

## 3. Close-out

- [ ] 3.1 Golden review (terrain_aerial200km_height), numerics-reviewer, spec-guardian, verifier; relief candidates regenerated for the owner
- [ ] 3.2 Archive, status line, learning
