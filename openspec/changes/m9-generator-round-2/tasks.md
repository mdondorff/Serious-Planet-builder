## 1. Continent stage
- [x] 1.1 `continents.rs`: H2r3, layout retry, smooth value; `global::terrain` reads it; `GLOBAL_VERSION` 6
- [x] 1.2 Island warp 0.3 (from 0.6) and amplitude 1.3, swept on seeds 1-5
## 2. Belts on the new coasts
- [x] 2.1 Plate draws scored by land next to a convergent margin; coast distances on the smooth value; coast gate; mountains' membership; `CLASS_SOURCE_VERSION` 4
## 3. Preview and checks
- [x] 3.1 S9, S10, S11, polar in `shape.rs` and the report; default seed 14
- [x] 3.2 TEST-087, TEST-088 (tier B); tier-A tests for determinism and the fingerprint
- [ ] 3.3 Re-pin the known answers of GLOBAL_VERSION 6
- [ ] 3.4 numerics-reviewer, verifier, spec-guardian
