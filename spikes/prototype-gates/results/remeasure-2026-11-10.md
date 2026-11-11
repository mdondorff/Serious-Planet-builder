# SPIKE-106 round 2: re-measurement of the prototype gates on main 760bf0e7

Run 2026-11-10 by the spikes lane (a reader that built none of items 9 to 15), on the bulldozer's go. The plan is `gate-remeasure-plan.md`; the SPIKE number of this round is the bulldozer's to confirm.
Main 760bf0e7 (kind 13 v3, lakes, width law GEN-165 are in). Release build (46 s), Windows, 8 threads, BelowNormal, margin 200 (1,020 x 1,020 = 1,040,400 cells a window), 3 min a window for `gates`, 25 s for `local`, 41 s for `gate_surf`.
Probe code: local branch `spike/gate-remeasure` (`crates/generators/examples/gate_probe.rs`, `gate_surf.rs`, commits b58d3f51 and 277f0d9b); spike code, never merged. Raw output: `remeasure/`.

## Result

| Gate | Verdict of the measurement | Value |
|---|---|---|
| G-ROUND, G-DET | agree, 0 | 20 steps of 5 and 12 of 4, whole-region carve against the round solver at 1, 2, 5 and 8 threads: 0 cells differ on every window |
| G-PART | agree, 0 | tiles of 128 and 256 cells, on the base and on the carved surface: 0 filled, 0 receivers, 0 `A_d` differ from the resident route and from the own route on every window |
| G-LOCAL | round is a pure function of its key inputs: 0 (L2, L4 in 6 of 6 runs); the member order is not in the spec | section 2 |
| G-SURF at the centres | agree | 0.0e0 m before quantisation, 0.125 m after (half a step) |
| G-SURF-I | own measurement equals the library's on the same code; the values moved since the ledger rows | section 3 |
| Conservation | agree, at rounding | sum `Q` at the outlets against sum `a r` at most 3.0e-14 relative, sum `A_d` against `a w` at most 1.4e-14, own confluence error at most 4.4e-16 |
| Own `Q` against the library's | agree | at most 3.0e-15 relative; channel cells, heads, confluences and mouths equal |

Not measured here: G-CON on lake pieces, G-EQ, G-INC, G-BASIN, G-STORE, G-SCALE, G-RT, G-PROFILE, Windows against Linux. G-SED is measured only as the pit totals below.

## 1. `gates` (the structural gates on three windows)

| Window | Seed, face, region proper | Land | Round 20/5 and 12/4, 1/2/5/8 threads | Flood by opposite ties, sweep relaxation | Resident route against own | G-PART 128 and 256, z0 and carved | Channel cells own / library | Pit cells own / library | Pit components own = basins + artefacts + ramps |
|---|---|---|---|---|---|---|---|---|---|
| R3a | 15, 4, i 368-987, j 4720-5339 | 69.9 % | 0 differ | 0 differ (5 sweeps) | 0 receivers, 0 `A_d` bits | 0 | 73,430 / 73,430 | 20,517 / 20,517 | 392 = 207 + 184 + 1 |
| R3b | 15, 2, i 9968-10587, j 8304-8923 | 84.9 % | 0 differ | 0 differ (6 sweeps) | 0 | 0 | 67,599 / 67,599 | 6,418 / 6,418 | 82 = 50 + 30 + 2 |
| R1 (the prototype region) | 14, 1, i 8056-8675, j 760-1379 | 69.6 % | 0 differ | 0 differ (7 sweeps) | 0 | 0 | 45,623 / 45,623 | 42,357 / 42,357 | 136 = 113 + 23 + 0 |

