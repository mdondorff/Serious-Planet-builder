## MODIFIED Requirements
### Requirement: BUILD-036 Tiers A and B run in three parts per operating system
The CI job that runs tiers A and B SHALL run as three jobs per operating system, `windows (WARP) A+B 1/3` to `3/3` and `linux (lavapipe) A+B 1/3` to `3/3`, each `cargo xtask test -- --partition hash:N/3`, so that every tier-A and tier-B test runs in exactly one part, a test's part depends on its name alone, and each job stays under the 16 min budget of BUILD-014. Part 2 SHALL also run the determinism manifest and its upload (GEN-003), check, spec-lint, the licence check and the run-mode starts (BUILD-005); parts 1 and 3 run only their tests. The artifacts of a part SHALL carry its number (`test-logs-<OS>-ab-N`, `review-<OS>-ab-N`).

Verify: A
Status: active
Source: opus-planner (2026-11-03: the Windows A+B job took 16.4 min in #1128's run, its test step 806 s of which 257 s build and 548 s tests; the critical path of a CI round after BUILD-018's archive; 2026-11-03, inbox #79: with `count:N/2` Windows part 1 took 16m20s against 14m08s, test steps 535 s against 209 s, #1134), BUILD-005, BUILD-014

#### Scenario: Three parts, the checks once in part 2
- **WHEN** the repository check test reads the workflow file
- **THEN** the A+B matrix holds parts 1 to 3 for Windows and for Linux, the test command takes `--partition hash:{part}/3`, the manifest, its upload, check, spec-lint, licences and the run-mode starts run in part 2 only, and the artifact names carry the part
