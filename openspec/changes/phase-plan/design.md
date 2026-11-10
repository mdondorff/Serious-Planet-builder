## Context
- The complete resident build at k=4 (12,288 cells a face edge, 148.9 M land cells) read 11,329 MiB of peak commit with the spill and the release in the example only; the gate is 11,444 MiB (12e9 B).
- GEN-158 moves the spill and the release into `bake::build`.

## Decisions
1. The phase order is the one the measurements fixed: spill z, rounds, drivers, one weighted route returning its filled surface, hydrology, G-CON, release of `q` and `qw`, read-back, sediment pass.
2. One flood: `route_rules_filled` returns the filled surface the weighted route was made on; the plan no longer floods twice (k=1: 30 s against 47 s).
3. `verify_with_area` takes the weighted cell area from before the weight was dropped, so G-CON does not regenerate the weights (that regeneration had raised the peak).
4. Hydrology reads `routing.order`, `receiver` and `area`; routing, filled surface and runoff are dropped when H1 has its result.
5. Nothing changes a bit: `route_rules` is `route_core(.., false)`, `verify` is `verify_with_area(.., None)`.

## Measurements (examples/landmass_scale plan, k=4, two complete runs; raw: `target/review/gscale-memory/plan_k4_run{1,2}.log`)
| mark | working set run 1 (MiB) | run 2 |
|---|---|---|
| landmass | 2076 | 2078 |
| after the rounds (peak inside) | 2301 (7622) | 2077 (7633) |
| drivers | 4011 | 3786 |
| routing and filled surface | 7429 | 7205 |
| weight dropped | 6859 | 6635 |
| hydrology (peak) | 9956 (10861) | 9741 (10645) |
| H1 kept, q and qw released | 3120 | 2905 |
| pre-carve surface read back | 4259 | 4044 |
| after the sediment pass | 2983 | 2946 |

- Peak commit: run 1 11,363 MiB, run 2 11,131 MiB; worst 11,363 against 11,444 (margin 81 MiB, 0.7 %); spread between runs 232 MiB.
- Hydrology is the peak in both runs; `q` and `qw` (f64 a cell each) are the largest part of its growth.
- Hashes equal in both runs and equal to the earlier k=4 runs: H1 label 4c89a2e319dd594d, channel 102381d5446e2b78, segments 7f9817748fde93c0; z after the sediment pass f7bfbb2b39edd6c2. At k=1 the plan's hashes equal the old path's.
- G-CON at k=4 (both runs): q_outlets 1.2e-14, qw_closure 1.5e-14, confluence 5.7e-16 and 4.4e-16, area_outlets 5.4e-13; 0 disconnected, broken or miscovered; all at most 1e-9.
- Spill: 1,194,732,528 B (8 B a cell), deleted after the read-back.
- Wall seconds, run 1 / run 2: rounds 2505 / 2422, weighted route and filled surface 661 / 654, hydrology 75 / 74, check 21 / 20, sediment pass 2082 / 2076, drivers 9; plan 5354 / 5257. CPU: 1.05 / 1.06 cores busy over the plan (the landmass sampling before it: 3.8).
- Pits after the plan (canonical flood): start 1.0016 % of land, after thermal 2.41 % (deepest 1249.1 m), after deposition 0.7355 % (824.0 m), after pit spreading 0.7355 %. The number of one-cell canyon dams was not measured.

## Status (architect verdict, 2026-11-10)
- G-SCALE's complete-build row passes on its rule, with a thin margin: worst 11,363 MiB against 11,444, margin 81 MiB (0.7 %), smaller than the 232 MiB spread between the runs.
- The peak sits in hydrology: commit goes from 8,603 to 11,363 MiB there. A change that adds resident state in or before hydrology re-measures this row (two k=4 runs) before it merges. If a re-measure fails, the first lever is rule (1) inside hydrology, before any tile-wise route.
- Other A3 6.6 scenarios: not run. Not in this change: a tile-wise route, the world build's call site, GEN-155.
- Pits after the pass grow with k: 0.26 % (k=1), 0.35 % (k=2), 0.7355 % (k=4); thermal maximum 609.7 / 920 / 1,249.1 m. A/B-gate evidence, no fix ordered. The dam count comes with the next k=4 run.
- The k=4 runs wrote no stage sheets: the probe calls are in `build_resident`, but the example's plan mode installs no probe. The next k=4 run has it on.
