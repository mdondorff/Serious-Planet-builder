## 1. Change
- [ ] 1.1 Planner confirms the design and the standard flights; requirement IDs reserved
- [ ] 1.2 Spec deltas (BUDG-006 scenario kept; new requirements)

## 2. Tier A (GPU-free)
- [ ] 2.1 `SyntheticLoad` definitions with the shares of BUDG-001 to BUDG-003 and the calibration of the CPU spin on a fake clock; a real-clock tolerance test (5 %)
- [ ] 2.2 The core load: spin jobs on a share keep exactly `ceiling` threads busy and never more; terrain's ceiling holds beside them (BUDG-008 on fake work)
- [ ] 2.3 The report: shares, measured value, limit, verdict, largest overrun named (BUDG-007), on recorded fixtures

## 3. Tier C
- [ ] 3.1 The GPU burn pass, calibrated with timestamp queries to the GPU ms; skipped with a reason where timestamps are unsupported (software adapter)
- [ ] 3.2 The reserve scene on the software adapter in smoke mode (labelled, no verdict) and on the discrete GPU under `xtask perf reserve`

## 4. Measurement
- [ ] 4.1 Tell the planner when the scene is ready; the planner opens the window (ADR 0087); first run and report
