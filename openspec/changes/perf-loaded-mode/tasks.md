## 1. PR 1: gate and report
- [x] 1.1 Tier A: `gate::readiness_mode(.., Mode)` splits hard conditions from load readings; `LoadRecord`; `--loaded` on `xtask perf` and `test-render`, refused with `--machine-ready`; loaded mode records the ADR 0065 calibration instead of voiding the session and marks the quiet verdicts as not counting until PR 2 (BUILD-030)
- [x] 1.2 Tier A: the session log states `mode` on the session and on every run row, and the `load_record` (BUILD-032). Not in this PR: the CPU performance % reading (needs the counter lookup by index) and the modes of the one-off logs (`global-bake`, `coupled`, `envelope`, `edit`), which keep their own writers

## 2. PR 2: verdict function
- [x] 2.1 Tier A: `planet_perf::loaded::loaded_verdict` for the five kinds, the margins and the +18 % GPU note (BUILD-031)
- [ ] 2.2 The verdict on the rows of the loaded session log (BUILD-032): needs the table that maps each logged metric to a kind and a budget (the scenes' verdict lines are scattered: terrain GPU, plan CPU, streaming frame, upload); proposed in PR 3 with the A/B helper, which is its first user

## 3. PR 3: A/B helper
- [ ] 3.1 Tier A on fake runs: alternating rounds, kept logs, refusal count and reasons, the table (BUILD-033); `xtask perf --ab`
- [ ] 3.2 One live loaded A/B of a streaming scene as the check
