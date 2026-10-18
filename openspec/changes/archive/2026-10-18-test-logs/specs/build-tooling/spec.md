## ADDED Requirements

### Requirement: BUILD-012 Every test run keeps its full output
Every `cargo xtask test`, `test fast` and `test gpu [--real]` run SHALL write the full output of its build step and of its nextest run (stdout and stderr, line by line in arrival order, including the stdout and stderr of every failing test and the run summary) to `target/test-logs/<command>-<YYYYMMDD>-<HHMMSS>.log`, where `<command>` is `test`, `test-fast`, `test-gpu` or `test-gpu-real` and the time is UTC (a second run in the same second gets `-2`, `-3`, ...). The command SHALL print `full output: <path>` as its last line, whether the run passed or failed, also when the build step failed. It SHALL keep the newest 20 logs of each command and delete older ones of that command (a log of another command, and any other file in the folder, is left alone). The console output is unchanged apart from the colour nextest drops when its output is not a terminal.

Verify: A
Status: active
Source: opus-planner (2026-10-18), BUILD-003

#### Scenario: A failing run
- **WHEN** a test of a `test fast` run fails
- **THEN** the log holds the build lines, the failing test's name and its output and the summary, the console shows the same, and the last line is `full output: target/test-logs/test-fast-<date>-<time>.log`

#### Scenario: Rotation
- **WHEN** 25 logs of `test` and 3 of `test-gpu` exist and another `test` run starts
- **THEN** the newest 20 of `test` remain (the new one among them), the 3 of `test-gpu` and `test-gpu-real` are untouched, and other files stay

#### Scenario: Naming
- **WHEN** a `test gpu --real` run starts at 2026-10-18 20:30:15 UTC
- **THEN** its log is `target/test-logs/test-gpu-real-20261018-203015.log`, and `test` is told apart from `test-gpu-...` by the date and time after the command
