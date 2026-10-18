## Why
The CPU calibration of STRM-022 was recorded and never judged; a prose rule in `docs/status.md` (single 50,000 us, parallel 100,000 us) disagreed with the readiness gate, which judges load and temperature only. Window 11 and the desktop-app smoke sat at 46,000-48,000 and 88,000-105,000 us, close to it, and the gate said ok while the rule said invalid (opus-planner, 2026-10-19; ADR 0065, option B).

## What Changes
- `planet_perf::calibration` (new): the limits as named constants with their data in the doc comment (before single 52,000, before parallel median 112,000, after single 56,000, after parallel median 112,000 us; parallel limits for 18 threads), `judge` over the logged counts (`NotJudged`, `Within`, `Outside` with one clause per reading), `reason`.
- `validity`: `CauseKind::CpuCalibration` (voids frame and GPU verdicts, one cause per scene); `accept` judges the calibration again from the logged counts of every run (a recorded reason stands when the log has no counts); `calibration_status` for the log.
- The log carries `cpu_calibration_valid` (`true`, `false`, `null` = not judged) and `cpu_calibration_reasons`.
- The lit views and flights record the same calibration before and after their measured frames (`litperf.rs`); the streaming scenes already did. Smoke runs record none.
- TEST-039; the prose in `docs/status.md` and the explorer's briefing replaced by pointers.

## Tests
Tier A: limit boundaries for each of the four readings, the quiet-machine extremes and the slow-state and git-disturbed scenes, the thread-count rule and the not-judged case (`calibration`); the session cause, its wording, the log fields and one cause for three runs of a view (`validity`); old logs judged again, recorded reasons kept or dropped (`validity`); `accept` refusing a window whose `dive` is out of range (`accept`).

## Not tested here
The lit views' calibration against the real hardware (a perf window will show the first logged values; no window was run for this change), and the first real readings of the lit scenes against the limits (the limits come from the streaming scenes' 18-thread readings).

## Classes and bounds (ADR 0035)
No field and no shader value is added; the perf harness's verdict rules.

## Out of scope
Limits relative to a per-machine baseline (deferred until perf runs on a second machine, ADR 0065 point 4); a calibration for the terrain views.
