## 1. Source
- [x] 1.1 `classes.rs`: plate mask, memberships, quantisation, lattice, `k`, hill-band amplitude
- [x] 1.2 `global.rs`: `pub(crate)` for what the source reads; `hill_octaves`
## 2. Preview
- [x] 2.1 `preview.rs`, the `preview_world` example, `cargo xtask preview world`
## 3. Checks
- [x] 3.1 Tier A: sums, thread independence, continuity, memberships, `k`, preview bytes (GEN-103)
- [x] 3.2 Tier B: the preview of seed 1 reports the shares (TEST-086)
- [ ] 3.3 numerics-reviewer, verifier, spec-guardian; archive
