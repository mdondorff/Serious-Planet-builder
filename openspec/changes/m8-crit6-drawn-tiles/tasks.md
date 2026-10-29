## 1. Tests first
- [ ] 1.1 accept_m8: criterion 6 passes with an over horizon row, passes without a horizon table, fails on an over M6 row, MISSING without the M6 table (TEST-060)
- [ ] 1.2 editperf: the horizon table has no verdict column (TEST-065)
- [ ] 1.3 gate: a late or missing re-bake passes and is named in the report (TEST-065)

## 2. Implement
- [ ] 2.1 accept_m8.rs, editperf.rs, gate.rs, xtask accept.rs text

## 3. Close
- [ ] 3.1 verifier, prepush, PR, merge; archive; log fragment
