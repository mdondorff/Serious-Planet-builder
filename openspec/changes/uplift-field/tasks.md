## 1. Field
- [ ] 1.1 `Provinces` gains `back_arc` (lattice term 7, `LITHOLOGY_VERSION` 2); lithology tests follow
- [ ] 1.2 `uplift.rs`: `Uplift::{new, u_at}`, table constants, `UPLIFT_VERSION`; tests: bits for 1 and 8 threads, formula at the terms, continuity across a face edge, range, craton under belt

## 2. Carve and keys
- [ ] 2.1 `carve` takes `uplift: Option<&[f32]>`; `face.rs` samples it; `None` reproduces today's bits (test)
- [ ] 2.2 `BakeInputs::id` folds the versions and the table; key test (the layer key differs with and without)

## 3. Preview
- [ ] 3.1 `uplift_preview.rs`, the option, the app test (files, label, 1 and 4 threads, refusals)
- [ ] 3.2 Sheet in `docs/dev_screenshots/`

## 4. Close
- [ ] 4.1 numerics-reviewer, verifier, spec-guardian, archive, prepush, PR
