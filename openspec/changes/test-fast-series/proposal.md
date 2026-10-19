## Why
BUILD-014 prints the edit-to-first-test time of a `test fast` run and forgets it. The M5 process review (improvement 4) could not fill its edit-to-first-test row: no series exists, and the whole-workspace figure (reported only) was mistaken for a breach of the 20 s budget, which is for the changed crates (BUILD-003). Selection times are the figure the budget is about.

## What Changes
- Every `cargo xtask test fast` run whose build succeeded appends one line to `target/test-logs/test-fast-series.tsv`: UTC time, the selection (`every crate` or the crate names), build seconds, test seconds, total seconds, `pass` or `fail`.
- BUILD-019 and tier-A tests of the line format, the append, and the log rotation leaving the file alone.
- The file is local (under `target/`); a review reads it with the process-review table of a milestone report.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling.

## Out of scope
A tracked history in git, a plot, failing on a budget, CI timings (item 5 of the process review).
