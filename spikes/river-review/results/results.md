# River review: the ADR 0123 variants on equal terms

For: the owner's external review of ADR 0123. Order: lead #138 item 2, 2026-11-14 (crusader). Pictures: [images/README.md](images/README.md) (spikes).

## Method
- **Windows** (620 regional cells a side, margin 200, L cells of 101.8 m):

  | Window | Face | Regional cells i, j | Seed |
  |---|---|---|---|
  | R1h | 1 | 8056-8675, 760-1379 | 14 |
  | R3a | 4 | 368-987, 4720-5339 | 15 |
  | R3b | 2 | 9968-10587, 8304-8923 | 15 |

  R1h is the E1 region's window (stores r3a-e1, r3a-p2, r3a-p2e).
- **Variants:**
  - today: B + regional;
  - round only: today + one round of the regional kernel at L;
  - D4 only: SPIKE-116's 4-neighbour hand-over;
  - D4 + round: the hand-over, then one round at L.
- **Tools:**
  - `erosion_at_l` builds each surface (spike branch spike/river-review, never merged).
  - Every surface is measured by main's river gate (`planet_cache::river_gate::measure`, BUILD-055) without changes.
  - The canyon split comes from `canyon_diag` on the same branch.
- **Schott-fair:** spikes' numbers (drain_base probe, SPIKE-118), in [table.csv](table.csv) where they have a column.

