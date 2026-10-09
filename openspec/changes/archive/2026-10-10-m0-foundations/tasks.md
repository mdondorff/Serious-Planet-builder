## 1. Workspace and command surface

- [x] 1.1 Cargo workspace with the nine chain crates, `testkit`, `perf`, `xtask`; `.cargo` alias (BUILD-001, BUILD-002)
- [x] 1.2 `cargo xtask` commands: test (fast, all, gpu), check, layering, spec-lint, accept, perf, bless, repro stub (BUILD-002, BUILD-003)
- [x] 1.3 Layering and GPU-free dependency check (BUILD-001)
- [x] 1.4 Single binary with three run modes (BUILD-004)

## 2. Verification harness

- [x] 2.1 Test kit: PNG I/O, golden compare with candidates, image counts (TEST-002, TEST-003, TEST-007)
- [x] 2.2 Headless hello triangle with metrics and golden candidate (TEST-004, REND-007)
- [x] 2.3 Shader validation test, tier B (REND-002)
- [x] 2.4 spec-lint (TEST-001)
- [x] 2.5 Performance-harness skeleton: adapter refusal, statistics, clock logging, JSON (TEST-005, TEST-006)

## 3. CI and policy

- [x] 3.1 GitHub Actions workflow for Windows (WARP) and Linux (lavapipe) (BUILD-005)
- [x] 3.2 `deny.toml` licence policy (BUILD-006)

## 4. Decisions and specs

- [x] 4.1 Twelve decide-now ADRs as `proposed`
- [x] 4.2 Seed specs for the ten subsystems with trace IDs, tiers and statuses

## 5. Close-out

- [x] 5.1 `cargo xtask accept M0` green apart from CI, goldens and Rasierklinge items
- [x] 5.2 Verifier agent report with no failures
- [x] 5.3 Archive the change, log in `docs/status.md`
