## 1. PR 1: gate and report
- [x] 1.1 Tier A: `gate::readiness_mode(.., Mode)` splits hard conditions from load readings; `LoadRecord`; `--loaded` on `xtask perf` and `test-render`, refused with `--machine-ready`; loaded mode records the ADR 0065 calibration instead of voiding the session and marks the quiet verdicts as not counting until PR 2 (BUILD-030)
- [x] 1.2 Tier A: the session log states `mode` on the session and on every run row, and the `load_record` (BUILD-032). Not in this PR: the CPU performance % reading (needs the counter lookup by index) and the modes of the one-off logs (`global-bake`, `coupled`, `envelope`, `edit`), which keep their own writers

## 2. PR 2: verdict function
- [x] 2.1 Tier A: `planet_perf::loaded::loaded_verdict` for the five kinds, the margins and the +18 % GPU note (BUILD-031)
- [x] 2.2 The verdict on the rows of the loaded session log (BUILD-032): `loaded::row_verdicts` maps the rows of the streaming scenes (frame median and p99: needs a quiet window) and of the terrain views (GPU median, CPU plan median, frame p99) to a kind and a budget, `loaded_verdicts` in each run object; rows of the lit passes and the one-off scenes carry none (Tracked)

## 3. PR 3 and 4: A/B helper, CPU performance
- [x] 3.1 Tier A on fake runs (`planet_perf::ab`): alternating rounds, kept logs, refusal count and reasons, the table with the metric table (BUILD-033); `xtask perf --ab` (`xtask/src/perfab.rs`: the worktree of the ref under `target/ab/base`, the runs under `target/ab/runs/<time>/`)
- [ ] 3.2 One live loaded A/B of a streaming scene as the check
- [x] 3.3 PR 4: the CPU performance % in the load record (`Win32_PerfFormattedData_Counters_ProcessorInformation`, locale-free) and the row verdicts (2.2)
