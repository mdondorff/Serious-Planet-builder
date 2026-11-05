## 1. Design
- [x] 1.1 Design notes (this change), sent to the planner before any code
## 2. Field
- [x] 2.1 `orography.rs`: wind per season, the land-sea term, the Jacobi advection-rainout, the lattices, `OROGRAPHY_VERSION`; tests: bits for 1 and 8 threads, windward wet and lee dry on a synthetic ridge, ocean source and interior drying, face-edge continuity, seasonality range, `AI` formula
- [x] 2.2 `BakeInputs` holds it; `class_sample` takes the precipitation from it; `GLOBAL_VERSION` 7; the pinned known answers of the global bake and the store refusals follow
## 3. Preview
- [x] 3.1 `orography_preview.rs`, the option, the app test; the sheet in `docs/dev_screenshots/`
## 4. Close
- [ ] 4.1 numerics-reviewer, verifier, spec-guardian, archive, prepush, PR
