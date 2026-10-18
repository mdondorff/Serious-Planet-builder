## ADDED Requirements

### Requirement: BUILD-014 Process budgets are reported by the tools
`cargo xtask test fast` SHALL print, after every run whose build succeeded, its edit-to-first-test time (the wall time of the build step plus the nextest run) with the two parts, and SHALL print `BUDGET EXCEEDED: edit-to-first-test <s> > 30 s (...)` when the total is over 30 s (the owner's budget of 2026-10-18; exactly 30 s is within it): a warning, never a failure. `cargo xtask ci-stats [--runs N]` SHALL read the last N (default 20) completed runs on `main` with `gh` and print, for each CI job, the median and the p90 of its duration (nearest rank; jobs that ended `success` or `failure` count, skipped and cancelled jobs do not) and the number of runs, and SHALL print `BUDGET EXCEEDED: CI median of '<job>' <m> min > 16 min` for each job whose median is over 16 min (the owner's budget of 2026-10-18); it does not fail on it. The tools only report: an exceeded budget becomes a Tracked item in `docs/status.md` (the `housekeeping` skill says so).

Verify: A
Status: active
Source: owner (2026-10-18; #315), BUILD-003, BUILD-011, SPIKE-15

#### Scenario: Edit-to-first-test
- **WHEN** a `test fast` run builds for 12.0 s and tests for 9.5 s, and another builds for 49.0 s and tests for 13.2 s
- **THEN** the first prints `edit-to-first-test 21.5 s (build 12.0 s + tests 9.5 s; budget 30 s)` and the second `BUDGET EXCEEDED: edit-to-first-test 62 s > 30 s`; both runs keep their exit status

#### Scenario: CI median
- **WHEN** one job took 15, 15, 15, 20 and 30 min and another 14, 17, 18, 19 and 25 min over five runs
- **THEN** the first reports median 15.0 min and p90 30.0 min without a flag, and the second median 18.0 min with the `BUDGET EXCEEDED` line, and no other line is flagged