Further per window: inputs read in 12 to 15 s with 0 of 200 spot reads differing from a single-thread read; the own outlet mask against `Grid::outlets` 0 cells differ; cell area against Gauss-Legendre at most 1.1e-8 relative, distance against the great circle at most 1.2e-12.
Unkept pit cells (F - z above 0.5 m, not in a basin): R3a 320 (0.044 % of land), R3b 105 (0.012 %), R1 36 (0.005 %). Kept basins: R3a 207 (0 endorheic), R3b 50 (0), R1 113 (3 endorheic). Land change of the whole window (m, p5 / p50 / p95 / max): R3a -81.0 / -3.0 / 13.0 / 20.0, R3b -88.1 / 7.4 / 16.8 / 20.0, R1 -416.9 / -57.6 / 8.6 / 20.0.
Channel density: R3a 10.1 % of land cells (34.7 heads a 1,000 km2), R3b 7.7 % (17.0), R1 6.3 % (15.9).

## 2. G-LOCAL (`gate_probe local`)

The probe cuts the round's routing into catchments from its own routing (own flood, own receivers, own `A_d`), builds the key inputs of A3 6.5 (members, receivers as positions, outlet flags, `A_d`, distances, taper, round-start surface), and runs its own kernel (breach with the GEN-162 exit base, then 5 implicit sweeps, written from the documented rules, not from `catchment_round`) on them.
Rounds 1 (from the base) and 3 (after two carved rounds), f64 rounds.

| Test | R3a | R3b | R1 |
|---|---|---|---|
| Catchments, with more than one cell, largest (round 1) | 315,589, 6,387, 154,353 | 160,037, 2,528, 145,292 | 318,586, 2,541, 263,713 |
| L1 own kernel on own key inputs against the library's one round (round 1 / round 3) | 0 / 0 | 0 / 0 | **135 / 0** |
| L1b own kernel on the library's own member order and inputs (round 1 / round 3) | 0 / 0 | 0 / 0 | 0 / 0 |
| L2 descending, shuffled order, 1, 5 and 8 threads | 0 | 0 | 0 |
| L4 library `catchment_round` against own kernel on the same inputs | 0 | 0 | 0 |
| L4 clip Local against Off on identical inputs (round 1 / round 3), catchments differing | 89 / 53 | 209 / 61 | 90 / 70 |

