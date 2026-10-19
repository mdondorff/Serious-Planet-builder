## ADDED Requirements

### Requirement: BUILD-020 CI evidence is recorded per job
`cargo xtask ci-stats --record --since YYYY-MM-DD [--until YYYY-MM-DD]` SHALL read, with `gh`, every completed run created in the range (pull requests and pushes, every branch) and append one tab-separated row per job that ended `success` or `failure` to `docs/perf/ci-history.tsv`: the run id, the creation time, the event, the branch, the job name, the conclusion, the duration in seconds and the names of the job's failed steps joined by `+` (empty for a success). A run whose id is already in the file SHALL be skipped, so that a repeated call adds nothing. The file SHALL start with a header row. A missing `--since` SHALL be an error.

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-014

#### Scenario: Rows and the skip
- **WHEN** run 101 (a pull request on `change/x`) has the jobs `linux (lavapipe) A+B` (success, 300 s) and `windows (WARP) C 1/2` (failure, 60 s, failed step `test gpu (tier C, software adapter)`) and a skipped job, and the file already holds run 99
- **THEN** two rows for run 101 are appended, the failed one ending in `test gpu (tier C, software adapter)`, the skipped job has none, and recording run 101 and run 99 again adds no row
