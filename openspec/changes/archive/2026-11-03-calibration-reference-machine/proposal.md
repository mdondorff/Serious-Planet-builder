## Why
ADR 0092 point 5 (owner, 2026-11-02): reference-machine calibrations "are named as such and give no verdict elsewhere". `planet_perf::calibration::judge` applied the single limits (TEST-039) on any machine. A session on another adapter got a calibration cause beside its adapter cause, and its log read `cpu_calibration_valid` false. `accept` judged such a log the same way, and the preflight refused on its rounds. The planner kept the ADR as accepted and asked for the code to follow (opus-planner, 2026-11-03); it was tracked in docs/status.md until now.

## What Changes
- `calibration::judge_for(count, on_reference_machine)`: on another machine any reading present is `Judgement::NotReferenceMachine`, "no verdict (not the reference machine)". A run without readings stays not judged, and the reference machine is judged as before (`judge`).
- The machine is the reference one by the readiness gate's rule: `check_reference_adapter`, or `is_reference_adapter_name` for a log, which records the adapter's name only. A log that names no adapter is judged as before.
- The session's causes, `calibration_status` (now given the adapter), `accept`'s judging of a log again, and the preflight (a `reference_machine` fact; its rounds are reported and never refuse) use it.
- TEST-039's and TEST-040's Scope lines state the rule; the Tracked line is removed.

## Impact
- `crates/perf/src/calibration.rs`, `validity.rs`, `preflight.rs`, `lib.rs`; `crates/app/src/preflightperf.rs`.
- Tier-A tests (TEST-039, TEST-040): the judgement off and on the reference machine; the session, the log and `accept` on another adapter; the preflight's rounds off the reference machine.
- Nothing changes on the reference machine.
