## MODIFIED Requirements
### Requirement: BUILD-018 Tier C on Windows runs in four parts from one build
The CI job that runs tier C SHALL build the tier-C binaries once on Windows into a nextest archive (`windows (WARP) C build`, `test gpu --archive-out FILE`) and SHALL run them as four jobs, `windows (WARP) C 1/4` to `C 4/4`, each `test gpu --archive-file FILE -- --partition count:N/4` with no build of its own, so that every tier-C test runs in exactly one part and each part's job time stays under the 16 min budget of BUILD-014. The test groups (the serialised `warp-heavy` group among them) hold within each part. The Linux leg SHALL run whole. The artifacts of a part SHALL carry its number (`review-Windows-N`, `test-logs-Windows-gpu-N`).

Verify: A
Status: active
Source: opus-planner (2026-10-19; 2026-11-03, inbox #74: two halves took 21.2 and 20.6 min after #1103; 2026-11-03, options 2 and 4: with three parts one drew 878 s of serialised `warp-heavy` tests and ran 21-22 min, #1125, and each part spent 5-6 min building), BUILD-005, BUILD-014

#### Scenario: Four parts from one archive
- **WHEN** the repository check test reads the workflow file
- **THEN** the build job makes the archive and uploads it, the tier-C matrix holds a Windows entry for parts 1 to 4 that needs the build job and runs the archive with `--partition count:{part}/4`, and the artifact names carry the part

#### Scenario: The archive arguments
- **WHEN** `test gpu --archive-out FILE` and `--archive-file FILE` are given, alone, together, or with `test fast`
- **THEN** the first builds only the `gpu_all` binaries of `planet` and `planet-render` into FILE; both together and either with `test fast` are refused with their reason
