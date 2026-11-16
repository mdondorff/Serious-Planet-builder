# E3: the D-1 table (lead #160, owner 2026-11-15)

One table over five windows for the D-1 choice among the E3 variants.
- **Gate columns:** every store is read through one `cargo xtask river-gate --table` build (main b1d99baf, `--no-key-check`, 37 rows, 7,955 s). The full gate CSV is `table-d1-gate.csv`.
- **Ridge columns:** from spikes' SPIKE-122 addendum: `spikes/e3-variants-def/results/d1-table-spikes.csv` for the ridge lowering, and `raw/ridge-split.csv` (#1549) for the cut-through saddles.
- **Variants:**
  - A, one round at L (crusader's binary, rebuilt here bit for bit);
  - B, K dt x 1/4 (SPIKE-121);
  - D, E and F as spikes baked them, cap m = 2 (SPIKE-122);
  - spikes' fixed forms on R1h and M1: D/E regcap, D/E regcap+crest, D delta (the minimal-change hand-over);
  - D at cap m = 1 and 3 on R1h.
- **Bars (P3, set before the results):**
  - G-DRAIN: basins at most 10 %;
  - G-INCISION: the valley depth share over 50 m at most R1h 1.80, R3a 0.10, R3b 0.10, M1 40.80, F1 0.10 %;
  - the trunk corridor (#159): no pass share is set; the owner sets it from these numbers.

## Columns, in the lead's order
| Column | Definition | Source |
|---|---|---|
| Ridge > 20 m before / after K7, % | Ridge cells of today's start (core, land, at least 2 m above both cells 2 cells away on an axis or diagonal) lowered more than 20 m, on the surface K7 starts from and on the baked store | spikes' CSV |
| Saddles before K7 | Distinct cut-through saddles before K7 (ridge cells lowered over 20 m with a new 1 km2 channel within one cell), the lead's ranking measure | `ridge-split.csv` |
| Ground > 50 m below start, % | Land cells inside whose surface lies more than 50 m below the start (`lowered_pct`) | gate |
| Relief 1 km, m | p50 of the height range over 10 x 10 cells (about 1 km), store / start, over the counted tiles | gate |
| Corridor k_c 2, raw / snapped, % | Share of H1 trunk reaches whose minimax route inside a 1.6 km corridor stays within the window's bar (R1h 27.5, R3a 10, R3b 10, M1 50, F1 10 m); raw H1 ends, and snapped nodes and edge crossings | gate (#1544, #1546) |
| Edge 2 km, % | H1's edge crossings matched by an L crossing within 2 km (a reported number and a tie-break, not a bar) | gate |
| G-DRAIN, G-INCISION | Basins %; valley depth share over 50 m %; each against its bar | gate |
| Bake s, peak GB | From the bake logs only (lead, 2026-11-16: no re-bakes); everything else "not recorded" | explorer's and crusader's logs |

## The table
| Window | Variant | Ridge > 20 m before K7 % | after K7 % | Saddles before K7 | Ground > 50 m below start % | Relief 1 km store / start, m | Corridor k_c 2 raw % | snapped % | Edge 2 km % | G-DRAIN basins % | G-INCISION valley > 50 m % | Bake s | Peak GB |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| R1h | A | 5.66 | 30.12 | 1156 | 14.39 | 76.86 / 60.66 | 96.34 | 97.19 | 77.27 | 0.58 (pass) | 1.26 (pass, bar 1.80) | 508 | 5.22 |
| R1h | B | 2.17 | 24.21 | 764 | 8.48 | 69.56 / 60.66 | 96.28 | 96.93 | 86.36 | 0.47 (pass) | 1.57 (pass, bar 1.80) | 435 | not recorded |
| R1h | D | 14.36 | 42.2 | 3708 | 7.35 | 65.59 / 60.66 | 97.58 | 98.53 | 97.73 | 2.37 (pass) | 0.49 (pass, bar 1.80) | not recorded | not recorded |
| R1h | E | 17.36 | 45.47 | 3696 | 7.32 | 64.06 / 60.66 | 98.06 | 99.16 | 95.45 | 2.21 (pass) | 0.22 (pass, bar 1.80) | not recorded | not recorded |
| R1h | F | 10.16 | 27.03 | 5010 | 10.94 | 76.95 / 60.66 | 95.02 | 97.84 | 93.18 | 0.44 (pass) | 0.33 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D cap m=1 | n/a | n/a | n/a | 7.34 | 65.47 / 60.66 | 97.81 | 98.53 | 97.73 | 4.42 (pass) | 0.42 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D cap m=3 | n/a | n/a | n/a | 7.35 | 65.61 / 60.66 | 97.70 | 98.49 | 97.73 | 1.72 (pass) | 0.47 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D regcap | 14.36 | 42.19 | n/a | 7.06 | 62.46 / 60.66 | 98.60 | 98.07 | 97.73 | 12.30 (**fail**) | 0.20 (pass, bar 1.80) | not recorded | not recorded |
| R1h | E regcap | 17.35 | 45.46 | n/a | 7.05 | 60.88 / 60.66 | 98.98 | 98.65 | 95.45 | 10.54 (**fail**) | 0.10 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D regcap+crest | 14.26 | 42.05 | n/a | 7.06 | 62.56 / 60.66 | 98.53 | 98.09 | 97.73 | 12.46 (**fail**) | 0.39 (pass, bar 1.80) | not recorded | not recorded |
| R1h | E regcap+crest | 17.32 | 45.42 | n/a | 7.05 | 60.90 / 60.66 | 99.02 | 98.75 | 95.45 | 10.48 (**fail**) | 0.10 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D delta | 0.26 | 21.32 | n/a | 6.22 | 67.05 / 60.66 | 96.09 | 95.57 | 88.64 | 16.53 (**fail**) | 2.62 (**fail**, bar 1.80) | not recorded | not recorded |
| R3a | A | 0.89 | 4.22 | 4 | 0.38 | 14.39 / 9.97 | 99.75 | 99.83 | 25.00 | 0.05 (pass) | 0.08 (pass, bar 0.10) | 460 | not recorded |
| R3a | B | 0.34 | 2.51 | 2 | 0.08 | 12.48 / 9.97 | 99.92 | 99.83 | 26.19 | 0.00 (pass) | 0.02 (pass, bar 0.10) | 408 | not recorded |
| R3a | D | 1.51 | 8.94 | 44 | 0.06 | 11.13 / 9.97 | 100.00 | 99.97 | 32.14 | 1.80 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3a | E | 1.53 | 9.04 | 43 | 0.06 | 11.19 / 9.97 | 100.00 | 99.96 | 33.33 | 7.19 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3a | F | 3.52 | 6.13 | 232 | 0.40 | 12.66 / 9.97 | 99.66 | 99.88 | 35.71 | 0.97 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3b | A | 4.57 | 15.53 | 239 | 0.92 | 13.71 / 10.48 | 98.61 | 99.30 | 68.82 | 0.25 (pass) | 0.01 (pass, bar 0.10) | 501 | not recorded |
| R3b | B | 2.74 | 9.88 | 228 | 0.29 | 12.34 / 10.48 | 98.78 | 99.36 | 69.89 | 0.03 (pass) | 0.01 (pass, bar 0.10) | 448 | not recorded |
| R3b | D | 8.69 | 28.57 | 941 | 0.19 | 11.61 / 10.48 | 98.87 | 99.65 | 72.04 | 1.73 (pass) | 0.01 (pass, bar 0.10) | not recorded | not recorded |
| R3b | E | 8.75 | 28.64 | 946 | 0.19 | 11.59 / 10.48 | 98.86 | 99.65 | 74.19 | 1.97 (pass) | 0.01 (pass, bar 0.10) | not recorded | not recorded |
| R3b | F | 14.13 | 25.15 | 3302 | 1.26 | 14.14 / 10.48 | 96.09 | 98.60 | 69.89 | 0.41 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| M1 | A | 35.94 | 81.87 | 16665 | 68.59 | 423.62 / 371.53 | 89.14 | 95.44 | 81.71 | 3.50 (pass) | 16.49 (pass, bar 40.80) | 2007 | not recorded |
| M1 | B | 9.58 | 75.06 | 12506 | 56.08 | 402.67 / 371.53 | 88.92 | 94.63 | 79.27 | 3.32 (pass) | 22.23 (pass, bar 40.80) | 1811 | not recorded |
| M1 | D | 41.92 | 83.39 | 25670 | 54.17 | 390.12 / 371.53 | 92.31 | 95.29 | 85.37 | 9.09 (pass) | 14.37 (pass, bar 40.80) | not recorded | not recorded |
| M1 | E | 49.09 | 84.82 | 25709 | 53.12 | 385.60 / 371.53 | 94.19 | 97.37 | 84.15 | 6.54 (pass) | 11.71 (pass, bar 40.80) | not recorded | not recorded |
| M1 | F | 18.58 | 57.44 | 18778 | 48.81 | 423.72 / 371.53 | 87.47 | 95.13 | 79.27 | 1.72 (pass) | 16.41 (pass, bar 40.80) | not recorded | not recorded |
| M1 | D regcap | 40.57 | 81.38 | n/a | 51.61 | 379.87 / 371.53 | 93.98 | 95.21 | 85.37 | 13.40 (**fail**) | 15.72 (pass, bar 40.80) | not recorded | not recorded |
| M1 | E regcap | 47.65 | 82.72 | n/a | 50.74 | 375.32 / 371.53 | 96.39 | 97.05 | 84.15 | 11.42 (**fail**) | 10.75 (pass, bar 40.80) | not recorded | not recorded |
| M1 | D regcap+crest | 39.93 | 80.92 | n/a | 51.53 | 379.89 / 371.53 | 94.15 | 95.04 | 86.59 | 11.98 (**fail**) | 17.18 (pass, bar 40.80) | not recorded | not recorded |
| M1 | E regcap+crest | 47.45 | 82.59 | n/a | 50.72 | 375.35 / 371.53 | 96.45 | 96.94 | 85.37 | 10.93 (**fail**) | 10.93 (pass, bar 40.80) | not recorded | not recorded |
| M1 | D delta | 2.63 | 68.54 | n/a | 48.91 | 398.53 / 371.53 | 88.36 | 91.08 | 82.93 | 19.77 (**fail**) | 27.23 (pass, bar 40.80) | not recorded | not recorded |
| F1 | A | 1.84 | 4.74 | 42 | 0.07 | 14.39 / 11.49 | 96.06 | 98.33 | 34.48 | 0.18 (pass) | 0.00 (pass, bar 0.10) | 549 | not recorded |
| F1 | B | 1.14 | 2.57 | 46 | 0.01 | 13.26 / 11.49 | 96.15 | 98.33 | 34.48 | 0.00 (pass) | 0.00 (pass, bar 0.10) | 483 | not recorded |
| F1 | D | 2.02 | 8.67 | 334 | 0.01 | 12.40 / 11.49 | 97.12 | 99.62 | 44.83 | 4.50 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| F1 | E | 2.09 | 8.81 | 365 | 0.01 | 12.25 / 11.49 | 97.22 | 99.63 | 49.43 | 5.17 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| F1 | F | 19.27 | 25.44 | 5122 | 0.24 | 13.80 / 11.49 | 98.34 | 98.88 | 43.68 | 1.26 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |

## Reading
- **Gates:** A, B, D, E and F as baked pass G-DRAIN and G-INCISION separately on all five windows. Every fixed D/E form (regcap, regcap+crest, delta) fails G-DRAIN on R1h and M1 (10.5 to 19.8 %), and D delta also fails G-INCISION on R1h.
- **Ridges before K7:**
  - B cuts the fewest ridge cells on every window, and the fewest saddles on all but F1 (A 42, B 46).
  - A comes next. D and E cut 1.5 to 11 times A's saddles (M1 1.5 times, R3a 11 times). F has the most saddles on R1h, R3a, R3b and F1.
- **Ridges after K7:** K7 alone lowers 20 to 23 % of R1h's ridge cells and two thirds of M1's in every variant (spikes; flagged for E4). After K7, B is lowest on R1h, R3a, R3b and F1; on M1, F is lowest (57 % against B's 75 %). D and E stay highest on every window.
- **Ground lowered over 50 m:** R1h 7 to 14 % (A 14 %, F 11 %, B 8 %, D and E 7 %); R3a, R3b and F1 at most 1.3 %. On M1 it is 49 to 69 %: A 69 %, B 56 %, D and E 53 to 54 %, F 49 %.
- **Relief:** every variant raises 1 km relief above the start, M1 most (A and F about 424 m against 372 m).
- **Corridor at k_c 2, snapped:**
  - E is highest on R1h (99.2 %) and M1 (97.4 %); D and E tie on R3a, R3b and F1.
  - B is at or below A everywhere but R3b.
  - On M1 every variant leaves 2.6 to 5.4 % of reaches over 50 m. Per #159 that is a **D-2 input** (H1 or KR must adjust on mountains), not a D-1 blocker.
- **Edge agreement:** set by the window more than the variant. D or E is highest on R1h, R3b, M1 and F1 (R1h: D 97.7 %, E 95.5 %, A 77.3 %); F is highest on R3a (35.7 %).
- **Cost:** recorded for A and B only; it decides only a tie.

## Cross-sections (T1 to T3)
- [R1h](images/xsec-abcdef-r1h.png)
- [R3a](images/xsec-abcdef-r3a.png)
- [R3b](images/xsec-abcdef-r3b.png)
- [M1](images/xsec-abcdef-m1.png)
- [F1](images/xsec-abcdef-f1.png)

These are spikes' figures of A to F against today's surface and a GLO-30 valley (SPIKE-121 results, Visuals).
