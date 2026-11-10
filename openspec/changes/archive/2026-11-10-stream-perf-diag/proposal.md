# stream-perf-diag (inbox #107; no requirement)

Architect 2026-11-10: the `gpu_stream_perf` lit-scene smoke hit its 240 s limit twice on CI (#1290 run 42747526646, #1295 run 42765844178) with an empty phase report and no times.

## What (output only)
- The progress lines of the lit perf run in smoke mode (`lit/<view>: N nodes (...)` and `lit/<flight>: C cameras x R timed frames in S s, ...`) end with ` [run +T s]`, the seconds since the run's renderer was made.
- The run marks its phases (`lit/<view> start|end`, `lit/<flight> flight start|end`, both shore filters) with `stall::phase_mark_owned`, so `planet::stall::phase_report()` in the timeout panic names the view or flight the run was in.
- Unchanged: `RUN_LIMIT`, the nextest timeout, what the test checks (it now also asserts the stamps and the marks).
