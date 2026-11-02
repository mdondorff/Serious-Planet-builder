## Why
ADR 0092 (accepted, owner 2026-11-02) turns the core shares into fractions of the P-core capacity. Today BUDG-003 and the code fix the reference machine's counts: terrain 12 of 20 threads (`planet_perf::reserve::TERRAIN_CORES`), the tile pool = logical threads − `GEN_RESERVE`, horizon bakes ≤ `HORIZON_WORKERS_MAX` = 8, the coupled reduction ≤ 8, and the reserve test's synthetic loads use fixed core counts. The job system sees only `available_parallelism`, with no P/E topology (SPIKE-89). The owner wants a conservative terrain share: flight is the performance goal, teleport is recorded only, and no machine's counts appear as rules.

## What Changes
1. **Capacity.** `B = P − 2`, where P = logical threads on performance cores (SMT siblings counted). A machine without core classes counts every thread as P.
2. **Shares.** `share = clamp(floor(f × B), min, max)` per subsystem, with a fit rule when the minimums exceed B. The fractions, minimums and maximums are a table in BUDG-003. Per-machine counts appear only as labelled worked examples.
3. **Topology detection** (a new requirement):
   - Windows: `GetLogicalProcessorInformationEx`, a feature of the existing `windows = 0.62` dependency of `planet-streaming`.
   - Linux: sysfs.
   - Fallback: `available_parallelism`.
   - Override: `PLANET_CPU_TOPOLOGY=P,E`.
   - No new crate.
4. **Code.** The tile pool, the terrain ceiling, the horizon bakes (terrain / 2), the coupled reduction (B) and the synthetic loads (BUDG-011) derive from the shares. No hardcoded counts remain.
5. **Cross-references.** TEST-039 and TEST-040 are named as reference-machine calibrations; the README, domain and status pointers are updated.

## Impact
- Code: `planet-streaming` (topology and shares, pure where possible), `planet` app (pipelined rig, horizon, coupled run), `planet-perf` (reserve test).
- Specs: BUDG-003, BUDG-011, BUDG-012 (a scenario), STRM-020, STRM-064, TEST-039 and TEST-040 (labels), plus one new requirement for the detection (ID from the planner).
- Behaviour on Rasierklinge: terrain falls from 12 to 6 threads in the worked example (0.6 × B, B = 10). Flights are confirmed in the next quiet slot (ADR 0092; the slot's step 4 already runs them at 6 workers).
- No perf run in these PRs.
