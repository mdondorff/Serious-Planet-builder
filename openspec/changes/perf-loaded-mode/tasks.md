## 1. PR 1: gate and report
- [ ] 1.1 Tier A: `readiness(.., Mode)` splits hard conditions from load readings; `LoadRecord`; `--loaded` refused with `--machine-ready` (BUILD-030)
- [ ] 1.2 Tier A: the report rows state `loaded` or `quiet` with the readings (BUILD-032); CPU performance % by index lookup or `unavailable`

## 2. PR 2: verdict function
- [ ] 2.1 Tier A: `loaded_verdict` for the five kinds, the margins and the +18 % GPU note (BUILD-031); the verdict in the loaded rows

## 3. PR 3: A/B helper
- [ ] 3.1 Tier A on fake runs: alternating rounds, kept logs, refusal count and reasons, the table (BUILD-033); `xtask perf --ab`
- [ ] 3.2 One live loaded A/B of a streaming scene as the check
