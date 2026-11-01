## Why
ADR 0087 (accepted 2026-10-30; owner 2026-10-31: concurrent perf testing is acceptable when it is monitored and acted on): a time verdict measured on a loaded machine is a PASS when it is under budget by the loaded margin, a loaded FAIL is never a verdict. Today the readiness gate (ADR 0020) refuses every loaded run, so each perf verdict waits for an exclusive window. The job-system A/B of one evening needed a retry loop around the gate, 7 refusals caused by other lanes' `cargo` processes, 8 of 20 sessions invalid by the calibration limits (ADR 0065) and an ad-hoc interleaving script. Planned item B9 follow-up (planner, inbox #10, 2026-10-31).

## What Changes
1. A loaded mode of `cargo xtask perf` (`--loaded`): the gate keeps its hard conditions (AC power, power scheme, reference adapter, GPU temperature, GPU throttling) and records the load conditions (build and test processes, CPU load, GPU utilisation, the CPU calibration of ADR 0065) as readings instead of refusing on them (BUILD-030). `--machine-ready` stays the owner's override; loaded mode is not one.
2. The loaded verdict rules of ADR 0087 as one pure function: loaded PASS at the 5 % margin, loaded FAIL is inconclusive, CPU-timed scenes only pinned or at a 30 % margin, GPU timings carry the +18 % note, the exclusive verdicts stay quiet-only (BUILD-031).
3. The report states `loaded` or `quiet` and the load readings on every row (BUILD-032).
4. An A/B helper: interleaved runs of the working tree and a ref, one verdict (BUILD-033).

## Impact
- Code: `planet_perf` (gate split, verdict function, report fields; GPU-free, tier A), `xtask` (`--loaded`, `--ab`).
- Specs: build-tooling BUILD-030 to BUILD-033 (planner-reserved). TEST-040 (preflight) gains a note that loaded mode records its readings.
- Three PRs: gate and report (BUILD-030, BUILD-032), verdict function (BUILD-031), A/B helper (BUILD-033). No perf window: tier A on fake readings, one loaded A/B as the live check.
- Amends ADR 0020 in the part ADR 0087 already amended: coder lanes may run perf scenes for loaded passes.