## Definitions
- **Basins:** river cells in closed basins at least 0.5 m deep and 16 cells (fill epsilon 1e-7 m); bar 10 %.
- **Valley depth (ring 1-2):** river cells more than 50 m below the p50 of the surface 1 to 2 cells out (102-204 m). This is the canyon a viewer sees (DEC crusader-4, lead 2026-11-14).
- **Valley depth (ring 2-4):** the same, 2 to 4 cells out (204-407 m). This is the gate's ring.
- **Burn below drained surface:** river cells whose surface stands more than 50 m above the river level (every pit breached). This is K7's cut, and SPIKE-117's "canyon".
- **Erosion depth (against the start surface):** river cells whose level stands more than 50 m below today's surface.
- **Lakes:** river cells in basins deeper than D = 20 m.
- **Relief:** p50 of the height range over 2 x 2 and 10 x 10 cells (m).
- **Edge:** H1 crossings of the region's edge (Q >= 1 m3/s) matched by an L crossing within 2 km.
- **Axis share:** channel steps along a grid axis (spikes' numbers).

## Numbers
All values are shares of river cells (%) except relief (m) and edge (count).

| Window | Variant | Basins | Valley depth (ring 1-2) | Valley depth (ring 2-4) | Burn below drained surface | Erosion depth | Lakes | Relief 2x2 / 10x10 | Edge | Axis share |
|---|---|---|---|---|---|---|---|---|---|---|
| R1h | today | 67.28 | 0.01 | 2.74 | 7.90 | 0.00 | 47.53 | 5.9 / 54.9 | 36 of 44 | 47.2 |
| R1h | round only | 0.98 | 5.29 | 16.50 | 0.00 | 30.75 | 0.00 | 6.7 / 61.6 | 36 of 44 | 47.4 |
| R1h | D4 only | 21.77 | 0.04 | 3.49 | 0.00 | 5.70 | 3.98 | 5.8 / 52.3 | 42 of 44 | 69.5 |
| R1h | D4 + round | 2.05 | 9.95 | 21.08 | 0.00 | 21.16 | 0.00 | 6.8 / 58.8 | 42 of 44 | 66.6 |
| R3a | today | 53.69 | 0.00 | 0.02 | 0.01 | 0.00 | 1.63 | 1.0 / 9.3 | 34 of 84 | 44.4 |
| R3a | round only | 1.78 | 0.20 | 0.76 | 0.00 | 1.10 | 0.00 | 1.2 / 11.0 | 33 of 84 | 42.6 |
| R3a | D4 only | 15.16 | 0.00 | 0.00 | 0.00 | 0.06 | 0.00 | 1.0 / 8.4 | 43 of 84 | 54.6 |
| R3a | D4 + round | 0.00 | 0.21 | 0.65 | 0.00 | 0.75 | 0.00 | 1.2 / 9.9 | 43 of 84 | 49.9 |
| R3b | today | 62.40 | 0.00 | 0.06 | 1.21 | 0.00 | 25.29 | 1.2 / 10.7 | 74 of 93 | 45.9 |
| R3b | round only | 4.40 | 0.74 | 3.20 | 0.00 | 6.00 | 0.00 | 1.3 / 12.1 | 74 of 93 | 45.5 |
| R3b | D4 only | 11.44 | 0.00 | 0.02 | 0.00 | 1.70 | 0.00 | 1.2 / 10.3 | 72 of 93 | 72.3 |
| R3b | D4 + round | 0.19 | 0.90 | 4.32 | 0.00 | 6.17 | 0.00 | 1.4 / 11.7 | 73 of 93 | 68.6 |

Schott-fair (spikes, same windows): basins 0.00 % on all three windows; axis share 56.7 / 46.1 / 53.5 %; relief 2x2 7.1 / 1.1 / 1.5 m.

## Earth reference (GLO-30)
GLO-30 tiles were resampled to square 101.8 m cells (the L cell), with river cells by the gate's rule. Valley depths are shares over 50 m.

| Tile | Relief 10x10 | Valley depth (ring 1-2) | Valley depth (ring 2-4) | Burn |
|---|---|---|---|---|
| Erg Chebbi N31W004 (flattest) | 13.6 | 0.10 | 1.32 | 0.00 |
| Brandenburg N52E012 | 19.9 | 0.00 | 0.00 | 0.00 |
| Badlands N43W102 | 29.4 | 0.00 | 0.00 | 0.00 |
| Loess hills N41W096 | 34.4 | 0.00 | 0.00 | 0.00 |
| Black Forest N48E008 | 95.5 | 1.82 | 30.83 | 0.00 |
| Valley and Ridge N40W078 | 97.3 | 0.06 | 2.97 | 0.00 |
| Scottish Highlands N57W005 | 127.6 | 0.06 | 3.09 | 0.00 |
| Alps N47E011 | 333.7 | 3.66 | 17.34 | 0.02 |
| Himalaya N27E086 (hilliest) | 486.6 | 40.78 | 84.48 | 0.53 |

Contains modified Copernicus Sentinel data [2021] processed by ESA (GLO-30).

## Cost
| Item | Time | Peak commit |
|---|---|---|
| Region front (shared by all variants) | 25-42 s | |
| One round at L, per window | 65-99 s (one run 238 s while another heavy job ran before it) | 4.8-5.4 GB (whole run) |
| D4 hand-over, per window | 86-110 s | 5.8-6.2 GB (whole run) |
| Schott-fair pipeline, per window (spikes) | 344-387 s plus the hand-over | |
| Edit: L round over the dirty catchments' box (R1h ridge +60 m) | 0.02 s (round only), 0.32 s (D4 + round) | |
| Edit: Barnes 2016 local part, one 768-cell tile | 0.06-0.09 s | |

- A window covers 1,020 x 1,020 regional cells (830 km a side). Its 620-cell core covers about 255,000 km2.
- Per landmass: about 4 cores per million km2 of land, so one round at L costs about 6 min per million km2 on Rasierklinge (8 threads), plus about the same again for the D4 hand-over. Overlapping margins would make this lower.
- Not run: the scorecard descriptors per variant (the lead's quota ruling, 2026-11-14). Relief at 2 x 2 and 10 x 10 cells is the only relief measure here.

## Findings
- **Drainage:**
  - One round at L drains every window: basins 0.98 / 1.78 / 4.40 % against 54-67 % today, and no lakes at D = 20 m.
  - D4 alone does not drain (11-22 %).
- **Narrow incision:** the round over-incises at the scale a viewer reads as a canyon. Valley depth at ring 1-2 is 5.29 % on R1h (relief 62 m at 10 x 10), against 0.06-1.82 % on Earth at 1.5-2 x that relief and 3.66 % in the Alps at about 5 x.
  - D4 + round doubles it (9.95 %).
  - On the flat windows R3a and R3b it stays at or under 0.9 %, where Earth's flat tiles read 0-0.1 %.
- **SPIKE-117's canyon:** its 0 % was the burn below drained surface. That is right as K7's cut, but it is 0 for any draining surface (Earth 0.00-0.53 %), so it does not judge canyons. Its "meets the canyon bar" does not hold for visible valleys.
- **Ring 2-4:** R1h's 16.50 % lies inside Earth's spread for 95-130 m relief (2.97-30.83 %). The 1 % bar would fail real mountains.
- **D4:** it raises the edge agreement (R1h 36 to 42 of 44, R3a 33-34 to 43 of 84) and aligns rivers to the grid axes (66.6-72.3 % against 42.6-47.4 %).
- **Verdict:** the round needs tuning before it becomes the decision.

## Tuning candidates (not run)
- **K scaled to the cell size:** the round's erodibility tuned at 814 m cells cuts too fast at 101.8 m.
- **Hillslope diffusion or thermal widening after the round:** widens the narrow incision toward Earth's ring-1-2 share.
- **Schott-style retargeting (SPIKE-118):** coarse scales first, then the fine scale toward a target, which bounds the incision per scale.

## Visuals
Spikes' sheets and obliques: [images/README.md](images/README.md). The obliques come from stores built with kernel versions 7/9, before the GEN-173 cap. Nothing new here: the table is numbers only.
