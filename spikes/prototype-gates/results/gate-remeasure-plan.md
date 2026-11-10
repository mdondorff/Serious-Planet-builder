# Plan: independent re-measurement of the prototype gates on current main

Status: plan only (briefing item 12, bulldozer inbox #1 of 2026-11-10 step 2). Nothing is built or run. Main at 59a7deaa; the plan is stamped to it and re-based before any run. SPIKE-106 (closed 2026-11-08, main 8b57df69) is the baseline of what an independent probe found.

## Rule of independence
- Own build in this worktree's `target/`, own probe code on a spike branch (never merged), no reuse of the lanes' logs or example output as evidence. The ledger rows are read only after the probe's numbers exist, for the comparison.
- The probe re-implements what the gate judges from the documented rules, as SPIKE-106 did: the filled surface (a Dijkstra with the opposite tie order, and a queue-free relaxation of the definition), receivers, clipped MFD area, `Q`, channel cells and conservation sums, basin components, cell area and distance (Gauss-Legendre against the metric). What it shares with the code under test stays listed (the base, `jitter`, `Grid::neighbour`, `runoff_mm`).
- The probe of SPIKE-106 (`spike/prototype-gates`, `crates/generators/examples/gate_probe.rs`, f1f0e9c8) is the start. It predates later changes on main (GEN-160 canonical K7, GEN-162 breach exit base, GEN-163 lake pieces, GEN-165 width law): the port is part of the cost.

## What is re-measured
| Gate | Windows | How (own probe unless stated) | Compare with | Expected run time (8 threads) |
|---|---|---|---|---|
| G-ROUND | R3a, R3b, prototype region R1 | whole-region carve against the round solver, 20 steps of 5 and 12 steps of 4, 1, 2, 5, 8 threads | ledger G-ROUND rows (explorer, spikes) | 1 to 1.5 min a window |
| G-DET | the same | 1, 2, 5, 8 threads, resident against partitioned, bit equality of z, receivers, `A_d`, H1 counts; plus an FNV hash of the canonical outputs written to a file, so a Linux run of the same commit can be compared by CI (Windows against Linux is not a thing this machine can show) | ledger G-DET | within the G-ROUND run |
| G-PART | the same | tile 128 and 256, z0 and carved, with the weights if `partitioned_route` takes them on main (SPIKE-106 finding 3: it did not); the spiral fixtures | G-PART rows | about 1 min a window |
| G-CON | the same, and the lake pieces | `Q` at confluences and outlets, `Q_w` closure, `A_d` at outlets, connectivity, on the produced pieces and not only on H1: a channel piece ends before water and restarts at the spill (GEN-163), so the probe states which G-CON form it judges (adjacent to review item 11) | G-CON rows | under 1 min a window |
| G-LAND | the same | own pit classification (kept basin, artefact, ramp, unclassified) from heights and `F - z > 0.5 m`; relief p50 and hypsometry against the whole-region carve, which the library example does not print | G-LAND row (`partial`) | under 1 min a window |
| G-SED | the same | own accounting of eroded, deposited and exported from `z0`, `z` and the report, per catchment and per round; the library's sediment pass is the code under test and is shared; thermal maximum per cell reported (review §3) | G-SED rows | 1 to 2 min a window |
| G-SURF, G-SURF-I | tile (65, 7) of face 1 of the prototype region | `k7_tile` through the library; own evaluation of `S = B + L` at the local cell centres and at the lattice vertices, own interpolant of the bake's cell values | G-SURF rows (pass at 0.125 m) | not measured by me: the example's stage times are not in the ledger; guess 5 to 10 min, to be replaced by one timed run |

Windows: R3a (seed 15, face 4), R3b (seed 15, face 2) at margin 200 (1.04e6 cells) as in SPIKE-106 section 6; R1 (seed 14, face 1). Optional fourth window: a seed-16 hills region, fixtures chosen before the run.

## Not covered (stated, not hidden)
G-EQ and G-INC (edit paths; the edit-cases example is explorer's), G-BASIN, G-STORE, G-SCALE (a 2 x 11 GB run, belongs to the explorer's k=4 slot), G-RT, G-PROFILE (not run by anyone on main), G-LOCAL (not run by anyone; the key-input claim is the one S1 rests on), Windows against Linux. Offered if wanted: a G-LOCAL probe (a catchment round recomputed from its key inputs alone) and G-PROFILE on the fixture tile; each needs its own design first.

## Cost
| Part | Time | Load |
|---|---|---|
| Cold release build of `planet_generators` examples in this worktree | 8 to 15 min (estimate; not measured here) | all cores; run with `-j 4` at BelowNormal if the machine is shared |
| Port of the probe to main's API | 1 to 1.5 h of the lane, no machine load | none |
| Window runs (3 windows, all gates, 1/2/5/8 threads) | about 25 to 35 min (SPIKE-106's runs: inputs read 11 to 14 s, 4 carve configs of 3 to 6 s each, route checks 10 to 20 s, per window and profile) | 8 threads at BelowNormal, under 3 GB each (windows of 1e6 to 3.3e6 cells) |
| G-SURF tile | 5 to 10 min (guess) | one thread mostly |
| Disk | about 6 to 10 GB under `target/`; 118 GB free | prepush and bake refuse below 20 GB |

Total machine time with the build: about 1 h to 1.25 h, in three slots if wanted: (1) the build, (2) the window runs (the part that needs 8 free threads, because the thread-count variants measure at 1, 2, 5, 8), (3) the G-SURF tile. Nothing here needs the real GPU, `test gpu --real` or a perf window; none of it is a measurement of time (CON-21).

## Order and stops
1. After a go for slot 1: build, a one-window smoke of the ported probe (R3a, G-ROUND only, 2 min).
2. Slot 2: the three windows, all gates; a disagreement with a reported number is reported to the architect and not fixed.
3. Slot 3: G-SURF.
4. Results `spikes/prototype-gates/results/` (a SPIKE number from the bulldozer if the register is to hold it; `docs/spikes.md` is at 9,505 of 10,240 B), one docs-only PR.
Stop and report if: the ported probe disagrees on a gate at once (check the port first, then report); a window needs over 8 GB.
