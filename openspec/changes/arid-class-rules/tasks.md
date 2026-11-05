## 1. Design
- [x] 1.1 Design notes (this change), sent to the planner before any code
## 2. Rules
- [x] 2.1 `arid.rs`: the two rules, the blend, `ARID_RULES_VERSION`; tests: sums to 1, smooth (no step beyond GEN-097's limit over the lattice), badlands only on soft semi-arid broken ground, sand only on arid low supplied ground, deterministic
- [x] 2.2 `BakeInputs::weights_at` applies it; `id()` folds the version and the climate identity; key tests
## 3. Preview
- [x] 3.1 `arid_preview.rs`, the option, the app test; the sheet
## 4. Close
- [ ] 4.1 numerics-reviewer, verifier, spec-guardian, archive, prepush, PR
