# The channel network bar: drainage density and R_b on the GLO-30 benchmark (lead #163 item 2, 2026-11-17)

Order: lead #163 item 2 (river-graph plan, pre-work): drainage density with a channel-threshold parameter and the Strahler bifurcation ratio `R_b`, measured on the GLO-30 benchmark tiles and the five `S0` windows at 1 km2 and 75 km2; the bar rule's numbers fixed before any RG result.

## Method
- Code: BUILD-059 (`river-gate`'s network lines and `net1_*`, `net2_*` fields; `grid=file,w,h,sea` input). Definitions in `openspec/specs/build-tooling/spec.md` BUILD-059.
- One `cargo xtask river-gate --table` build (branch `change/river-network`, 3fbdc0f7), 30 rows, 32 min. Raw: `raw/network/network-163.csv`, `raw/network/network-163-lines.txt`.
- Tiles: SPIKE-119's prepared grids (section 3.1: L cells of 101.8 m, box mean, no pit filling), 21 GLO-30 and 4 3DEP DTM; sea level 0 m, Death Valley (N36 W117) -200 m; counted inside a 10-cell border.
- Windows: the start surface `S0` of R1h, R3a, R3b, M1 and F1, counted over the region's box.
- Thresholds: 1 km2 (the fine graph's `A_crit`) and 75 km2 (the gate's river cells).
- Relief: `relief10_start_m`, the p50 height range over 10 x 10 cells (about 1 km).
- Bar rule (#163): per window, the range over the GLO-30 tiles whose 1 km relief lies within 1 to 2 times the window's start relief. `R_b` ranges leave out tiles with fewer than 2 fitted orders. DTM tiles are a check of the DSM, not in the bar.

## Per row
| Row | Relief 1 km, m | Land km2 | Density 1 km2 | Streams 1 km2 | R_b 1 km2 (orders) | Density 75 km2 | Streams 75 km2 | R_b 75 km2 (orders) |
|---|---|---|---|---|---|---|---|---|
| glo-N27E086 | 486.6 | 10,553 | 0.623 | 2784 / 606 / 125 / 26 / 3 / 1 | 5.37 (5) | 0.0801 | 33 / 7 / 2 / 1 | 4.06 (3) |
| glo-N31W004 | 13.6 | 10,141 | 0.857 | 2695 / 633 / 142 / 30 / 6 | 4.48 (4) | 0.0971 | 37 / 9 / 1 | 4.11 (2) |
| glo-N33W092 | 15.2 | 9,908 | 0.827 | 2852 / 657 / 148 / 37 / 9 / 2 | 4.22 (5) | 0.1039 | 38 / 12 / 2 | 3.17 (2) |
| glo-N35W076 | 10.8 | 976 | 0.385 | 214 / 42 / 6 / 1 | 5.97 (3) | 0.0000 | - | 0.00 (0) |
| glo-N36W113 | 73.6 | 9,552 | 0.803 | 2382 / 536 / 121 / 27 / 7 / 1 | 4.33 (5) | 0.0852 | 39 / 9 / 2 / 1 | 4.42 (3) |
| glo-N36W117 | 75.5 | 9,552 | 1.105 | 2977 / 744 / 166 / 41 / 9 / 1 | 4.26 (5) | 0.0946 | 41 / 11 / 2 | 3.73 (2) |
| glo-N40W078 | 97.3 | 9,018 | 0.703 | 2390 / 537 / 138 / 26 / 7 / 1 | 4.35 (5) | 0.0928 | 32 / 8 / 1 | 4.00 (2) |
| glo-N41W096 | 34.4 | 8,874 | 0.724 | 2409 / 504 / 94 / 18 / 4 / 1 | 5.02 (5) | 0.1236 | 28 / 7 / 2 | 4.00 (2) |
| glo-N42W123 | 167.3 | 8,740 | 0.774 | 2416 / 560 / 141 / 34 / 10 / 1 | 3.96 (5) | 0.0767 | 35 / 9 / 1 | 3.89 (2) |
| glo-N43W102 | 29.4 | 8,596 | 0.776 | 2400 / 519 / 115 / 26 / 8 / 2 | 4.22 (5) | 0.0968 | 34 / 8 / 1 | 4.25 (2) |
| glo-N45E014 | 151.9 | 7,374 | 0.729 | 2092 / 476 / 110 / 23 / 5 / 1 | 4.53 (5) | 0.0663 | 26 / 7 / 2 | 3.71 (2) |
| glo-N46W122 | 268.0 | 8,140 | 0.707 | 2086 / 473 / 113 / 20 / 6 / 1 | 4.42 (5) | 0.0711 | 33 / 7 / 1 | 4.71 (2) |
| glo-N47E011 | 333.7 | 7,984 | 0.697 | 2087 / 468 / 104 / 21 / 7 / 2 | 4.26 (5) | 0.0850 | 24 / 7 / 2 | 3.43 (2) |
| glo-N48E008 | 95.5 | 7,828 | 0.715 | 2050 / 473 / 110 / 28 / 5 / 1 | 4.42 (5) | 0.0915 | 36 / 10 / 3 | 3.60 (2) |
| glo-N52E012 | 19.9 | 7,172 | 0.854 | 2054 / 508 / 115 / 33 / 7 / 2 | 4.09 (5) | 0.0970 | 31 / 7 / 2 / 1 | 3.94 (3) |
| glo-N57W005 | 127.6 | 6,064 | 0.667 | 1532 / 332 / 81 / 19 / 5 / 1 | 4.18 (5) | 0.0694 | 22 / 4 / 1 | 5.50 (2) |
| glo-N61E006 | 370.6 | 5,111 | 0.598 | 1295 / 299 / 69 / 12 / 1 | 4.72 (4) | 0.0385 | 16 / 2 | 0.00 (1) |
| glo-N70W154 | 9.6 | 2,722 | 0.646 | 724 / 174 / 37 / 6 | 4.42 (3) | 0.0635 | 7 / 2 | 0.00 (1) |
| glo-S04W061 | 18.6 | 11,909 | 0.853 | 3473 / 783 / 176 / 41 / 9 / 2 | 4.42 (5) | 0.0921 | 51 / 9 / 2 | 5.67 (2) |
| glo-S25E015 | 69.4 | 10,842 | 0.835 | 2685 / 560 / 128 / 33 / 9 / 2 / 1 | 4.14 (6) | 0.1102 | 50 / 13 / 4 / 1 | 3.54 (3) |
| glo-S44E170 | 430.2 | 7,958 | 0.746 | 2071 / 483 / 114 / 31 / 7 / 1 | 4.11 (5) | 0.0834 | 37 / 8 / 1 | 4.62 (2) |
| dtm-N33W092 | 4.7 | 9,908 | 0.859 | 2703 / 602 / 137 / 33 / 7 / 2 | 4.40 (5) | 0.1286 | 41 / 10 / 3 | 4.10 (2) |
| dtm-N36W113 | 73.5 | 9,552 | 0.805 | 2402 / 541 / 122 / 29 / 7 / 1 | 4.31 (5) | 0.0858 | 38 / 9 / 2 / 1 | 4.36 (3) |
| dtm-N40W078 | 91.8 | 9,018 | 0.689 | 2297 / 522 / 126 / 25 / 7 / 1 | 4.32 (5) | 0.0964 | 31 / 8 / 1 | 3.88 (2) |
| dtm-N46W122 | 271.0 | 8,140 | 0.705 | 2064 / 471 / 112 / 25 / 6 / 1 | 4.31 (5) | 0.0710 | 33 / 7 / 1 | 4.71 (2) |
| r1h | 54.9 | 175,883 | 0.656 | 55536 / 12198 / 2588 / 562 / 127 / 32 / 6 / 1 | 4.53 (7) | 0.0790 | 600 / 132 / 30 / 4 / 1 | 5.21 (4) |
| r3a | 9.3 | 207,258 | 0.679 | 65536 / 14505 / 3319 / 789 / 148 / 21 / 1 | 4.87 (6) | 0.0476 | 729 / 133 / 17 / 1 | 6.55 (3) |
| r3b | 10.7 | 226,070 | 0.664 | 73523 / 15711 / 3213 / 721 / 151 / 33 / 8 / 2 | 4.61 (7) | 0.0928 | 802 / 198 / 37 / 10 / 3 | 4.41 (4) |
| m1 | 378.7 | 254,962 | 0.859 | 80400 / 19010 / 4382 / 1009 / 260 / 62 / 14 / 3 / 1 | 4.25 (8) | 0.0870 | 969 / 237 / 53 / 11 / 2 | 4.45 (4) |
| f1 | 11.6 | 254,681 | 0.615 | 76656 / 16990 / 3506 / 814 / 204 / 51 / 8 | 4.33 (6) | 0.0973 | 949 / 242 / 62 / 10 | 3.91 (3) |

## Bar per window
| Window | Start relief 1 km, m | Tile band, m | Tiles | Density 1 km2 | R_b 1 km2 | Density 75 km2 | R_b 75 km2 | Start: D 1 / R_b 1 / D 75 / R_b 75 |
|---|---|---|---|---|---|---|---|---|
| r1h | 54.9 | 55 to 110 | 5: N36W113, N36W117, N40W078, N48E008, S25E015 | 0.703 to 1.105 (5) | 4.14 to 4.42 (5) | 0.085 to 0.110 (5) | 3.54 to 4.42 (5) | 0.656 / 4.53 / 0.079 / 5.21 |
| r3a | 9.3 | 9 to 19 | 5: N31W004, N33W092, N35W076, N70W154, S04W061 | 0.385 to 0.857 (5) | 4.22 to 5.97 (5) | 0.000 to 0.104 (5) | 3.17 to 5.67 (3) | 0.679 / 4.87 / 0.048 / 6.55 |
| r3b | 10.7 | 11 to 21 | 5: N31W004, N33W092, N35W076, N52E012, S04W061 | 0.385 to 0.857 (5) | 4.09 to 5.97 (5) | 0.000 to 0.104 (5) | 3.17 to 5.67 (4) | 0.664 / 4.61 / 0.093 / 4.41 |
| m1 | 378.7 | 379 to 757 | 2: N27E086, S44E170 | 0.623 to 0.746 (2) | 4.11 to 5.37 (2) | 0.080 to 0.083 (2) | 4.06 to 4.62 (2) | 0.859 / 4.25 / 0.087 / 4.45 |
| f1 | 11.6 | 12 to 23 | 4: N31W004, N33W092, N52E012, S04W061 | 0.827 to 0.857 (4) | 4.09 to 4.48 (4) | 0.092 to 0.104 (4) | 3.17 to 5.67 (4) | 0.615 / 4.33 / 0.097 / 3.91 |

## The bar (fixed before any RG result)
| Window | Tiles | Density 1 km2, km/km2 | R_b 1 km2 | Density 75 km2, km/km2 | R_b 75 km2 |
|---|---|---|---|---|---|
| R1h | 5 | 0.703 to 1.105 | 4.14 to 4.42 | 0.085 to 0.110 | 3.54 to 4.42 |
| R3a | 5 | 0.385 to 0.857 | 4.22 to 5.97 | 0.000 to 0.104 | 3.17 to 5.67 (3 tiles) |
| R3b | 5 | 0.385 to 0.857 | 4.09 to 5.97 | 0.000 to 0.104 | 3.17 to 5.67 (4 tiles) |
| M1 | 2 | 0.623 to 0.746 | 4.11 to 5.37 | 0.080 to 0.083 | 4.06 to 4.62 |
| F1 | 4 | 0.827 to 0.857 | 4.09 to 4.48 | 0.092 to 0.104 | 3.17 to 5.67 |

## Reading
- `S0` against its own bar (a baseline, not a result):

  | Window | Density 1 km2 | R_b 1 km2 | Density 75 km2 | R_b 75 km2 |
  |---|---|---|---|---|
  | R1h | 0.656, below | 4.53, above | 0.079, below | 5.21, above |
  | R3a | 0.679, inside | 4.87, inside | 0.048, inside | 6.55, above |
  | R3b | 0.664, inside | 4.61, inside | 0.093, inside | 4.41, inside |
  | M1 | 0.859, above | 4.25, inside | 0.087, above | 4.45, inside |
  | F1 | 0.615, below | 4.33, inside | 0.097, inside | 3.91, inside |

- `R_b` at 1 km2 is 3.96 to 5.97 on all 21 GLO-30 tiles and 4.25 to 4.87 on the five windows: Horton's usual 3 to 5 band, with the windows inside the tiles' spread.
- Density at 1 km2 is 0.385 to 1.105 km/km2 on the tiles, 0.598 to 1.105 without N35 W076, and 0.615 to 0.859 on the windows.
- DSM against DTM, the four 3DEP tiles: density at 1 km2 differs by 0.002 to 0.032, `R_b` at 1 km2 by 0.02 to 0.18. At 75 km2 density differs by up to 0.025 and `R_b` by up to 0.93 (N33 W092: 3.17 DSM, 4.10 DTM). The DSM's canopy moves the 1 km2 numbers little; the 75 km2 `R_b` is fragile either way.

## Caveats
- N35 W076 (Outer Banks) has 976 km2 of land: no channel reaches 75 km2, so the 75 km2 density range of R3a and R3b starts at 0. Without it, R3a reads 0.064 to 0.104 at 75 km2 and 0.646 to 0.857 at 1 km2; R3b 0.092 to 0.104 and 0.827 to 0.857.
- `R_b` at 75 km2 rests on 2 or 3 fitted orders with 7 to 51 first-order streams a tile: a coarse number. A window holds 175,883 to 254,962 km2 of land against at most 11,909 on a tile, and fits 3 or 4 orders.
- M1's band holds 2 tiles (Himalaya N27 E086, Southern Alps S44 E170); SPIKE-119 found no Earth valley matching M1's trunk.
- One tile is one routing on one 1-degree box: the area rule sees only the tile (SPIKE-119 section 3.4), so the highest orders are cut and large rivers are missing.

## Visuals
None: the measure is numbers on the benchmark's existing tiles; SPIKE-119's figures show the tiles.

Statistics derived from Copernicus DEM GLO-30: produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. 3DEP: U.S. Geological Survey, 3D Elevation Program 1 arc-second Digital Elevation Model (public domain).
