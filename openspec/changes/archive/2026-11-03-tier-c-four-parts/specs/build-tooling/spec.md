## MODIFIED Requirements
### Requirement: BUILD-018 Tier C on Windows runs in four parts
The CI job that runs tier C SHALL run its Windows leg as four jobs, `windows (WARP) C 1/4` to `C 4/4`, each `cargo xtask test gpu -- --partition count:N/4`, so that every tier-C test runs in exactly one part and each part's job time stays under the 16 min budget of BUILD-014. The test groups (the serialised `warp-heavy` group among them) hold within each part. The Linux leg SHALL run whole. The artifacts of a part SHALL carry its number (`review-Windows-N`, `test-logs-Windows-gpu-N`).

Verify: A
Status: active
Source: opus-planner (2026-10-19; 2026-11-03, inbox #74: two halves took 21.2 and 20.6 min after #1103; 2026-11-03, option 2: with three parts one drew 878 s of serialised `warp-heavy` tests and ran 21-22 min, #1125), BUILD-005, BUILD-014

#### Scenario: Four parts
- **WHEN** the repository check test reads the workflow file
- **THEN** the tier-C matrix holds a Windows entry for parts 1 to 4, the test command takes `--partition count:{part}/4`, and the artifact names carry the part
