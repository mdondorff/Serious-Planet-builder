## Why
Round 3 of the M9 generator (planner, 2026-11-02): the relief on the round-2 continents. SPIKE-93 measured Earth's relief with ETOPO 2022 and put the engine's seed 3 far from it: 20 % of land under 500 m against 58 %, a median land elevation of about 1,180 m against 410 m, coastal land at 789 m against 82 m, belts following every coast, mountains 8.5 % of land.

## What Changes
- **Base elevation (GEN-096):** the coast ramp tops at 620 m over a land depth of 1.2 (1000 m over 0.9); the belt term 7800 m (7500), the plateau uplift 600 m (800).
- **Class source (GEN-094, GEN-095):** up to 64 plate draws (16); belt half-width `200 + 220 n` km (`260 + 300 n`); the coast gate ramps over the smooth depth (0.3) and the true depth (0.2); a hilly-country field adds hills away from the belts; the mountains' membership is `smoothstep(0, 0.4, max(core, flank))`, the hills' `smoothstep(0.1, 0.35, max(flank, old margin, hilly))`; the plateau field's ramp is wider. `CLASS_SOURCE_VERSION` 5; `GLOBAL_VERSION` stays 6.
- **Preview (TEST-086):** the report prints SPIKE-93's rows (land under 500 m, median elevation, coastal median, land above 1 and 3 km, belt land, the `k`-step distribution) beside Earth and the proposed bands, and the class bands follow TEST-082.
- **TEST-082:** the single-seed bands become SPIKE-93's bands over seeds 1 to 5 with a gate form (the median within the band widened by 10 % of its width; no seed outside by more than the stated tolerance). The class bands are approved (owner, 2026-11-02); the other bands are the planner's, from SPIKE-93.
- **GEN-097 (owner, 2026-11-02):** the step limit applies to the 99th percentile of the lattice steps of the untapered `k` (at most 2.0, a bake parameter `max_step`); the largest step is reported (5.1-5.8 on seeds 1-5, 5.2 on seed 14). The preview report and TEST-082's gate test follow.
- Not in this change: belt land (13-23 %, median 9.7 %, seed 4 at 5.4 %), which needs belts only at convergent margins (reported, gated with that round); wiring into the bake.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-094 to GEN-097 (planned, in `m9-bake`), GEN-105 (round 2's plate draws: 64)
- `testing`: TEST-082 (planned, in `m9-bake`), TEST-086's report

Expected golden changes: none (the class source reaches no render path; the global stage is unchanged).
