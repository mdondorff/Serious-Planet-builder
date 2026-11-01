## 1. Change
- [x] 1.1 Planner confirms the design and the standard flights; requirement IDs reserved (BUDG-011, BUDG-012; BUDG-013 held)
- [x] 1.2 Spec deltas (BUDG-011, BUDG-012; BUDG-006 stays)

## 2. Tier A (GPU-free)
- [x] 2.1 (`planet_perf::reserve`: `LOADS`, `spin`, `calibrate`) `SyntheticLoad` definitions with the shares of BUDG-001 to BUDG-003 and the calibration of the CPU spin on a fake clock; a real-clock tolerance test (5 %) (BUDG-011)
- [x] 2.2 (`planet::reserveload::CoreLoad`) The core load: spin jobs on a share keep exactly `ceiling` threads busy and never more; terrain's ceiling holds beside them (BUDG-011, BUDG-008 on fake work)
- [x] 2.3 (`planet_perf::reserve::ReserveReport`) The report: shares, measured value, limit, verdict, largest overrun named (BUDG-007, BUDG-012), on recorded fixtures

## 3. Tier C
- [x] 3.1 (`planet_render::BurnPass`, `planet_frame::reserve::ReservePlan`; the test passes on the software adapter as "cannot calibrate" and on the RTX 3080 Ti within 5 % of 3.0 ms) The GPU burn pass, calibrated with timestamp queries to the GPU ms; skipped with a reason where timestamps are unsupported (software adapter)
- [ ] 3.2 The reserve scene on the software adapter in smoke mode (labelled, no verdict) and on the discrete GPU under `xtask perf reserve`

## 4. Measurement
- [ ] 4.1 Tell the planner when the scene is ready; the planner opens the window (ADR 0087); first run and report
