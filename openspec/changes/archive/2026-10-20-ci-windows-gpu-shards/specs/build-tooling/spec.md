## ADDED Requirements

### Requirement: BUILD-018 Tier C on Windows runs in two halves
The CI job that runs tier C SHALL run its Windows leg as two jobs, `windows (WARP) C 1/2` and `windows (WARP) C 2/2`, each `cargo xtask test gpu -- --partition count:N/2`, so that every tier-C test runs in exactly one half and each half's job time stays under the 16 min budget of BUILD-014. The Linux leg SHALL run whole. The artifacts of a half SHALL carry its number (`review-Windows-N`, `test-logs-Windows-gpu-N`).

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-005, BUILD-014

#### Scenario: Two halves
- **WHEN** the repository check test reads the workflow file
- **THEN** the tier-C matrix holds a Windows entry for part 1 and one for part 2, the test command takes `--partition count:{part}/2`, and the artifact names carry the part
