## Why
The M5 process review (#387) marked the CI failures of pull-request runs "n/m: `gh run list` no longer returns the M5 PR runs". That was wrong: the runs are retrievable (`gh run list --event pull_request --created 2026-10-16..2026-10-18 --limit 1000` returns 376 runs, 22 failed); the review had queried too few runs. A review needs the per-job times, the failure causes and the reruns of every run, and GitHub keeps them only for a limited time. Improvement 5 of the review.

## What Changes
- `cargo xtask ci-stats --record --since YYYY-MM-DD [--until YYYY-MM-DD]` reads every completed run created in that range (pull requests and pushes) with `gh` and appends one row per job that ran to a verdict (`success` or `failure`) to `docs/perf/ci-history.tsv`: run id, created, event, branch, job, conclusion, seconds, and the names of the failed steps (the failure cause). Runs already in the file are skipped.
- BUILD-020 and tier-A tests of the row format and the skip.
- The `housekeeping` skill's process review (step 9) records the milestone's range first and reads the CI rows from the file.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling.

## Out of scope
Recording from CI itself, a plot, retention of the file beyond the milestone archive (the housekeeping step moves it to `docs/archive/<M>/`).