Findings:
- **The member order is part of the result and A3 does not define it.** R1 round 1: 135 cells of one catchment (15,128 cells) differ, up to 4.59 m. The probe ordered members by (filled level, id), the reading of A3 6.3 and 6.5. The library walks `Routing.order`, a depth-first walk of the donors from the outlet in index order (`cut`). The lists differ in 1,047 to 2,560 catchments a run (same members, other order). The breach is order dependent where two donors lower one receiver in sequence under `breach_max`. On the library's order the own kernel matches in all 6 runs (L1b), and receivers, `A_d` and distances are equal in every catchment with the same list.
- The order is a function of ids and receivers only, so purity holds. A3 6.5 must name it (the key lists "the member cell ids" without an order) and the key hash must keep the members in that order (read from the code, not re-checked).
- **The memo key misses the round rules.** The clip rule alone changes the result of 53 to 209 catchments on identical inputs (L4). `catchment_key` hashes no round rules (round.rs, read from the code). Latent while the rules are fixed per solve (fixed by #1355).
- L3 (one input changed at a time, largest and median catchment, round 1): +1 mm on z moves 7 / 0 / 0 cells (R3a / R3b / R1; the largest catchment), `A_d` x (1 + 1e-9) 7 / 26 / 235, distance 7 / 27 / 496, taper -1e-6 7 / 27 / 491, the receiver-is-outlet flag flipped 0 / 29 / 2,209, two siblings exchanged in the member order 0 / 0 / 0 on the largest and 2 on a median catchment of R3b. The outlet flag and the sibling order are read (R3b, R1). R3a moves 0 for both.
- Limits: one probe, three windows, two rounds, f64 rounds (repeat after GEN-166's f32 rounds); L3 not run with a rule flip (waits for the memo-key fix); L3 flips one member of one catchment at a time.

## 3. G-SURF and G-SURF-I (`gate_surf`)

**Correction 2026-11-11: the G-SURF-I numbers in this section were measured on code from before the units fix and are the bug, not a change of K7; see `gsurfi-reread-2026-11-11.md`.**

The tile (65, 7) of face 1 seed 14 is built as `k7_region` builds it (library code: lattice, regional solve, H1 channels, K7 with the channels burned in; 17.3 s). The gate is measured by own code: Catmull-Rom read with the clamp to the four cells, `L = z - B` quantised to i16 steps of 0.25 m, the vertices of the store lattice (every 6 local cells, `B` the lattice's stored node value), the cell centres.

| Quantity | Own | Library `surface_gate` | Ledger rows (ea6b1b2b) |
|---|---|---|---|
| Stored change against `(z - b)` as f32, 1,048,576 core cells | 0 differ; |L| beyond i16 steps: 0 | - | - |
| G-SURF at the centres, before quantisation | 0.0e0 m | 0.0e0 m | 5.7e-14 m |
| G-SURF at the centres, after quantisation | 0.1250 m | 0.1250 m | 0.125 m |
| Vertices | 28,730 | 28,900 | 28,900 |
| Vertices over 0.25 m | 179 (0.62 %) | 179 | 192 (0.66 %) |
| p50 / p95 / p99 of the quantised difference | 0.0400 / 0.1318 / 0.2192 m | 0.0400 / 0.1322 / 0.2192 m | 0.040 / 0.134 / 0.225 m |
| Maximum, unquantised / quantised | 2.6401 / 2.7565 m | 2.6401 / 2.7565 m | 2.55 m |
| By bake step (up to 5 m, to 20 m, over 20 m): vertices, over 0.25 m | 28,702, 28, 0; 176, 3, 0 | 28,871, 29, 0; 176, 3, 0 | 3,333, 10,895, 14,672 over 0.25 m: 0, 20, 172 |
| Largest difference over step height | 0.147 | 0.147 | at most 0.098 |
| Overshoot of the runtime read beyond the four composed cells | 2.6329 m, 69 vertices over 0.25 m | same | - |
| `B` at the vertices, own lattice value against the library's `base` | 0.0e0 m | - | - |

Findings:
- **The own measurement equals the library's** on every number that does not depend on the vertex set (p50, p99, the 179, the maximum, the overshoot).
- **The vertex sets differ by 170.** The library takes vertices `6 k` against window cell indices without the half cell (170 x 170); the probe takes the cell-index coordinate `6 k - 0.5` inside [66, 1085] (169 x 170). The vertices over 0.25 m are the same 179. A3 12.1's "inside the core, two cells in from its edge" does not fix this.
- **The G-SURF-I numbers moved since the ledger rows** (192 over, maximum 2.55 m, difference over step 0.098). K7's burn-in changed between ea6b1b2b and 760bf0e7 (59a7deaa, the width law, GEN-165). The cause of the move was not isolated (no run on the old commit).
- **The step classes do not match.** The ledger's classes (3,333 / 10,895 / 14,672 vertices, summing to the 28,900) are not the output of the current code's `nodes_by_step` on this tile (28,871 / 29 / 0, the largest `|z - z(here)|` within 2 cells). Either the rows use another step definition or another run; the probe did not find out which. The rows are worth a re-read on current main.
- Counter: `K7Result.burned` is 95,820,316 on a window of 1,327,104 cells. It counts something other than distinct cells (not investigated).
- The overshoot of the runtime read is 2.63 m at the worst vertex. A3 12.1 asks for "no uncontrolled overshoot"; the judgement is the architect's.

## 4. Reproduce

```
cargo build --release -j 4 -p planet-generators --example gate_probe --example gate_surf   # from spike/gate-remeasure
gate_probe gates R3a 4 368 987 4720 5339 15 200 8      # then R3b: 2 9968 10587 8304 8923 15, R1: 1 8056 8675 760 1379 14
gate_probe local R3a 4 368 987 4720 5339 15 200 8
gate_surf 200 8
```

The probe is reusable for G-ROUND, G-PART, G-DET and G-LOCAL after GEN-166's f32 rounds and after the memo-key fix (L3 with a rule flip).
