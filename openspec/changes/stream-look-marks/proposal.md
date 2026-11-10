# stream-look-marks (inbox #108 item 3; no requirement)

Architect 2026-11-10: the CI run 42780685812 failed `an_in_memory_run_is_labelled_not_the_production_source_...` (`look-2km` smoke) after the line `script built in 2.9 s`; the child hung for about 235 s with no further output.

## What (output only)
- The progress lines of a streaming perf smoke run (`world opened`, `script built`, `rig built`, `start-up load`, `settled at the start view`, `case ran`) end with ` [run +T s]`, the seconds since the run started (`stall::run_clock_start`, `stall::run_stamp`; the lit run of TEST-106 uses the same clock).
- The run marks its phases for `stall::phase_report()`: `stream run: start`, `world opened`, `<scene> script built`, `rig built`, `roots loaded`, `frame loop start`, in a smoke run each frame (`<scene> frame k of n`), `frame loop end`, `post-pass start`. A timeout panic of `gpu_stream_perf` therefore names the phase and the frame the run was in.
- Unchanged: the limits, the nextest timeout, what the tests check (the `look-2km` smoke test also asserts the stamps and the marks).
