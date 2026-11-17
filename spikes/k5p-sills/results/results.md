# SPIKE-131: K5' (K5's rounds on sill edges)

Opened: 2026-11-17 (lead #133, DEC spikes-2 ruled A)
Question: does K5', K5's regional rounds routed on node heights and sill edges, make SPIKE-127's stop test "no" on all five windows, and how much of that test is a measurement artefact?
Why it matters: river graphs G1a's stop rule (plan §4.2, addendum §5): K5' before G1c (#132).
Time box: 1 day (lead #133). Used: about 0.4 day.
Success criteria: K5' on five windows, M1 first; G1a's measures repeated on K5' + K6'; the lost confluences split into artefact and real re-routing (lead #133).
Fallback if it fails: a stated reason why K5' cannot fix the stop test (lead #133 item 4). Used: section 3.
Code location: spike branch `spike/k5p-sills` (a5a4ecc0, `crates/cache/examples/k5p_sills.rs`; SPIKE-127's `g1a_sills.rs` with the K5' rounds and the split stop test).

## 1. Method
| Item | Definition |
|---|---|
| `r` | `B - C1(z_b)` at L (the base's band under 814 m) |
| K5' node height | `h = z_s + r_min`, `r_min` the lowest L cell of `r` in the 814 m cell (static) |
| K5' sill | `s_r + max(z_s(p), z_s(q))`, `s_r` the minimax of `r` over 8-connected L paths between the two cells' lowest `r` cells, inside the two cells (static) |
| K5' round | K5's round (`solve_rounds_with`, 4 rounds of 5 steps, the same carve parameters) with its routing replaced by SPIKE-127's sill routing on `h` and the sills; the carve acts on `h` (the surface it routes on); `z_s' = h - r_min` |
| Bit check | the same round loop with today's `route_rules`: 0 of 1,040,400 cells differ from `solve_region` on M1 |
| `S0'` | `B + C1(z_s' - z_b)`: K5''s start surface |
| K5' + K6' | SPIKE-127's K6' on the node heights and sills of `S0'` |
| K5''s channels | its routing rule on its final `z_s'` with K6's weights, cells over 75 km2 |
| Stop test, split | as SPIKE-127 (top 10 rivers leaving the box, 20 main-stem cells or more; major confluence: tributary D8 area >= 75 km2 and >= 10 % of the stem's; no confluence of the other graph within 2 cells with a segment of 0.5-2x the area), and each confluence lost or gained is classed: **moved** when the tributary's water on the other graph reaches the same river (its main stem within 1 cell, or its exit within 2 cells) further on; **real** when it reaches an outlet or the box's edge first (a tributary to another river). "Via an MFD-split line": the moved path crosses a trunk cell with a D8 area under 75 km2 first. A river changes really when its outlet moves more than 8 km or a confluence changes really |
| Everything else | SPIKE-127's definitions (corridor k_c 2 snapped, land-only snap; D-2 re-trace; perturbations; the control and the `z_s` +-0.5 m baseline) |

## 2. Results, five windows
| Measure | R1h | R3a | R3b | M1 | F1 |
|---|---|---|---|---|---|
| Stop test, rivers changed / changed really: K6' (SPIKE-127) | 7 / **0** | 4 / 2 | 3 / **0** | 9 / **0** | 4 / **0** (of 9) |
| The same, K5' + K6' | 9 / 2 | 8 / 3 | 6 / 2 | 9 / 4 | 4 / 0 (of 9) |
| The same, control / `z_s` +-0.5 m baseline | 1 / 0, 1 / 0 | 4 / 0, 8 / 2 | 0 / 0, 1 / 1 | 0 / 0, 0 / 0 | 0 / 0, 0 / 0 |
| Confluences lost real + moved (via MFD-split line), K6' | 0 + 10 (2) | 1 + 8 (2) | 0 + 5 (0) | 0 + 17 (4) | 0 + 5 (1) |
| The same, K5' + K6' | 5 + 17 (3) | 3 + 8 (2) | 7 + 8 (1) | 14 + 15 (5) | 0 + 6 (0) |
| Confluences gained real + moved, K6' / K5' + K6' | 0 + 3 / 2 + 9 | 0 + 0 / 0 + 7 | 0 + 3 / 0 + 10 | 0 + 7 / 0 + 20 | 0 + 3 / 0 + 2 |
| `z_s' - z_s` on the box's land p10 / p50 / p90, m | -32 / 7 / 107 | -5 / 0.7 / 17 | -9.5 / 1.8 / 29 | -167 / 58 / 365 | -9 / 1.4 / 19 |
| Mean erosion `z_b - z_s`, K5 -> K5', m | 101 -> 78 | 11.5 -> 7.9 | 15.0 -> 8.7 | 508 -> 426 | 7.5 -> 3.8 |
| Trunk cells more than 1 cell off: today -> K5' + K6' | 31.3 % | 27.7 % | 25.0 % | 32.4 % | 21.0 % |
| K5' + K6' trunk cells off K5''s channels: exact / more than 1 cell | 29.2 / 3.3 % | 27.6 / 4.1 % | 26.0 / 2.0 % | 39.3 / 8.9 % | 16.1 / 0.9 % |
| K6' (SPIKE-127) off K5's channels, more than 1 cell | 5.7 % | 5.3 % | 4.0 % | 14.5 % | 1.0 % |
| Box land to an outlet over 8 km away, today -> K5' + K6' | 11.8 % | 4.1 % | 3.6 % | 7.4 % | 1.0 % |
| Corridor pass: today on `S0` / K6' on `S0` / K5' + K6' on `S0'` | 88.6 / 87.3 / 81.9 % | 99.1 / 99.2 / 97.9 % | 90.2 / 88.3 / 83.5 % | 68.5 / 63.0 / 62.1 % | 93.7 / 93.2 / 90.8 % |
| D-2 failing under the land-only snap / gain a route on K6': B, `S0` | 56 / 27, 2 | 4 / 1, 1 | 13 / 7, 1 | 427 / 269, 91 | 65 / 30, 22 |
| D-2 gain a route on K5' + K6': B, `S0`, `S0'` | 18, 1, 0 | 1, 1, 1 | 2, 1, 4 | 179, 84, 113 | 28, 23, 42 |
| Trunk heads today / K6' / K5' + K6' (share of K5' + K6''s heads with a D8 area under 75 km2) | 1,792 / 2,948 / 2,603 (91 %) | 5,394 / 7,150 / 6,740 (98 %) | 3,288 / 4,756 / 4,255 (94 %) | 3,614 / 5,249 / 5,028 (89 %) | 4,982 / 5,059 / 4,937 (87 %) |
| Kept H1 basins today / K6' / K5' + K6' | 69 / 236 / 337 | 227 / 195 / 328 | 61 / 115 / 178 | 106 / 230 / 295 | 224 / 212 / 260 |

- Five windows, D-2: K6' gains 334 of 565 on B and 117 on `S0`; K5' + K6' gains 228 on B, 110 on `S0`, 160 on `S0'`. B's store was carved by today's K5: the B column favours graphs near today's trunks.
- Full logs `raw/run-<w>.txt` (the `G1A stop test` rows name every confluence, its class and steps); per D-2 reach and graph `raw/d2-<w>.csv`.

### 2.1 Sensitivity of K5' + K6'
| Trunk cells more than 1 cell off / rivers changed really | R1h | R3a | R3b | M1 | F1 |
|---|---|---|---|---|---|
| Today, `z_s` +-0.5 m (baseline) | 1.5 % / 0 | 5.7 % / 2 | 1.1 % / 1 | 0.1 % / 0 | 1.0 % / 0 |
| K5' + K6', `S0'` +-0.5 m | 1.5 % / 0 | 4.0 % / 1 | 1.5 % / 2 | 0.3 % / 0 | 3.3 % / 0 |
| K5' + K6', sills +-1 m | 4.2 % / 0 | 5.5 % / 1 | 3.1 % / 0 | 2.0 % / 0 | 5.7 % / 0 |

### 2.2 Cost (8 threads; machine shared with other lanes' runs)
| Item | R1h | R3a | R3b | M1 | F1 |
|---|---|---|---|---|---|
| K5 (`solve_region`), s | 13.2 | 5.4 | 6.9 | 8.9 | 9.4 |
| K5': sills of `r` + rounds, s | 3.9 + 4.7 | 3.2 + 4.2 | 3.4 + 5.3 | 3.3 + 7.0 | 3.4 + 5.9 |
| Process peak (`B`, `r`, `S0`, `S0'`, B's ground, water copies), GB | 3.67 | 3.70 | 3.69 | 3.70 | 3.69 |
- K5' costs about K5 plus one sill pass over `r` (the same pass as SPIKE-127's, about 6 min a face) and 46 MB of sill arrays a window; per round the flood and routing replace K5's.

## 3. Findings
1. **The stop test was driven by artefacts.** With real changes only (an outlet moved over 8 km, or a tributary reaching another river), K6' alone changes no top river on R1h, R3b, M1 and F1; on R3a it changes 2, the same as R3a's own `z_s` +-0.5 m baseline (2). All 37 lost confluences of SPIKE-127's K6' on those four windows are the same tributaries joining the same river further on (7 of them through an MFD-split line first).
2. **K5' cannot fix the stop test: there is nothing to fix, and K5' itself re-routes.** K5' + K6' changes 2-4 top rivers really on R1h, R3a, R3b and M1 (29 tributaries to another river), against 0 for K6' alone on all but R3a.
3. **K5' changes K5's surface, not only its routing.** It erodes 16-49 % less (mean `z_b - z_s`), moves `z_s` by p50 0.7-58 m (M1 p90 365 m), and moves 21-32 % of the trunk cells more than one cell from today's.
4. **K5''s start surface dams more.** Corridor pass on `S0'` is 1.2-6.7 points below today's on `S0` and 0.9-5.4 below K6' on `S0`.
5. **K5' agrees with its own channels** (3.3, 4.1, 2.0, 8.9, 0.9 % more than one cell off, all under 10 %), but that agreement is K5' with itself; K6' alone already passes the rule against today's K5 on four windows (M1 14.5 %).
6. **D-2:** on `S0'` K5' + K6' gains 160 routes (K6' on `S0` 117); on B fewer (228 against 334), since B was carved along today's trunks.
7. **Stability:** K5' + K6' under `S0'` +-0.5 m and sills +-1 m is as stable as today's K6 under `z_s` +-0.5 m (0-2 rivers changed really).

## 4. Recommendation
- The exit of #133 by its second branch: K5' cannot fix the stop test, because the test was an artefact and K5' introduces real re-routing and more damming.
- #132 (G1c) on K6''s trunks (SPIKE-127), with today's K5; K5' not adopted.
- The stop test in the plan counts real changes only (outlet over 8 km, tributary to another river), beside the control and the noise baseline; moved confluences are reported apart.
- M1's K5-channel share (14.5 % more than one cell off) stays a reported number: all of M1's lost confluences are moved, none real.

## Visuals
None: the measures are graph topology, counts and corridor shares; SPIKE-127's maps (`spikes/g1a-sills/results/images/`) show K6''s trunks, and K5' is not recommended.
