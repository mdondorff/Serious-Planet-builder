## 1. Keys
- [x] 1.1 `HeightField::body`, `Bound` carries the body, height key words, `KEY_FORMAT_VERSION`
- [x] 1.2 `body_params_hash` in world-def; mesh key and `MeshKeyer` use it
- [x] 1.3 Plan snapshots re-recorded and checked (hashes and mesh keys only); manifest lines

## 2. Tests
- [x] 2.1 Body alone changes height and mesh keys; planet equals the direct field; every parameter changes the body hash; batch keyer equals the single key; known-answer pin with the version

## 3. Close-out
- [x] 3.1 Spec GEN-012, status line, reviews, verifier, archive
