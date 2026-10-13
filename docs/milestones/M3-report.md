# M3 Field pipeline and cache: report (2026-10-13) and go/no-go gate

Written for: the owner, reading this to decide whether M3 passes. Evidence below is from this session's runs and the merged log; `docs/log.md` has every change.

## Summary
The generator pipeline now runs from a world definition to stored, verifiable tiles: a schema-versioned definition with read-recording contexts, a staged field model, a global-coarse bake of continents, elevation and climate stored as content-addressed blocks, a terrain field and an eroded terrain field that are pure functions of the direction, a persistent Hilbert-ordered tile store with keys, verification, compaction and a size cap, and height-aware level-of-detail selection that plans realistic terrain at production parameters. The code is on main with CI green on Windows (WARP) and Linux (lavapipe), except this change (`generate --plan-check`, this report). Not yet on main: ADR 0030 and the 16384 perf-window line, both on the planner's PR #95 (open when this was written); GEN-014 stays `planned` in the spec until that merges.

## Criteria (report §15, M3)
| Criterion | Result | Evidence |
|---|---|---|
| Bit-exact CPU tiles across runs, threads and OSes | pass for the reference generator and, by known answers, for the new fields | the CI `determinism (Windows vs Linux)` job hashes only the reference generator's manifest; the bake, `cpu-terrain-v1` and `cpu-erosion-v1` are pinned by known-answer and thread tests that CI runs on both systems (GEN-013, GEN-016, GEN-017); persistent known answers are not in the manifest yet |
| Global 1 km bake in at most 10 min (GEN-014) | pass | the architect's perf window: 370.3 s on 20 threads at 16384 cells per face edge, 11.66 GB store (recorded in PR #95, not yet merged); a second bake during this acceptance, outside the readiness gate and so not a measurement: 355.8 s, 1,536 blocks, at most 20 in memory (`docs/milestones/M3-plan-check-16384.txt`). The memory figure is derived from the block counter, not a process measurement |
| A definition edit invalidates only the affected tiles | pass for the mechanism | STRM-004 tests (tier A, and the tier B item-edit oracle `oracle_tiles_item_edit`, on a stamp generator); for the real fields any definition edit changes the world's node set id and so every tile (ADR 0025 point 3), so it is not yet shown on them |
| `cache-verify` passes | pass in tiers A and B; blocks verified on the production bake | STRM-004, STRM-009, STRM-010 tests; `generate --verify-global` on the production bake: 24 of 1,536 blocks checked, 24 match, 0 stale, 0 unreadable. A tile `cache-verify` on a real store is the owner's |
| Metamorphic tests (partition invariance, coarse/fine agreement) | pass | GEN-015 (the same position reads the same bits through any tile decomposition) and GEN-016 / GEN-017 (one point, one height from tiles of different levels, across face edges and the cube corner); the reference generator's coarse/fine agreement is exact by construction (GEN-006) |
| Realistic terrain plans at production parameters (cells 32, viewport / tau 240; ADR 0016) | **pass at production resolution** | see below and `docs/milestones/M3-plan-check-16384.txt` (the raw output) |
| Global hydrology spike | done (SPIKE-03), decided by the owner (ADR 0030) | `docs/spikes.md`; the stream-power stage is after M3 |
| GPU budgets and regressions | not re-measured | the one valid perf window (2026-10-12) passed every budget, but ran before the regression report existed; the comparison has never run and the ground-1m rise is unexplained (see Perf) |

## `cargo xtask accept M3` (2026-10-13, this branch)
check (fmt, clippy, layering), spec-lint, tests A+B (31.9 s), tier C on the software adapter (51.0 s) and on the discrete GPU (35.5 s, `test gpu --real`), and the three run-mode smokes all pass. The last step, `test fast` timing, **failed once: 11.0 s against the 10 s budget**, when it ran straight after the GPU steps; two reruns on the quiet machine took 8.4 s elapsed (7.5 s of tests, 472 tests). The budget is not weakened; the margin is thin and the slowest tests are the older planet-frame LOD tests (4 to 5 s each).

## Production-resolution planning check (the condition of ADR 0016 and ADR 0030)
`generate --global-dir D --global-res 16384` then `generate --plan-check D --global-res 16384` (new in this change). The standard flight (ten scripted views) planned at cells 32, viewport / tau = 1080 / 4.5 = 240, over the stored 16384-cell bake, with both fields; no view is rejected for steepness:

