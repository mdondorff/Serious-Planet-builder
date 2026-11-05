## 1. Field
- [x] 1.1 `Provinces` gains `back_arc` (lattice term 7, `LITHOLOGY_VERSION` 2); lithology tests follow
- [x] 1.2 `uplift.rs`: `Uplift::{new, at, of_terms, candidates}`, table constants, `UPLIFT_VERSION`; tests: bits for 1 and 8 threads, formula at the terms, continuity across a face edge, range, craton under belt

## 2. Carve and keys
- Not here: the explorer's plan PR 4 (with K) wires `uplift: Option<&[f32]>` into `carve`, `face.rs` and `BakeInputs::id`, as design decisions 3 and 4 describe; that PR's spec delta carries them (the uplift not scaled by the factor or K, the keys).

## 3. Preview
- [x] 3.1 `uplift_preview.rs`, the option, the app test (files, label, 1 and 4 threads, refusals)
- [x] 3.2 Sheet in `docs/dev_screenshots/`

## 4. Close
- [ ] 4.1 (ADR 0103; ) numerics-reviewer, verifier, spec-guardian, archive, prepush, PR
