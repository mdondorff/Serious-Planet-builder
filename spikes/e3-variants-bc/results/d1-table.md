# E3: the D-1 table (lead #160, owner 2026-11-15; corridor re-run after DEC explorer-4 and -5, 2026-11-16)

One table over five windows for the D-1 choice among the E3 variants.
- **Gate columns:** every store is read through one `cargo xtask river-gate --table` build: main cb5a504d, `--no-key-check`, 42 rows. The full gate CSV is `table-d1-gate.csv`.
- **Corridor columns:** these supersede the first table's (#1550). The corridor measure now routes over a store's counted tiles only (DEC explorer-4, #1551). Before, rises in the window's margin, where a store holds the start surface, counted against every variant alike. It also routes and snaps on the water surface (DEC explorer-5, #1555), since a snapped coastal node had landed on sea floor.
- **Ridge columns:** spikes' SPIKE-122 addendum, `spikes/e3-variants-def/results/d1-table-spikes.csv`; the cut-through saddles from `raw/ridge-split.csv` (#1549); G's from `spikes/e3-variant-g/results/d1-rows.csv` (#1553).
- **Variants:**
  - A, one round at L (crusader's binary, rebuilt here bit for bit);
  - B, K dt x 1/4 (SPIKE-121);
  - D, E and F as spikes baked them, cap m = 2 (SPIKE-122);
  - G, B on E's valley-aware base (SPIKE-125);
  - spikes' fixed forms on R1h and M1: D/E regcap, D/E regcap+crest, D delta (the minimal-change hand-over);
  - D at cap m = 1 and 3 on R1h.
- **Bars (P3, set before the results):**
  - G-DRAIN: basins at most 10 %;
  - G-INCISION: the valley depth share over 50 m at most R1h 1.80, R3a 0.10, R3b 0.10, M1 40.80, F1 0.10 %;
  - the trunk corridor (#159): no pass share is set; the owner sets it from these numbers.

## Columns, in the lead's order
| Column | Definition | Source |
|---|---|---|
| Ridge > 20 m before / after K7, % | Ridge cells of today's start (core, land, at least 2 m above both cells 2 cells away on an axis or diagonal) lowered more than 20 m, on the surface K7 starts from and on the baked store | spikes |
| Saddles before K7 | Distinct cut-through saddles before K7 (ridge cells lowered over 20 m with a new 1 km2 channel within one cell); the lead's ranking measure | spikes |
| Ground > 50 m below start, % | Land cells inside whose surface lies more than 50 m below the start (`lowered_pct`) | gate |
| Relief 1 km, m | p50 of the height range over 10 x 10 cells (about 1 km), store / start, over the counted tiles | gate |
| Corridor k_c 2, raw / snapped, % | Share of H1 trunk reaches whose minimax route, over the store's counted tiles inside a 1.6 km corridor, stays within the window's bar (R1h 27.5, R3a 10, R3b 10, M1 50, F1 10 m), on the water surface; raw H1 ends, and snapped nodes and edge crossings | gate (#1544, #1546, #1551, #1555) |
| Edge 2 km, % | H1's edge crossings matched by an L crossing within 2 km (a reported number and a tie-break, not a bar) | gate |
| G-DRAIN, G-INCISION | Basins %; valley depth share over 50 m %; each against its bar | gate |
| Bake s, peak GB | From the logs only (lead, 2026-11-16: no re-bakes); everything else "not recorded" | explorer, crusader, spikes (G) |

## The table
| Window | Variant | Ridge > 20 m before K7 % | after K7 % | Saddles before K7 | Ground > 50 m below start % | Relief 1 km store / start, m | Corridor k_c 2 raw % | snapped % | Edge 2 km % | G-DRAIN basins % | G-INCISION valley > 50 m % | Bake s | Peak GB |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| R1h | A | 5.66 | 30.12 | 1156 | 14.39 | 76.86 / 60.66 | 96.73 | 98.17 | 77.27 | 0.58 (pass) | 1.26 (pass, bar 1.80) | 508 | 5.22 |
| R1h | B | 2.17 | 24.21 | 764 | 8.48 | 69.56 / 60.66 | 96.69 | 97.86 | 86.36 | 0.47 (pass) | 1.57 (pass, bar 1.80) | 435 | not recorded |
| R1h | D | 14.36 | 42.2 | 3708 | 7.35 | 65.59 / 60.66 | 97.93 | 99.03 | 97.73 | 2.37 (pass) | 0.49 (pass, bar 1.80) | not recorded | not recorded |
| R1h | E | 17.36 | 45.47 | 3696 | 7.32 | 64.06 / 60.66 | 98.41 | 99.65 | 95.45 | 2.21 (pass) | 0.22 (pass, bar 1.80) | not recorded | not recorded |
| R1h | F | 10.16 | 27.03 | 5010 | 10.94 | 76.95 / 60.66 | 95.37 | 98.41 | 93.18 | 0.44 (pass) | 0.33 (pass, bar 1.80) | not recorded | not recorded |
| R1h | G | 4.08 | 28.65 | 1495 | 8.38 | 68.92 / 60.66 | 97.19 | 98.21 | 90.91 | 0.40 (pass) | 1.22 (pass, bar 1.80) | 397 | 5.25 |
| R1h | D cap m=1 | n/a | n/a | n/a | 7.34 | 65.47 / 60.66 | 98.16 | 99.01 | 97.73 | 4.42 (pass) | 0.42 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D cap m=3 | n/a | n/a | n/a | 7.35 | 65.61 / 60.66 | 98.05 | 98.99 | 97.73 | 1.72 (pass) | 0.47 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D regcap | 14.36 | 42.19 | n/a | 7.06 | 62.46 / 60.66 | 99.01 | 98.61 | 97.73 | 12.30 (**fail**) | 0.20 (pass, bar 1.80) | not recorded | not recorded |
| R1h | E regcap | 17.35 | 45.46 | n/a | 7.05 | 60.88 / 60.66 | 99.37 | 99.24 | 95.45 | 10.54 (**fail**) | 0.10 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D regcap+crest | 14.26 | 42.05 | n/a | 7.06 | 62.56 / 60.66 | 98.93 | 98.63 | 97.73 | 12.46 (**fail**) | 0.39 (pass, bar 1.80) | not recorded | not recorded |
| R1h | E regcap+crest | 17.32 | 45.42 | n/a | 7.05 | 60.90 / 60.66 | 99.41 | 99.34 | 95.45 | 10.48 (**fail**) | 0.10 (pass, bar 1.80) | not recorded | not recorded |
| R1h | D delta | 0.26 | 21.32 | n/a | 6.22 | 67.05 / 60.66 | 96.44 | 96.47 | 88.64 | 16.53 (**fail**) | 2.62 (**fail**, bar 1.80) | not recorded | not recorded |
| R3a | A | 0.89 | 4.22 | 4 | 0.38 | 14.39 / 9.97 | 99.78 | 99.91 | 25.00 | 0.05 (pass) | 0.08 (pass, bar 0.10) | 460 | not recorded |
| R3a | B | 0.34 | 2.51 | 2 | 0.08 | 12.48 / 9.97 | 99.92 | 99.90 | 26.19 | 0.00 (pass) | 0.02 (pass, bar 0.10) | 408 | not recorded |
| R3a | D | 1.51 | 8.94 | 44 | 0.06 | 11.13 / 9.97 | 100.00 | 99.98 | 32.14 | 1.80 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3a | E | 1.53 | 9.04 | 43 | 0.06 | 11.19 / 9.97 | 100.00 | 99.98 | 33.33 | 7.19 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3a | F | 3.52 | 6.13 | 232 | 0.40 | 12.66 / 9.97 | 99.72 | 99.91 | 35.71 | 0.97 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3a | G | 0.33 | 2.53 | 3 | 0.08 | 12.51 / 9.97 | 99.96 | 99.95 | 27.38 | 0.00 (pass) | 0.02 (pass, bar 0.10) | 395 | 5.04 |
| R3b | A | 4.57 | 15.53 | 239 | 0.92 | 13.71 / 10.48 | 98.91 | 99.65 | 68.82 | 0.25 (pass) | 0.01 (pass, bar 0.10) | 501 | not recorded |
| R3b | B | 2.74 | 9.88 | 228 | 0.29 | 12.34 / 10.48 | 99.05 | 99.66 | 69.89 | 0.03 (pass) | 0.01 (pass, bar 0.10) | 448 | not recorded |
| R3b | D | 8.69 | 28.57 | 941 | 0.19 | 11.61 / 10.48 | 99.15 | 99.91 | 72.04 | 1.73 (pass) | 0.01 (pass, bar 0.10) | not recorded | not recorded |
| R3b | E | 8.75 | 28.64 | 946 | 0.19 | 11.59 / 10.48 | 99.13 | 99.91 | 74.19 | 1.97 (pass) | 0.01 (pass, bar 0.10) | not recorded | not recorded |
| R3b | F | 14.13 | 25.15 | 3302 | 1.26 | 14.14 / 10.48 | 96.42 | 99.05 | 69.89 | 0.41 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| R3b | G | 2.66 | 9.77 | 245 | 0.28 | 12.39 / 10.48 | 99.08 | 99.66 | 70.97 | 0.06 (pass) | 0.01 (pass, bar 0.10) | 399 | 5.05 |
| M1 | A | 35.94 | 81.87 | 16665 | 68.59 | 423.62 / 371.53 | 89.67 | 95.90 | 81.71 | 3.50 (pass) | 16.49 (pass, bar 40.80) | 2007 | not recorded |
| M1 | B | 9.58 | 75.06 | 12506 | 56.08 | 402.67 / 371.53 | 89.38 | 95.07 | 79.27 | 3.32 (pass) | 22.23 (pass, bar 40.80) | 1811 | not recorded |
| M1 | D | 41.92 | 83.39 | 25670 | 54.17 | 390.12 / 371.53 | 92.67 | 95.86 | 85.37 | 9.09 (pass) | 14.37 (pass, bar 40.80) | not recorded | not recorded |
| M1 | E | 49.09 | 84.82 | 25709 | 53.12 | 385.60 / 371.53 | 94.52 | 97.93 | 84.15 | 6.54 (pass) | 11.71 (pass, bar 40.80) | not recorded | not recorded |
| M1 | F | 18.58 | 57.44 | 18778 | 48.81 | 423.72 / 371.53 | 87.78 | 95.74 | 79.27 | 1.72 (pass) | 16.41 (pass, bar 40.80) | not recorded | not recorded |
| M1 | G | 26.09 | 78.14 | 21615 | 55.06 | 401.24 / 371.53 | 90.36 | 96.14 | 81.71 | 2.95 (pass) | 18.24 (pass, bar 40.80) | 1923 | 5.72 |
| M1 | D regcap | 40.57 | 81.38 | n/a | 51.61 | 379.87 / 371.53 | 94.56 | 96.06 | 85.37 | 13.40 (**fail**) | 15.72 (pass, bar 40.80) | not recorded | not recorded |
| M1 | E regcap | 47.65 | 82.72 | n/a | 50.74 | 375.32 / 371.53 | 96.97 | 97.89 | 84.15 | 11.42 (**fail**) | 10.75 (pass, bar 40.80) | not recorded | not recorded |
| M1 | D regcap+crest | 39.93 | 80.92 | n/a | 51.53 | 379.89 / 371.53 | 94.72 | 95.86 | 86.59 | 11.98 (**fail**) | 17.18 (pass, bar 40.80) | not recorded | not recorded |
| M1 | E regcap+crest | 47.45 | 82.59 | n/a | 50.72 | 375.35 / 371.53 | 97.02 | 97.77 | 85.37 | 10.93 (**fail**) | 10.93 (pass, bar 40.80) | not recorded | not recorded |
| M1 | D delta | 2.63 | 68.54 | n/a | 48.91 | 398.53 / 371.53 | 88.69 | 91.53 | 82.93 | 19.77 (**fail**) | 27.23 (pass, bar 40.80) | not recorded | not recorded |
| F1 | A | 1.84 | 4.74 | 42 | 0.07 | 14.39 / 11.49 | 96.22 | 98.58 | 34.48 | 0.18 (pass) | 0.00 (pass, bar 0.10) | 549 | not recorded |
| F1 | B | 1.14 | 2.57 | 46 | 0.01 | 13.26 / 11.49 | 96.28 | 98.59 | 34.48 | 0.00 (pass) | 0.00 (pass, bar 0.10) | 483 | not recorded |
| F1 | D | 2.02 | 8.67 | 334 | 0.01 | 12.40 / 11.49 | 97.25 | 99.79 | 44.83 | 4.50 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| F1 | E | 2.09 | 8.81 | 365 | 0.01 | 12.25 / 11.49 | 97.35 | 99.80 | 49.43 | 5.17 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| F1 | F | 19.27 | 25.44 | 5122 | 0.24 | 13.80 / 11.49 | 98.61 | 99.33 | 43.68 | 1.26 (pass) | 0.00 (pass, bar 0.10) | not recorded | not recorded |
| F1 | G | 1.08 | 2.48 | 90 | 0.01 | 13.13 / 11.49 | 96.36 | 98.70 | 34.48 | 0.00 (pass) | 0.00 (pass, bar 0.10) | 447 | 4.59 |

## Reading
- **Gates:** A, B, D, E, F and G as baked pass G-DRAIN and G-INCISION separately on all five windows. Every fixed D/E form (regcap, regcap+crest, delta) fails G-DRAIN on R1h and M1 (10.5 to 19.8 %), and D delta also fails G-INCISION on R1h.
- **Ridges before K7:**
  - B cuts the fewest ridge cells on R1h and M1. G cuts marginally fewer on R3a, R3b and F1: 0.33 against 0.34 %, 2.66 against 2.74 %, 1.08 against 1.14 %.
  - B cuts the fewest saddles on all windows but F1 (A 42, B 46, G 90).
  - G sits between B and D/E: up to 2 times B's saddles (R1h 1,495 against 764; M1 21,615 against 12,506).
  - D and E cut 1.5 to 11 times A's saddles. F has the most saddles on R1h, R3a, R3b and F1.
- **Ridges after K7:** K7 alone lowers 20 to 23 % of R1h's ridge cells and two thirds of M1's in every variant (spikes; flagged for E4).
  - The lowest per window: B on R1h (24.2 %); G on R3b (9.8 %) and F1 (2.5 %); B and G level on R3a (2.5 %); F on M1 (57 % against B's 75 %).
  - D and E stay highest on every window.
- **Ground lowered over 50 m:**
  - R1h 7 to 14 %: A 14 %, F 11 %, B and G 8 %, D and E 7 %.
  - R3a, R3b and F1 at most 1.3 %.
  - M1 49 to 69 %: A 69 %, B 56 %, G 55 %, D and E 53 to 54 %, F 49 %.
- **Relief:** every variant raises 1 km relief above the start, M1 most (A and F about 424 m against 372 m).
- **Corridor at k_c 2, snapped** (counted tiles, water surface):
  - E is highest on R1h (99.6 %) and M1 (97.9 %), and level with D on R3a, R3b and F1.
  - G is between A and E. B and A are within 0.9 points: B is lower on R1h, R3a and M1, and higher by 0.01 on R3b and F1.
  - On M1 every variant leaves 2.1 (E) to 4.9 % (B) of reaches over 50 m. Per #159 that is a **D-2 input** (H1 or KR must adjust on mountains), not a D-1 blocker.
- **Edge agreement:** set by the window more than the variant.
  - D or E is highest on R1h, R3b, M1 and F1 (R1h: D 97.7 %, E 95.5 %, G 90.9 %, A 77.3 %).
  - F is highest on R3a (35.7 %).
- **Cost:** A, B and G bake in 400 to 550 s, M1 1,800 to 2,000 s. Peak memory is about 5 GB where recorded (A R1h, G everywhere). It decides only a tie.

## Cross-sections (T1 to T3)
- A to F, spikes' figures (SPIKE-121 Visuals): [R1h](images/xsec-abcdef-r1h.png), [R3a](images/xsec-abcdef-r3a.png), [R3b](images/xsec-abcdef-r3b.png), [M1](images/xsec-abcdef-m1.png), [F1](images/xsec-abcdef-f1.png).
- A, B and G (SPIKE-125): [R1h](../../e3-variant-g/results/images/xsec-g-r1h.png), [R3a](../../e3-variant-g/results/images/xsec-g-r3a.png), [R3b](../../e3-variant-g/results/images/xsec-g-r3b.png), [M1](../../e3-variant-g/results/images/xsec-g-m1.png), [F1](../../e3-variant-g/results/images/xsec-g-f1.png).
