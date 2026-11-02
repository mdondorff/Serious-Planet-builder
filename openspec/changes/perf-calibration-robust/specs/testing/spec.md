## MODIFIED Requirements
### Requirement: TEST-039 CPU calibration decides whether a perf scene counts
A perf scene SHALL record the CPU calibration of STRM-022 (a fixed kernel timed alone and on the pool's threads, `load_calib_<before|after>_single_us`, `..._parallel_median_us` and `..._threads`) before and after its case, in the streaming scenes and in the lit views and flights alike (not in a smoke run, unless `--calibrate` asks for it so that the logging can be proved on any adapter). The reading before the case SHALL be taken after the scene's settle, cool-down and warm-up, right before its first measured frame. The scene's frame verdicts (frame time, CPU times, plan time, counts, RAM, slots, lag) count only if every reading logged is within its limit: before single at most 52,000 us, before parallel median at most 112,000 us, after single at most 56,000 us, after parallel median at most 112,000 us (ADR 0065, ADR 0093); a reading equal to its limit is within. The parallel limits hold for 18 threads, the reference machine's generation pool: a log with another or no thread count is judged on its single readings alone. A scene with any reading over its limit SHALL get no frame verdict: a session cause `no verdict: CPU calibration of '<scene>' outside the limits (ADR 0065): <when> <kind> <reading> us over <limit> us` (one clause per reading), voiding the frame verdicts like the other causes of TEST-012, in the log's `warnings`, in its `frame_valid`, and in the console's session lines. The cause SHALL NOT void the GPU verdicts: a GPU timing is judged by the clock floor and the noise rule (ADR 0036, ADR 0057), not by a CPU kernel (ADR 0093). A run whose scene starts a load of its own before its calibration (the reserve test's loads, BUDG-012) SHALL log `load_calib_declared_load` = 1; its readings over a limit SHALL be recorded in `cpu_calibration_reasons` with the suffix ` (declared load: recorded, not voiding)` and SHALL NOT be a session cause. The log SHALL carry `cpu_calibration_valid` (`true`, `false`, or `null` when no run logged a calibration) and `cpu_calibration_reasons`. `cargo xtask accept` SHALL judge the calibration again from the logged counts of every run, as it judges throttling and noise, with the same scope and the same declared-load rule, and name the reason in its refusal; a log without calibration counts is not judged (not invalid), and a reason recorded at run time stands when the log has no counts to judge again. The limits are named constants of the perf crate with their data in the doc comment.

Verify: A
Status: active
Source: ADR 0065, ADR 0093 (SPIKE-95), STRM-022, TEST-012, TEST-021, perf windows 7 and 11 and the desktop-app smoke (`docs/perf/smoke-desktop-app.md`), owner (2026-10-19)

Rationale (the data of 2026-10-19, in microseconds): on the quiet machine (native-UI coder session; competing processes 0.8 to 3.8 % of the CPU) before single spreads 46,170 to 49,913, before parallel median 85,042 to 106,885, after single 46,329 to 51,080 (59,335 in the one dive with `git` at up to 17 %), after parallel median 90,357 to 103,874; the slow planner terminal (2026-10-16) read before single 56,696 to 78,076 and before parallel median 127,845 to 131,044. The first rule (single 50,000, parallel 100,000, before and after) would have voided, on the quiet data, 2 of 7 window-11 scenes and 2 of 3 smoke dives on the before parallel median and 4 of 10 after single readings; the limits above, the quiet maximum plus about 5 %, void every scene of the slow state and the git-disturbed dive and none of the quiet scenes. SPIKE-95 (2026-11-02): with one timing per reading and the reading before the cool-down, the rule voided 36 % of the unloaded attempts of SPIKE-90 window 1 and 5 of 6 quiet reserve sessions; the readings of 2026-10-19 were single timings, so the limits are kept until the first window measures the new estimator.

#### Scenario: Limits and boundaries
- **WHEN** each of the four readings equals its limit, and then is one microsecond over
- **THEN** the scene is within at the limit, and with a reading over it the scene gets no verdict naming that reading, its value and its limit

#### Scenario: The quiet machine counts, the disturbed scene does not
- **WHEN** the extremes of windows 7 and 11 and the smoke dives are judged, and the slow planner terminal's dive (before single 66,877, parallel 130,386) and the smoke dive with `git` (after single 59,335)
- **THEN** the quiet scenes are within, and the others get no verdict with the readings named

#### Scenario: The calibration voids frame verdicts only
- **WHEN** a lit session's only cause is a calibration reading over its limit
- **THEN** `frame_valid` is false, `gpu_session_valid` is true, the console says the frame verdicts are not valid and the GPU verdicts are, and `accept` judging the log again agrees

#### Scenario: A declared load is recorded, not voiding
- **WHEN** a reserve run logs `load_calib_declared_load` = 1 with a before single reading of 62,183 us, and a run without the count logs the same reading
- **THEN** the first session has no calibration cause, `cpu_calibration_valid` false and the reason with the declared-load suffix, the second gets no frame verdict, and `accept` judging both logs again agrees

#### Scenario: Old logs
- **WHEN** a log written before the rule is judged again: with calibration counts out of range, within range, or without calibration counts (the lit scenes before this change)
- **THEN** the first is no verdict, the second is valid, and the third is not judged and stays valid; a recorded calibration reason the counts do not support goes away

#### Scenario: Accept names the reading
- **WHEN** `accept` is given a window whose `dive` log has an out-of-range calibration
- **THEN** it refuses the window with `INVALID for frame verdicts` and the reason naming the scene, the reading and the limit

#### Scenario: The logging is wired in every kind of scene
- **WHEN** a smoke run of one lit view, one lit flight (both shore-filter runs) and one streaming scene is given `--calibrate` on the software adapter, and a smoke run of a lit view is given none
- **THEN** every run of the first logs the four readings and the two thread counts (positive), the log has `cpu_calibration_valid` true or false and not null, and the run without the option logs none and has `cpu_calibration_valid` null
