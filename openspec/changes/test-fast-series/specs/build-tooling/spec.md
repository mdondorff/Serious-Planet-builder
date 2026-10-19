## ADDED Requirements

### Requirement: BUILD-019 Test fast keeps a series of its times
After every `cargo xtask test fast` run whose build succeeded, the command SHALL append one tab-separated line to `target/test-logs/test-fast-series.tsv`: the UTC time (`YYYY-MM-DDTHH:MM:SSZ`), the selection (`every crate` or the selected crate names joined by `+`), the build seconds, the test seconds, their sum (one decimal each) and `pass` or `fail`. A file that cannot be written SHALL be a warning, not a failure. The log rotation of BUILD-012 SHALL leave the file alone.

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-014, BUILD-012, BUILD-003

#### Scenario: One line per run
- **WHEN** a run of the crates `planet-frame` and `planet-render` builds for 12.0 s, tests for 9.5 s and passes at 2026-10-19 17:25:38 UTC, and a later run of every crate builds for 1.3 s, tests for 35.1 s and fails
- **THEN** the series file holds `2026-10-19T17:25:38Z`, `planet-frame+planet-render`, `12.0`, `9.5`, `21.5`, `pass` and then a line with `every crate`, `1.3`, `35.1`, `36.4`, `fail`, and rotating the logs of `test-fast` removes neither
