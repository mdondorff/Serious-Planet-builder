## MODIFIED Requirements
### Requirement: TEST-039 CPU calibration decides whether a perf scene counts
A perf scene SHALL record the CPU calibration of STRM-022 (a fixed kernel timed alone and on the pool's threads, `load_calib_<before|after>_single_us`, `..._parallel_median_us` and `..._threads`) before and after its case, in the streaming scenes and in the lit views and flights alike (not in a smoke run, unless `--calibrate` asks for it so that the logging can be proved on any adapter). The scene's verdicts count only if every reading logged is within its limit: before single at most 52,000 us, before parallel median at most 112,000 us, after single at most 56,000 us, after parallel median at most 112,000 us (ADR 0065); a reading equal to its limit is within. The parallel limits hold for 18 threads, the calibration's thread count (TEST-089): a log with another or no thread count is judged on its single readings alone. A scene with any reading over its limit SHALL get no verdict: a session cause `no verdict: CPU calibration of '<scene>' outside the limits (ADR 0065): <when> <kind> <reading> us over <limit> us` (one clause per reading), voiding the frame verdicts, not the GPU verdicts (TEST-089), in the log's `warnings`, in its `frame_valid`, and in the console's session lines. The log SHALL carry `cpu_calibration_valid` (`true`, `false`, or `null` when no run logged a calibration) and `cpu_calibration_reasons`. `cargo xtask accept` SHALL judge the calibration again from the logged counts of every run, as it judges throttling and noise, and name the reason in its refusal; a log without calibration counts is not judged (not invalid), and a reason recorded at run time stands when the log has no counts to judge again. The limits are named constants of the perf crate with their data in the doc comment.

Verify: A
Status: active
Source: ADR 0065, ADR 0093 (SPIKE-95), STRM-022, TEST-012, TEST-021, perf windows 7 and 11 and the desktop-app smoke (`docs/perf/smoke-desktop-app.md`), owner (2026-10-19)

Scope: the limits are the reference machine's calibration (Rasierklinge, ADR 0065), and they give no verdict on another machine (ADR 0092 point 5): readings logged on another adapter than the reference one (by the readiness gate's rule; a log names its adapter) are recorded and judged `no verdict (not the reference machine)`, neither a session cause nor `cpu_calibration_valid` false; on the reference machine the parallel limits apply at its 18 threads (TEST-089).

Rationale (the data of 2026-10-19, in microseconds): on the quiet machine (native-UI coder session; competing processes 0.8 to 3.8 % of the CPU) before single spreads 46,170 to 49,913, before parallel median 85,042 to 106,885, after single 46,329 to 51,080 (59,335 in the one dive with `git` at up to 17 %), after parallel median 90,357 to 103,874; the slow planner terminal (2026-10-16) read before single 56,696 to 78,076 and before parallel median 127,845 to 131,044. The first rule (single 50,000, parallel 100,000, before and after) would have voided, on the quiet data, 2 of 7 window-11 scenes and 2 of 3 smoke dives on the before parallel median and 4 of 10 after single readings; the limits above, the quiet maximum plus about 5 %, void every scene of the slow state and the git-disturbed dive and none of the quiet scenes. SPIKE-95 (2026-11-02): with one timing per reading and the reading before the cool-down, the rule voided 36 % of the unloaded attempts of SPIKE-90 window 1 and 5 of 6 quiet reserve sessions; the readings of 2026-10-19 were single timings, so the limits are kept until the first window measures the new estimator.

#### Scenario: Limits and boundaries
- **WHEN** each of the four readings equals its limit, and then is one microsecond over
- **THEN** the scene is within at the limit, and with a reading over it the scene gets no verdict naming that reading, its value and its limit

#### Scenario: The quiet machine counts, the disturbed scene does not
- **WHEN** the extremes of windows 7 and 11 and the smoke dives are judged, and the slow planner terminal's dive (before single 66,877, parallel 130,386) and the smoke dive with `git` (after single 59,335)
- **THEN** the quiet scenes are within, and the others get no verdict with the readings named

#### Scenario: Old logs
- **WHEN** a log written before the rule is judged again: with calibration counts out of range, within range, or without calibration counts (the lit scenes before this change)
- **THEN** the first is no verdict, the second is valid, and the third is not judged and stays valid; a recorded calibration reason the counts do not support goes away

#### Scenario: Accept names the reading
- **WHEN** `accept` is given a window whose `dive` log has an out-of-range calibration
- **THEN** it refuses the window with `INVALID for frame verdicts` and the reason naming the scene, the reading and the limit

#### Scenario: The logging is wired in every kind of scene
- **WHEN** a smoke run of one lit view, one lit flight (both shore-filter runs) and one streaming scene is given `--calibrate` on the software adapter, and a smoke run of a lit view is given none
- **THEN** every run of the first logs the four readings and the two thread counts (positive), the log has `cpu_calibration_valid` true or false and not null, and the run without the option logs none and has `cpu_calibration_valid` null

### Requirement: TEST-040 Perf preflight
A preflight SHALL run before a perf window (`cargo xtask perf preflight [--out DIR] [--smoke] [--machine-ready]`, the scene `preflight` of the binary). It SHALL sample the machine's load for about 70 s, run the readiness gate (ADR 0020, TEST-012) at the start, and measure the CPU calibration (STRM-022) three times about 30 s apart, on the calibration's thread count (TEST-089). It SHALL report the AC state, the active power plan, the top 5 competing processes by mean CPU share over the sampled period, and each calibration reading with its limit. It SHALL refuse, with a non-zero exit and one line per cause naming the reading and the limit, when any gate condition fails, when a calibration cannot be measured, or when the worst of the three readings lies over a limit of ADR 0065 for a reading taken before a case (TEST-039: single over 52,000 us, or parallel median over 112,000 us at the reference thread count); it SHALL otherwise print `PREFLIGHT OK`. The decision SHALL be a pure function over the measured facts, used by the scene and tested at tier A. With `--out DIR` the report SHALL be written to `DIR/preflight.md`. `--smoke` SHALL run one round on any adapter, print the report, and never refuse; `--machine-ready` SHALL print every failure as an owner override and not refuse. A preflight SHALL NOT be a measurement and SHALL NOT change any perf log.

Verify: A, C
Status: active
Source: M5 process review improvement 3, ADR 0020, ADR 0065

Scope: its calibration limits are the reference machine's (TEST-039), at the reference thread count; a preflight serves the reference machine's perf windows (ADR 0020). On another machine its rounds are reported as `no verdict (not the reference machine)` and never refuse; the gate refuses the adapter there.

#### Scenario: Ready machine
- **WHEN** the gate has no failure and all three calibrations are within the limits
- **THEN** the decision passes and the report lists the AC state, the power plan, the top 5 processes and the readings

#### Scenario: Refusals
- **WHEN** the gate fails (battery, a build process, high load), or a calibration is missing, or the worst single reading is over 52,000 us, or the worst parallel median at the reference thread count is over 112,000 us
- **THEN** the decision refuses and names each cause with its measured value and limit

#### Scenario: Worst of three
- **WHEN** two calibrations are within the limits and the third is over
- **THEN** the decision refuses, naming the third reading

#### Scenario: Smoke
- **WHEN** the scene runs with `--smoke` on the software adapter
- **THEN** it prints the report and exits successfully whatever the readings