| View | Terrain field: nodes / deepest level | Eroded field: nodes / deepest level |
|---|---|---|
| orbit-20000km | 52 / 2 | 52 / 2 |
| high-2000km | 429 / 6 | 429 / 6 |
| aerial-200km | 651 / 9 | 655 / 9 |
| aerial-20km | 1,095 / 12 | 1,106 / 12 |
| low-2km | 2,599 / 16 | 2,625 / 16 |
| ground-200m | 4,468 / 19 | 4,498 / 19 |
| ground-1m | 8,735 / 26 | 8,763 / 26 |
| north-pole-1km | 2,556 / 17 | 2,562 / 17 |
| cube-corner-1km | 3,585 / 17 | 3,617 / 17 |
| face-edge-1km | 3,542 / 17 | 3,572 / 17 |

The eroded field's real, interval-arithmetic bounds cost 0.6 % more nodes over the ten views (27,879 against 27,712; up to 1.0 % at aerial-20km and low-2km). Planning takes under 0.3 s per view from aerial-20km down; the orbit, high and aerial-200km views took 0.7 to 6.5 s on cold block reads. The tier A tests plan the same flight on a 256-cell bake.

## What M3 delivered
- Field pipeline: `WorldDefinition` (TOML, schema version 1, cell index), `GenContext` read recording, the stage model, `cpu-global-coarse-v1` bake (tiled, thread independent), the global block store (kind 3), `read_direction` and the tile reader (ADR 0027), `cpu-terrain-v1` (GEN-016) and `cpu-erosion-v1` (GEN-017).
- Cache: 128-bit persistent keys (ADR 0018), pack format with torn-tail handling and Hilbert index (STRM-008), tile payload codec, verification that fails when it compares nothing, compaction and the size cap with an injectable rank (STRM-013, ADR 0029).
- LOD: height-aware selection and morph (LOD-008), the plan cache, per-view pinned plan counts (TEST-011).
- Harness: the readiness gate (ADR 0020), the global-bake scene, regression comparison against the last valid log (TEST-012).
- Spikes: SPIKE-04 (erosion variants; B picked) and SPIKE-03 (hydrology; the stream-power stage is post-M3, ADR 0030). Spike code stays on `spike/erosion-variants` and `spike/hydrology`, unmerged.

## Perf
- First perf window (2026-10-12, valid): ground-1m GPU median 4.06 to 4.22 ms, p99 12 ms; ground-200m 2.3 ms; all budgets pass (8 ms / 20 ms); CPU plan 5.9 ms warm at ground-1m (budget 8).
- The ground-1m GPU median rose from about 3.0 to 4.2 ms between the owner's run and this one. No node, draw or triangle count changed; the cause is not isolated (per-vertex or per-fragment cost after the 44-byte morph-height vertex and the larger fragment shader). `docs/learnings.md` proposes a three-build A/B for the next perf window. The regression report now flags such rises automatically.
- `cargo xtask perf` has not been run on the eroded field (nothing renders it yet).

## Open risks and limits
- The eroded field is not consumed by the app yet (nothing selects it over `cpu-terrain-v1`); wiring is the first post-M3 change that needs a decision about which field a world uses.
- The eroded field's `f32`-rounding margin in the slope bound is not separately tested (disclosed in the change).
- The global stream-power stage needs a routing without lattice hatching and an explanation of the 1.6x / 2.7x across-edge gradient ratios before it ships (ADR 0030 point 2).
- The per-tile node set id, the last-hit sidecar (the cap's eviction is by write order, not LRU: ADR 0029) and the ADR 0021 point 7 contract test for cell-reading fields are open; none blocks M3, all must land before the first regional field is stored.
- The GPU rise above is unexplained.
- `cargo xtask test fast` has a thin margin: 8.4 s on a quiet machine, 11.0 s once inside `accept` (see above); the old planet-frame LOD tests are the slowest, and this change's own CLI test (4 s) was one of the slowest until review moved it to the tier B binary `oracle_plan_check`.
- The relief-view flight (TEST-014) has not been reviewed by the owner on the real GPU at this gate.

## Needs the owner
1. The TEST-014 flight review (`target/review/flight-relief-nvidia/` plus contact sheet; run `planet test-render --scene terrain --flight N --out-dir DIR --adapter hardware`).
2. The go/no-go decision below.
3. Which field a world uses (terrain or eroded) once something renders it.

## Recommendation: **Go with conditions**
Every M3 exit criterion in report §15 has evidence, including the production-resolution steepness check that ADR 0016 required. Conditions before M4 starts: PR #95 merges (ADR 0030, the 16384 perf-window line) and GEN-014 becomes `active`; the owner completes the TEST-014 flight review; the `test fast` margin is watched (the next slow test moves to tier B). None of the open items is a foundation risk.

Stop here: M4 is not started until the owner decides.
