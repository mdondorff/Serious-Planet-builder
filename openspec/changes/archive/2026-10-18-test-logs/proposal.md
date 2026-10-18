## Why
Twice on 2026-10-18 a failing test's name or output was lost: xtask showed a summary, or the output scrolled away (opus-planner).

## What Changes
- `xtask::testlog`: `cargo xtask test`, `test fast`, `test gpu [--real]` tee the build step and the nextest run (stdout and stderr, in arrival order) into `target/test-logs/<command>-<UTC date>-<UTC time>.log`, keep the newest 20 per command, and print `full output: <path>` last, pass or fail.
- BUILD-012 added; tier-A tests on the naming (UTC, leap day, same-second suffix), the rotation (per command, other files untouched) and a failing child's stdout and stderr landing in the log.
- Learning entry.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling.

## Out of scope
`perf`, `accept`'s own steps, CI (CI keeps its artifacts), and a size cap beyond the 20 logs (a log of a `--real` run is a few hundred KB).
