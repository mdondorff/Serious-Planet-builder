## 1. Gate columns and bars (BUILD-060)
- [ ] 1.1 Kept-basin interiors as the recorded-spill lake (`z < spill_m` joined to the floor); test
- [ ] 1.2 `bar_agree_<A>km2` on min(routed, graph); `bar_above_head_<A>km2`; test
- [ ] 1.3 Uphill and ponds at 1 km2 (`bar_uphill_1km2`, `bar_ponds_1km2`); test
- [ ] 1.4 Relief-indexed thresholds in place of `win == M1`; test
- [ ] 1.5 `bar_trunk_exceptions`; test
- [ ] 1.6 Hump cuts against the reference (S_KH, else S0); `bar_hump_cuts`; test
- [ ] 1.7 Corner-only links over 5 m against S0; `bar_corner_links`; test
- [ ] 1.8 Margin columns `m_*`; test
- [ ] 1.9 `bar_saddles_s0` in `--table` against B's row; test

## 2. Window draw (BUILD-066)
- [ ] 2.1 `cargo xtask windows-draw`: strata from the coarse field, seeded draw, no overlap with the five windows; test
- [ ] 2.2 Commit the drawn windows as `tests/data/g3-windows.toml` before any run on them

## 3. Rows and values
- [ ] 3.1 B and RG on the five windows; the new bars' values proposed in the PR body (O10)
- [ ] 3.2 numerics-reviewer, spec-guardian, verifier; archive, log fragment, PR (lead review)
